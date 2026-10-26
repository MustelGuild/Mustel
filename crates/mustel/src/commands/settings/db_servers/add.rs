//! `mustel settings db-servers add` — cadastra um servidor.
//!
//! Paridade observada no oráculo (T9) e no código legado:
//! - com `-n` **e** `-H`, vai direto; faltando qualquer um, entra no fluxo interativo (e `-i` força
//!   o interativo, adição pedida pelo usuário);
//! - validação recusada → `ArgumentException` no legado → exit **41** (mensagem `Invalid host: 'x'.`
//!   etc.); no fluxo interativo as mesmas checagens **não** lançam: imprimem e devolvem **1**;
//! - servidor com nome já existente é **erro** (exit **40**), não atualização — mesmo existindo
//!   `add_or_update` no core, o comando legado recusa;
//! - sucesso imprime `Server 'x' added successfully.` e a dica do `set-password` (exit 0).

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::error::CoreError;
use mustel_core::postgres::tls::SslMode;
use mustel_core::usecases::db_servers;
use mustel_core::validation::{is_port_in_range, is_valid_host, is_valid_username};

use crate::cli::DbServersAddArgs;
use crate::ui::prompts;

/// Nome do binário usado nas mensagens de orientação (o legado dizia `fur`).
const BINARY: &str = "mustel";

/// Executa o comando.
pub fn run(args: DbServersAddArgs) -> Result<i32, CoreError> {
    let interactive_requested = args.interactive
        || args.name.as_deref().is_none_or(str::is_empty)
        || args.host.as_deref().is_none_or(str::is_empty);

    if interactive_requested {
        return add_interactive(&args);
    }

    add_direct(&args)
}

/// Caminho com todas as flags: valida, grava e (com `-W`) guarda a senha no cofre.
fn add_direct(args: &DbServersAddArgs) -> Result<i32, CoreError> {
    let name = require(args.name.as_deref(), "--name (-n)")?;
    let host = require(args.host.as_deref(), "--host (-H)")?;

    let store = ConfigStore::with_default_path()?;
    let server = build_entry(args, name, host)?;
    db_servers::add(&store, &server)?;

    let password = args
        .password
        .clone()
        .or_else(|| std::env::var("MUSTEL_PASSWORD").ok());

    if let Some(password) = password.as_deref() {
        db_servers::set_password(&store, name, password)?;
    }

    println!("Server '{name}' added successfully.");
    println!("Use '{BINARY} settings db-servers set-password {name}' to set the password.");
    Ok(0)
}

/// Fluxo interativo: mesmos campos e ordem do legado.
fn add_interactive(args: &DbServersAddArgs) -> Result<i32, CoreError> {
    let name = prompts::text("Server name*: ", args.name.as_deref())?;
    if name.trim().is_empty() {
        println!("Server name is required.");
        return Ok(1);
    }

    let host = prompts::text("Host*: ", args.host.as_deref())?;
    if host.trim().is_empty() {
        println!("Host is required.");
        return Ok(1);
    }
    if !is_valid_host(&host) {
        println!("Invalid host: '{host}'.");
        return Ok(1);
    }

    let port = prompts::text("Port [5432]: ", Some(&args.port.to_string()))?
        .trim()
        .parse::<i64>()
        .unwrap_or(0);
    if !is_port_in_range(port) {
        println!("Invalid port: {port}. Must be between 1 and 65535.");
        return Ok(1);
    }

    let username = prompts::text("Username [postgres]: ", Some("postgres"))?;
    if !is_valid_username(&username) {
        println!("Invalid username: '{username}'.");
        return Ok(1);
    }

    let databases = split_list(&prompts::text(
        "Databases (comma-separated, optional): ",
        Some(""),
    )?);

    let fetch_all = prompts::confirm("Auto-discover databases? ", false)?;
    let exclude_patterns = if fetch_all {
        let patterns = prompts::text(
            "Exclude patterns (comma-separated) [template*,postgres]: ",
            Some("template*,postgres"),
        )?;
        split_list_or_default(&patterns)
    } else {
        default_exclude_patterns()
    };

    let ssl_choices = [
        "Disable",
        "Allow",
        "Prefer",
        "Require",
        "VerifyCA",
        "VerifyFull",
    ]
    .map(str::to_string);
    let ssl_mode = prompts::select_one("SSL Mode [Prefer]: ", &ssl_choices, Some(2))?;

    let timeout = ask_number("Connection timeout [30s]: ", 30)?;
    let max_parallelism = ask_number("Max Parallelism [4]: ", 4)?;

    let store = ConfigStore::with_default_path()?;
    if store.get_server(&name)?.is_some() {
        // O fluxo interativo recusa antes do menu, com texto próprio (sem a dica do nome alternativo).
        return Err(CoreError::InvalidState(format!(
            "Server '{name}' already exists."
        )));
    }

    // Menu de ação: mesmas opções do legado, na mesma ordem.
    let actions = [
        "Save and set password",
        "Save and test connection",
        "Save without password",
        "Cancel",
    ]
    .map(str::to_string);
    let action = prompts::select_one("Action:", &actions, None)?;

    if action == "Cancel" {
        println!("Operation cancelled.");
        return Ok(0);
    }

    let server = ServerConfigEntry {
        name: name.clone(),
        host,
        port: u16::try_from(port).unwrap_or(5432),
        username,
        databases,
        fetch_all_databases: fetch_all,
        exclude_patterns,
        ssl_mode,
        timeout: u32::try_from(timeout).unwrap_or(30),
        max_parallelism: u32::try_from(max_parallelism).unwrap_or(4),
    };
    db_servers::add(&store, &server)?;

    match action.as_str() {
        "Save and set password" => {
            let password = prompts::password("Password: ")?;
            db_servers::set_password(&store, &name, &password)?;
            println!("Password saved securely for '{name}'.");
        }
        "Save and test connection" => {
            println!("Server '{name}' added successfully.");
            println!("Run '{BINARY} settings db-servers test {name}' to test the connection.");
        }
        _ => {
            println!("Server '{name}' added successfully.");
            println!("Use '{BINARY} settings db-servers set-password {name}' to set the password.");
        }
    }

    Ok(0)
}

