//! `mustel database pgpass add` — grava uma entrada no `pgpass.conf`.
//!
//! Paridade conferida no código legado (`PgPassAddCommand`):
//! - banco ausente ou em branco → `Error: the --database argument is required.` e exit **2**;
//! - host inválido (`SecurityUtils.IsValidHost`) → `Error: invalid host format: "x".` e exit **2**;
//!   `*` é válido;
//! - porta inválida (`SecurityUtils.IsValidPort`) → `Error: port must be a number between 1 and
//!   65535.` e exit **2**; `*` é válido;
//! - senha vazia → `Error: password cannot be empty.` e exit **2**;
//! - `--password` **vazio** conta como ausente e o legado pede a senha na hora (`IsNullOrEmpty`);
//! - sucesso → `Entry added: host:porta:banco:usuário` e exit **0**;
//! - entrada com a mesma identidade já no arquivo → `Entry already exists: ...` e exit **0**
//!   (duplicata **não** é erro, e o arquivo não é tocado);
//! - falha de escrita → mensagem própria e exit **1** (via `map_error`).
//!
//! Os exit codes **2** vêm do corpo do comando legado (`return Task.FromResult(2)`), e não do parser:
//! por isso `Ok(2)` e não um `CoreError` (que mapearia para 41).

use mustel_core::error::CoreError;
use mustel_core::pgpass::PgPassEntry;
use mustel_core::pgpass::entry::{DEFAULT_HOST, DEFAULT_PORT, DEFAULT_USERNAME};
use mustel_core::pgpass::store::PgPassStore;
use mustel_core::validation;

use crate::cli::PgpassAddArgs;
use crate::ui::{password_input, prompts};

use super::map_error;

/// Campos do comando depois das validações de uso.
#[derive(Debug)]
struct Target {
    hostname: String,
    port: String,
    database: String,
    username: String,
}

/// Executa o comando.
pub fn run(args: PgpassAddArgs) -> Result<i32, CoreError> {
    let store = PgPassStore::with_default_path().map_err(map_error)?;

    let target = match resolve_target(&args) {
        Ok(target) => target,
        Err(message) => {
            println!("{message}");
            return Ok(2);
        }
    };

    let stdin_password = if args.password_stdin {
        Some(password_input::read_from_stdin()?)
    } else {
        None
    };
    let environment_password = std::env::var("MUSTEL_PASSWORD").ok();
    let provided = stdin_password
        .as_deref()
        .or(args.password.as_deref())
        .or(environment_password
            .as_deref()
            .filter(|value| !value.is_empty()));
    let password = resolve_password(provided, || prompts::password("Password: "))?;
    if password.is_empty() {
        println!("Error: password cannot be empty.");
        return Ok(2);
    }

    let entry = PgPassEntry {
        hostname: target.hostname,
        port: target.port,
        database: target.database,
        username: target.username,
        password,
    };

    let outcome = store.add(&entry).map_err(map_error)?;

    // `Added` e `Duplicate` saem com 0: a duplicata já é o estado desejado.
    println!("{}", outcome.message(&entry));
    Ok(0)
}

/// Valida banco, host e porta na mesma ordem e com as mesmas mensagens do legado.
///
/// Cada erro sai com exit **2** — código do **corpo** do comando legado, não do parser (que o Mustel
/// remapeia para 1) nem da validação do núcleo (41).
fn resolve_target(args: &PgpassAddArgs) -> Result<Target, String> {
    // O banco vem da forma posicional (documentada) ou de `--database` (forma herdada); o clap já
    // recusa as duas juntas.
    let database = args
        .database_arg
        .clone()
        .or_else(|| args.database.clone())
        .filter(|value| !value.trim().is_empty());
    let Some(database) = database else {
        return Err("Error: the --database argument is required.".to_string());
    };

    let hostname = args
        .host
        .clone()
        .unwrap_or_else(|| DEFAULT_HOST.to_string());
    if !validation::is_valid_host(&hostname) {
        return Err(format!("Error: invalid host format: \"{hostname}\"."));
    }

    let port = args
        .port
        .clone()
        .unwrap_or_else(|| DEFAULT_PORT.to_string());
    if !validation::is_valid_port(&port) {
        return Err("Error: port must be a number between 1 and 65535.".to_string());
    }

    Ok(Target {
        hostname,
        port,
        database,
        username: args
            .username
            .clone()
            .unwrap_or_else(|| DEFAULT_USERNAME.to_string()),
    })
}

