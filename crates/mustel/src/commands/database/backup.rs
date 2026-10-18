//! `mustel database backup` — dump de um banco (ou de todos) com o `pg_dump`.
//!
//! Paridade conferida no `DatabaseBackupCommand` do legado:
//!
//! - **validação** (exit **2**, texto do legado): host inválido → `Error: Invalid host: 'x'`; porta →
//!   `Error: Invalid port: 'x'. Port must be between 1 and 65535.`; usuário (só se informado) →
//!   `Error: Invalid username: 'x'`; sem banco e sem `--all` →
//!   `Error: Database name is required when --all is not specified.`;
//! - a senha vem de `-W` → senha guardada do servidor que casa com **host:porta** → prompt
//!   (`Enter password: `), nessa ordem ([`mustel_core::postgres::password`]);
//! - banco único: sem `pg_dump` localizável → `Error: PostgreSQL binary 'pg_dump' not found. ...` e
//!   exit **3** (código do corpo do comando, não do `Program.cs`); `pg_dump` com código != 0 →
//!   `Error: pg_dump failed with exit code N. Error: <stderr>` e exit **1**;
//! - `--all`: o `pg_dump` é procurado **dentro do laço** (uma vez por banco, como no legado), a
//!   falha de listagem sai como exit **40** (`InvalidOperationException` no legado) e uma falha por
//!   banco **não** derruba o comando: imprime o erro daquele banco e segue, terminando com **0**;
//! - `--ssl-mode`, `--timeout` e `--command-timeout` são aceitos e **ignorados**, como no legado: não
//!   entram nos argumentos do `pg_dump` nem na conexão de `--all`;
//! - listar bancos usa o **driver** (D12) com a mesma query que o legado faz por `psql` — a
//!   divergência já registrada do `settings db-servers test`; o legado exige o `psql` instalado;
//! - **sucesso não imprime nada**: o progresso do legado é `ILogger.LogInformation` e o container
//!   registra o logger com `SetMinimumLevel(LogLevel.Warning)`. O que sobra é o arquivo (e o resumo
//!   do `--all`, também `LogInformation`, não aparece).
//!
//! No cancelamento a árvore do `pg_dump` é morta e o comando sai com **130** (o legado não ligava o
//! token ao processo: o Ctrl+C matava o grupo inteiro pelo console).

use mustel_core::config::ConfigStore;
use mustel_core::credentials::SystemKeyring;
use mustel_core::error::CoreError;
use mustel_core::postgres::client::{connect, list_databases};
use mustel_core::postgres::locator::{PG_DUMP, PostgresBinaryLocator};
use mustel_core::postgres::password::{self, PASSWORD_PROMPT};
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::process::{self, ProcessOptions};
use mustel_core::signal::CancellationToken;
use mustel_core::validation;

use crate::cli::DatabaseBackupArgs;
use crate::ui::prompts;

use super::{Failure, Printed, dump_file_name};

/// Conexão validada, sem a senha.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Connection {
    host: String,
    port: String,
    username: Option<String>,
    database: String,
}

/// Executa o comando.
pub async fn run(args: DatabaseBackupArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    run_with(
        args,
        &PostgresBinaryLocator::new(),
        &stored_password_from_environment,
        token,
    )
    .await
}

/// Variante com o localizador de binários e a fonte da senha guardada injetados (testes).
///
/// A senha guardada entra como closure `(host, porta) -> Option<String>` porque é isso que o legado
/// consulta (`ResolvePasswordFromUserConfig`) — e porque assim o teste **não** toca a configuração
/// nem o cofre reais da máquina.
pub(crate) async fn run_with(
    args: DatabaseBackupArgs,
    locator: &PostgresBinaryLocator,
    stored_password: &dyn Fn(&str, &str) -> Option<String>,
    token: &CancellationToken,
) -> Result<i32, CoreError> {
    match run_inner(args, locator, stored_password, token).await {
        Ok(exit) => Ok(exit),
        Err(failure) => failure.resolve(),
    }
}