fn require<'a>(value: Option<&'a str>, flag: &str) -> Result<&'a str, CoreError> {
    match value.filter(|text| !text.trim().is_empty()) {
        Some(text) => Ok(text),
        None => Err(CoreError::InvalidArgument(format!("{flag} is required."))),
    }
}

fn ask_number(message: &str, default: i32) -> Result<i64, CoreError> {
    let answer = prompts::text(message, Some(&default.to_string()))?;
    Ok(answer.trim().parse::<i64>().unwrap_or(i64::from(default)))
}

fn split_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_list_or_default(input: &str) -> Vec<String> {
    let parsed = split_list(input);
    if parsed.is_empty() {
        default_exclude_patterns()
    } else {
        parsed
    }
}

fn default_exclude_patterns() -> Vec<String> {
    vec!["template*".to_string(), "postgres".to_string()]
}

/// Recusa `--ssl-mode` fora do enum: um valor inválido gravado no cadastro conectaria em texto puro.
fn validate_ssl_mode(raw: Option<&str>) -> Result<(), CoreError> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };

    raw.parse::<SslMode>()
        .map(|_| ())
        .map_err(|error| CoreError::InvalidArgument(error.to_string()))
}

/// Monta a entrada a partir das flags (defaults iguais aos do legado).
///
/// A **porta** é checada aqui, e não no núcleo, porque a mensagem do legado cita o valor cru da flag
/// (`Invalid port: 99999.`), que não sobrevive à conversão para `u16` da entrada.
fn build_entry(
    args: &DbServersAddArgs,
    name: &str,
    host: &str,
) -> Result<ServerConfigEntry, CoreError> {
    validate_ssl_mode(args.ssl_mode.as_deref())?;

    if !is_port_in_range(i64::from(args.port)) {
        return Err(CoreError::InvalidArgument(format!(
            "Invalid port: {}. Must be between 1 and 65535.",
            args.port
        )));
    }

    Ok(ServerConfigEntry {
        name: name.to_string(),
        host: host.to_string(),
        port: u16::try_from(args.port).unwrap_or(5432),
        username: args.username.clone().unwrap_or_default(),
        databases: args
            .databases
            .as_deref()
            .map(split_list)
            .unwrap_or_default(),
        fetch_all_databases: args.fetch_all_databases,
        exclude_patterns: args
            .exclude_patterns
            .as_deref()
            .map(split_list_or_default)
            .unwrap_or_else(default_exclude_patterns),
        ssl_mode: args
            .ssl_mode
            .clone()
            .unwrap_or_else(|| "Prefer".to_string()),
        timeout: args
            .timeout
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(30),
        max_parallelism: args
            .max_parallelism
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(4),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invalid_ssl_mode_is_refused() {
        assert!(validate_ssl_mode(Some("Require")).is_ok());
        assert!(validate_ssl_mode(None).is_ok());

        let error = validate_ssl_mode(Some("Requiree")).expect_err("inválido");
        assert!(error.to_string().contains("invalid ssl mode"), "{error}");
    }

    #[test]
    /// CONTRACT: IF-120
    fn split_list_trims_and_drops_empties() {
        assert_eq!(split_list("mydb, app_dev ,"), vec!["mydb", "app_dev"]);
        assert!(split_list("   ").is_empty());
    }

    #[test]
    /// CONTRACT: IF-121
    fn missing_required_flags_report_the_legacy_text() {
        let error = require(None, "--name (-n)").expect_err("must fail");
        assert_eq!(error.to_string(), "--name (-n) is required.");
        assert_eq!(
            error.exit_code(),
            mustel_core::error::exit_code::INVALID_ARGUMENT
        );
    }

    #[test]
    /// CONTRACT: IF-119
    fn exclude_patterns_fall_back_to_the_defaults() {
        assert_eq!(split_list_or_default(""), default_exclude_patterns());
        assert_eq!(split_list_or_default("a",), vec!["a"]);
    }
}