/// Resolve a senha: `--password` vazio conta como ausente (`string.IsNullOrEmpty` no legado) e
/// dispara o prompt; só a senha **vazia** é recusada depois.
pub(crate) fn resolve_password(
    provided: Option<&str>,
    prompt: impl FnOnce() -> Result<String, CoreError>,
) -> Result<String, CoreError> {
    match provided.filter(|value| !value.is_empty()) {
        Some(password) => Ok(password.to_string()),
        None => prompt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(database: Option<&str>) -> PgpassAddArgs {
        PgpassAddArgs {
            database_arg: database.map(str::to_string),
            database: None,
            host: None,
            port: None,
            username: None,
            password: None,
            password_stdin: false,
        }
    }

    #[test]
    /// CONTRACT: IF-090
    fn the_database_can_come_from_the_positional_or_the_legacy_option() {
        let positional = resolve_target(&args(Some("mydb"))).expect("posicional");
        assert_eq!(positional.database, "mydb");

        let mut option = args(None);
        option.database = Some("outro".to_string());
        assert_eq!(resolve_target(&option).expect("opção").database, "outro");
    }

    #[test]
    /// CONTRACT: IF-091
    fn a_blank_database_is_refused() {
        for database in [None, Some(""), Some("   ")] {
            let error = resolve_target(&args(database)).expect_err("deve recusar");
            assert_eq!(error, "Error: the --database argument is required.");
        }
    }

    #[test]
    /// CONTRACT: IF-093
    fn defaults_match_the_legacy_entry_record() {
        let target = resolve_target(&args(Some("mydb"))).expect("válido");
        assert_eq!(target.hostname, "localhost");
        assert_eq!(target.port, "5432");
        assert_eq!(target.username, "postgres");
    }

    #[test]
    /// CONTRACT: IF-092
    fn an_invalid_host_is_refused_and_the_wildcard_is_accepted() {
        let mut invalid = args(Some("mydb"));
        invalid.host = Some("host_com_underscore".to_string());
        assert_eq!(
            resolve_target(&invalid).expect_err("deve recusar"),
            "Error: invalid host format: \"host_com_underscore\"."
        );

        let mut wildcard = args(Some("mydb"));
        wildcard.host = Some("*".to_string());
        assert_eq!(resolve_target(&wildcard).expect("curinga").hostname, "*");
    }

    #[test]
    /// CONTRACT: IF-092
    fn an_invalid_port_is_refused_and_the_wildcard_is_accepted() {
        for port in ["0", "70000", "abc", ""] {
            let mut command = args(Some("mydb"));
            command.port = Some(port.to_string());
            assert_eq!(
                resolve_target(&command).expect_err("deve recusar"),
                "Error: port must be a number between 1 and 65535."
            );
        }

        let mut wildcard = args(Some("mydb"));
        wildcard.port = Some("*".to_string());
        assert_eq!(resolve_target(&wildcard).expect("curinga").port, "*");
    }

    #[test]
    /// CONTRACT: IF-094
    fn an_empty_password_flag_means_absence_and_prompts() {
        for provided in [None, Some("")] {
            let password =
                resolve_password(provided, || Ok("digitada".to_string())).expect("prompt");
            assert_eq!(password, "digitada");
        }
    }

    #[test]
    /// CONTRACT: IF-094
    fn a_provided_password_is_used_verbatim() {
        // Espaço é senha válida para o legado; só a string vazia é ausência.
        assert_eq!(
            resolve_password(Some(" "), || unreachable!()).expect("senha"),
            " "
        );
        assert_eq!(
            resolve_password(Some("senha:com:dois"), || unreachable!()).expect("senha"),
            "senha:com:dois"
        );
    }

    #[test]
    fn an_empty_password_typed_at_the_prompt_is_still_empty() {
        let password = resolve_password(None, || Ok(String::new())).expect("prompt");
        assert!(password.is_empty(), "o comando recusa com exit 2");
    }
}