/// Fluxo do comando, com as falhas do legado no tipo [`Failure`].
async fn run_inner(
    args: DatabaseBackupArgs,
    locator: &PostgresBinaryLocator,
    stored_password: &dyn Fn(&str, &str) -> Option<String>,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let connection = validate(&args).map_err(Failure::Printed)?;
    let password = resolve_password(&args, &connection, stored_password)?;

    if args.all {
        backup_all(&args, &connection, &password, locator, token).await
    } else {
        backup_single(&args, &connection, &password, locator, token).await
    }
}

/// Senha guardada do servidor que casa com `host:porta` (config + cofre reais).
///
/// Sem diretório de configuração ou sem servidor correspondente não há senha guardada — o legado
/// engolia esses casos num `try`/`catch` vazio pelo mesmo motivo.
fn stored_password_from_environment(host: &str, port: &str) -> Option<String> {
    let config = ConfigStore::with_default_path().ok()?;
    password::stored_password(&config, &SystemKeyring::new(), host, port)
}

/// Valida host, porta, usuário e banco, na ordem e com as mensagens do legado.
fn validate(args: &DatabaseBackupArgs) -> Result<Connection, Printed> {
    let host = args.host.clone().unwrap_or_else(|| "localhost".to_string());
    let port = args.port.clone().unwrap_or_else(|| "5432".to_string());
    let username = args
        .username
        .clone()
        .filter(|value| !value.trim().is_empty());

    if !validation::is_valid_host(&host) {
        return Err(Printed::new(2, format!("Error: Invalid host: '{host}'")));
    }
    if !validation::is_valid_port(&port) {
        return Err(Printed::new(
            2,
            format!("Error: Invalid port: '{port}'. Port must be between 1 and 65535."),
        ));
    }
    if let Some(username) = &username
        && !validation::is_valid_username(username)
    {
        return Err(Printed::new(
            2,
            format!("Error: Invalid username: '{username}'"),
        ));
    }

    let database = args.database.clone().unwrap_or_default();
    if !args.all && database.trim().is_empty() {
        return Err(Printed::new(
            2,
            "Error: Database name is required when --all is not specified.",
        ));
    }

    Ok(Connection {
        host,
        port,
        username,
        database,
    })
}

/// Resolve a senha: flag → senha guardada do servidor (cofre) → prompt.
fn resolve_password(
    args: &DatabaseBackupArgs,
    connection: &Connection,
    stored_password: &dyn Fn(&str, &str) -> Option<String>,
) -> Result<String, CoreError> {
    let stored = stored_password(&connection.host, &connection.port);

    password::resolve_password(args.password.as_deref(), stored.as_deref(), || {
        prompts::password(PASSWORD_PROMPT)
    })
}

/// Dump de um banco só.
async fn backup_single(
    args: &DatabaseBackupArgs,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let Some(pg_dump) = locator.find_pg_dump() else {
        return Err(Failure::Printed(Printed::new(
            3,
            format!("Error: {}", binary_not_found(PG_DUMP)),
        )));
    };

    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| dump_file_name(&connection.database));

    match run_pg_dump(&pg_dump, args, connection, password, &output_path, token).await? {
        PgDumpOutcome::Success => Ok(0),
        PgDumpOutcome::Failed { exit_code, stderr } => {
            // O `stderr` entra **como veio**: o `ReadToEnd` do legado preserva a quebra de linha
            // final, e ela aparece na saída.
            Err(Failure::Printed(Printed::new(
                1,
                format!("Error: pg_dump failed with exit code {exit_code}. Error: {stderr}"),
            )))
        }
    }
}

/// Dump de todos os bancos do servidor, um arquivo por banco.
async fn backup_all(
    args: &DatabaseBackupArgs,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let databases = list_all_databases(connection, password).await?;

    if databases.is_empty() {
        // O legado usa `LogWarning`, que **aparece** (o nível mínimo é Warning).
        println!("No databases found.");
        return Ok(0);
    }

    let output_dir = args.output.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .display()
            .to_string()
    });
    let output_dir = std::path::PathBuf::from(output_dir);
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(CoreError::Io)?;
    }

    for database in databases {
        if token.is_cancelled() {
            break;
        }

        let single = Connection {
            host: connection.host.clone(),
            port: connection.port.clone(),
            username: connection.username.clone(),
            database: database.clone(),
        };
        let output_path = output_dir.join(dump_file_name(&database));
        let output_path = output_path.display().to_string();

        // O `pg_dump` é localizado dentro do laço, como no legado: um banco sem binário conta como
        // falha daquele banco, não do comando.
        let Some(pg_dump) = locator.find_pg_dump() else {
            println!(
                "Failed to backup database '{database}': {}",
                binary_not_found(PG_DUMP)
            );
            continue;
        };

        match run_pg_dump(&pg_dump, args, &single, password, &output_path, token).await {
            // Uma falha por banco não derruba o comando: o legado imprime o erro e segue.
            Ok(PgDumpOutcome::Failed { exit_code, .. }) => println!(
                "Failed to backup database '{database}': pg_dump failed with exit code {exit_code}"
            ),
            Ok(PgDumpOutcome::Success) => {}
            // Cancelamento e erro de execução não são "falha daquele banco" e sobem.
            Err(error) => return Err(error),
        }
    }

    // O legado devolve 0 mesmo com bancos falhando (o resumo dele é `LogInformation`, filtrado).
    Ok(0)
}

/// Lista os bancos do servidor com o driver (D12).
async fn list_all_databases(
    connection: &Connection,
    password: &str,
) -> Result<Vec<String>, Failure> {
    let settings = ConnectionSettings {
        host: connection.host.clone(),
        port: connection.port.parse().unwrap_or(5432),
        user: connection.username.clone().unwrap_or_default(),
        password: Some(password.to_string()),
        database: Some("postgres".to_string()),
        ..ConnectionSettings::default()
    };

    let client = connect(&settings).await.map_err(list_error)?;
    list_databases(&client).await.map_err(list_error)
}

/// Embrulha a falha de listagem como o legado: `InvalidOperationException` com a mensagem
/// `Failed to list databases: ...` e exit **40**.
fn list_error(error: CoreError) -> Failure {
    Failure::Fatal(CoreError::InvalidState(format!(
        "Failed to list databases: {error}"
    )))
}

/// Como terminou um `pg_dump` que chegou a rodar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PgDumpOutcome {
    /// Terminou com 0.
    Success,
    /// Terminou com código != 0 (o `stderr` alimenta a mensagem do caminho de banco único).
    Failed {
        /// Código de saída do processo.
        exit_code: i32,
        /// stderr capturado, como veio.
        stderr: String,
    },
}

/// Roda o `pg_dump` com os argumentos do legado.
async fn run_pg_dump(
    pg_dump: &std::path::Path,
    args: &DatabaseBackupArgs,
    connection: &Connection,
    password: &str,
    output_path: &str,
    token: &CancellationToken,
) -> Result<PgDumpOutcome, Failure> {
    let options = ProcessOptions::new(pg_dump)
        .with_args(build_pg_dump_args(args, connection, output_path))
        .with_env("PGPASSWORD", password);

    let outcome = process::run(&options, token).await?;

    if outcome.success() {
        return Ok(PgDumpOutcome::Success);
    }

    Ok(PgDumpOutcome::Failed {
        exit_code: outcome.exit_code,
        stderr: outcome.stderr,
    })
}

/// Mensagem do legado quando o binário não é encontrado.
fn binary_not_found(binary: &str) -> String {
    format!(
        "PostgreSQL binary '{binary}' not found. Please ensure PostgreSQL is installed and the \
         binaries are in your PATH."
    )
}

/// Argumentos do `pg_dump`, na ordem do legado.
///
/// `-F` vem **colado** ao formato (`-Fc`), e os padrões de exclusão entram depois de `-v`, com
/// `--exclude-table` antes de `--exclude-table-data`.
fn build_pg_dump_args(
    args: &DatabaseBackupArgs,
    connection: &Connection,
    output_path: &str,
) -> Vec<String> {
    let format = args.format.clone().unwrap_or_else(|| "c".to_string());
    let mut argv = vec![
        format!("-F{format}"),
        "-h".to_string(),
        connection.host.clone(),
        "-p".to_string(),
        connection.port.clone(),
        "-U".to_string(),
        connection.username.clone().unwrap_or_default(),
        "-d".to_string(),
        connection.database.clone(),
        "-f".to_string(),
        output_path.to_string(),
    ];

    if args.verbose {
        argv.push("-v".to_string());
    }

    for pattern in &args.exclude {
        if !pattern.trim().is_empty() {
            argv.push("--exclude-table".to_string());
            argv.push(pattern.clone());
        }
    }

    for pattern in &args.exclude_table_data {
        if !pattern.trim().is_empty() {
            argv.push("--exclude-table-data".to_string());
            argv.push(pattern.clone());
        }
    }

    argv
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::signal::cancellation_token;

    fn args(database: Option<&str>) -> DatabaseBackupArgs {
        DatabaseBackupArgs {
            database: database.map(str::to_string),
            host: None,
            port: None,
            username: None,
            password: Some("senha".to_string()),
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            all: false,
            exclude: Vec::new(),
            exclude_table_data: Vec::new(),
            verbose: false,
            output: None,
            format: None,
        }
    }

    fn connection(database: &str) -> Connection {
        Connection {
            host: "localhost".to_string(),
            port: "5432".to_string(),
            username: Some("postgres".to_string()),
            database: database.to_string(),
        }
    }

    /// Localizador que não acha nada (nenhum PATH, nenhuma pasta comum).
    fn empty_locator() -> PostgresBinaryLocator {
        PostgresBinaryLocator::with_search_paths(None, Vec::new(), Vec::new())
    }

    /// Fonte de senha guardada que sempre tem uma senha (não toca cofre nem config reais).
    fn always_stored(_host: &str, _port: &str) -> Option<String> {
        Some("guardada".to_string())
    }

    /// Fonte de senha guardada vazia.
    fn no_stored(_host: &str, _port: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_are_localhost_and_5432() {
        let connection = validate(&args(Some("mydb"))).expect("válido");

        assert_eq!(connection.host, "localhost");
        assert_eq!(connection.port, "5432");
        assert_eq!(
            connection.username, None,
            "usuário não tem default no legado"
        );
        assert_eq!(connection.database, "mydb");
    }

    #[test]
    fn an_invalid_host_is_refused_with_the_legacy_message() {
        let mut command = args(Some("mydb"));
        command.host = Some("host_com_underscore".to_string());

        assert_eq!(
            validate(&command).expect_err("recusa").message,
            "Error: Invalid host: 'host_com_underscore'"
        );
    }

    #[test]
    fn an_invalid_port_is_refused_with_the_legacy_message() {
        let mut command = args(Some("mydb"));
        command.port = Some("70000".to_string());

        assert_eq!(
            validate(&command).expect_err("recusa").message,
            "Error: Invalid port: '70000'. Port must be between 1 and 65535."
        );
    }

    #[test]
    fn an_invalid_username_is_refused_but_an_absent_one_is_fine() {
        let mut invalid = args(Some("mydb"));
        invalid.username = Some("com espaco".to_string());
        assert_eq!(
            validate(&invalid).expect_err("recusa").message,
            "Error: Invalid username: 'com espaco'"
        );

        // Usuário ausente não é validado (o legado só valida quando há valor).
        assert!(validate(&args(Some("mydb"))).is_ok());

        // Só espaço conta como ausente.
        let mut blank = args(Some("mydb"));
        blank.username = Some("   ".to_string());
        assert_eq!(validate(&blank).expect("válido").username, None);
    }

    #[test]
    fn the_database_is_required_unless_all_is_used() {
        for database in [None, Some(""), Some("   ")] {
            assert_eq!(
                validate(&args(database)).expect_err("recusa").message,
                "Error: Database name is required when --all is not specified."
            );
        }

        let mut all = args(None);
        all.all = true;
        assert!(validate(&all).is_ok(), "`--all` dispensa o banco");
    }

    #[tokio::test]
    async fn a_missing_pg_dump_is_exit_3_with_the_legacy_message() {
        let exit = run_with(
            args(Some("mydb")),
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 3);
    }

    #[tokio::test]
    async fn validation_failures_are_exit_2_and_do_not_touch_anything() {
        for database in [None, Some("")] {
            let exit = run_with(
                args(database),
                &empty_locator(),
                &always_stored,
                &cancellation_token(),
            )
            .await
            .expect("roda");
            assert_eq!(exit, 2, "banco {database:?}");
        }
    }

    #[tokio::test]
    async fn a_password_that_only_the_flag_has_avoids_the_prompt() {
        // `-W` preenchido: nem a fonte guardada nem o prompt entram, então o comando chega ao
        // `pg_dump` (que não existe) e sai com 3 — sem terminal.
        let exit = run_with(
            args(Some("mydb")),
            &empty_locator(),
            &no_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 3);
    }

    #[tokio::test]
    async fn without_any_password_the_prompt_fails_clearly_outside_a_terminal() {
        let mut command = args(Some("mydb"));
        command.password = None;

        let error = run_with(command, &empty_locator(), &no_stored, &cancellation_token())
            .await
            .expect_err("sem terminal não dá para perguntar a senha");

        // Divergência deliberada registrada: o legado quebra com erro de console; o Mustel explica
        // o que fazer e sai com 40.
        assert_eq!(
            error.exit_code(),
            mustel_core::error::exit_code::INVALID_STATE
        );
        assert!(
            error.to_string().contains("interactive terminal"),
            "{error}"
        );
    }

    #[test]
    fn the_binary_not_found_message_is_the_legacy_one() {
        assert_eq!(
            binary_not_found(PG_DUMP),
            "PostgreSQL binary 'pg_dump' not found. Please ensure PostgreSQL is installed and the \
             binaries are in your PATH."
        );
    }

    #[test]
    fn argv_follows_the_legacy_order() {
        let argv = build_pg_dump_args(&args(Some("mydb")), &connection("mydb"), "mydb.dump");

        assert_eq!(
            argv,
            vec![
                "-Fc",
                "-h",
                "localhost",
                "-p",
                "5432",
                "-U",
                "postgres",
                "-d",
                "mydb",
                "-f",
                "mydb.dump",
            ]
        );
    }

    #[test]
    fn argv_glues_the_format_and_keeps_an_empty_username() {
        let mut command = args(Some("mydb"));
        command.format = Some("p".to_string());
        let mut connection = connection("mydb");
        connection.username = None;

        let argv = build_pg_dump_args(&command, &connection, "saida.dump");

        assert_eq!(argv[0], "-Fp", "`-F` vem colado ao formato");
        assert_eq!(
            argv[6], "",
            "o legado passa string vazia quando não há usuário"
        );
    }

    #[test]
    fn argv_puts_verbose_before_the_exclusion_patterns() {
        let mut command = args(Some("mydb"));
        command.verbose = true;
        command.exclude = vec!["log*".to_string(), "temp_*".to_string()];
        command.exclude_table_data = vec!["audit".to_string()];

        let argv = build_pg_dump_args(&command, &connection("mydb"), "mydb.dump");

        assert_eq!(
            argv[11..],
            [
                "-v",
                "--exclude-table",
                "log*",
                "--exclude-table",
                "temp_*",
                "--exclude-table-data",
                "audit",
            ]
        );
    }

    #[test]
    fn argv_skips_blank_exclusion_patterns() {
        let mut command = args(Some("mydb"));
        command.exclude = vec!["".to_string(), "   ".to_string()];
        command.exclude_table_data = vec!["\t".to_string()];

        let argv = build_pg_dump_args(&command, &connection("mydb"), "mydb.dump");

        assert_eq!(argv.len(), 11, "padrões em branco não entram: {argv:?}");
    }

    #[test]
    fn a_process_error_that_is_not_cancellation_is_printed_as_exit_1() {
        let spawn = mustel_core::process::ProcessError::Spawn {
            program: "pg_dump".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };

        match Failure::from(spawn) {
            Failure::Printed(printed) => {
                assert_eq!(printed.code, 1);
                assert!(
                    printed.message.starts_with("Error: "),
                    "{}",
                    printed.message
                );
            }
            Failure::Fatal(error) => panic!("não pode subir como erro do núcleo: {error}"),
        }

        // Cancelamento sobe como cancelamento (exit 130).
        match Failure::from(mustel_core::process::ProcessError::Cancelled) {
            Failure::Fatal(error) => {
                assert_eq!(error.exit_code(), mustel_core::error::exit_code::CANCELLED)
            }
            Failure::Printed(printed) => {
                panic!("cancelamento não é saída impressa: {}", printed.message)
            }
        }
    }
}
