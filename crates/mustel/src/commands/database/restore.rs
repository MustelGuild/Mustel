//! `mustel database restore` — restaura um dump (ou todos de um diretório) com o `pg_restore`.
//!
//! Paridade conferida no `DatabaseRestoreCommand` do legado:
//!
//! - **validação** (exit **2**): host, porta, usuário (só se informado), banco (a menos de `--all`),
//!   `--ssl-mode` restrito a `Disable|Prefer|Require|VerifyCA|VerifyFull` (**`Allow` é recusado**
//!   aqui, embora exista na tradução de TLS do núcleo), `--timeout` e `--command-timeout` positivos;
//! - **exit codes** próprios: binário ausente e falha do `pg_restore` saem com **1** (no `backup` o
//!   binário ausente sai com 3 — os `catch` dos dois comandos são diferentes), caminho inválido e
//!   arquivo ausente saem com **2**, e "nome de banco inválido" imprime a mensagem **duas vezes**
//!   (uma do `AnsiConsole`, outra do `catch`) e sai com **1**;
//! - arquivo do dump: `<banco>.dump` no diretório atual quando o posicional `[file]` não vem;
//!   o caminho passa por `SecurityUtils.IsValidPath` antes de existir checagem de existência;
//! - **sem o arquivo o legado nem chega a perguntar a senha**? Não: `GetPassword` vem antes, então
//!   sem senha configurada ele quebra no prompt — é a divergência deliberada já registrada;
//! - `CreateDatabaseIfNeeded` roda **antes** do `pg_restore`: um `psql` com `SELECT 1 FROM pg_database
//!   WHERE datname = '<banco>'`; se o `psql` sair com **3**, o banco é criado com
//!   `CREATE DATABASE "<banco>"` (aspas dobradas). Outros códigos do `SELECT` são **ignorados**, como
//!   no legado;
//! - `pg_restore` recebe `-h -p -U -d`, depois `-c` (a menos de `--no-clean`), `-v`, `--no-owner`,
//!   `--no-acl`, `--single-transaction`, `-j N` (se `jobs > 0`) e o caminho do arquivo **por último**;
//! - o ambiente leva `PGPASSWORD` e, quando informados, `PGSSLMODE`, `PGCONNECT_TIMEOUT` e
//!   `PGCOMMAND_TIMEOUT` (só o `pg_restore` os recebe; o `psql` do `SELECT` só leva `PGPASSWORD`);
//! - `--all`: varre `*.dump` do diretório (só o nível de cima); diretório inexistente cai no catch
//!   genérico → exit **1**; sem arquivos → aviso `No .dump files found in the directory.` (aparece,
//!   porque é `LogWarning`) e exit 0; um arquivo por banco, com o nome vindo do arquivo e pulado
//!   quando não é identificador válido (`Skipping invalid database name: 'x'`); falha por banco não
//!   derruba o comando → termina com **0**.

use std::path::{Path, PathBuf};

use mustel_core::config::ConfigStore;
use mustel_core::credentials::SystemKeyring;
use mustel_core::error::CoreError;
use mustel_core::postgres::client;
use mustel_core::postgres::locator::{PG_RESTORE, PSQL, PostgresBinaryLocator};
use mustel_core::postgres::password::{self, PASSWORD_PROMPT};
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::process::{self, ProcessOptions};
use mustel_core::signal::CancellationToken;
use mustel_core::validation;

use crate::cli::DatabaseRestoreArgs;
use crate::ui::prompts;

use super::{DUMP_EXTENSION, Failure, Printed, dump_file_name};

/// Modos de `--ssl-mode` que o legado aceita neste comando.
const VALID_SSL_MODES: [&str; 5] = ["Disable", "Prefer", "Require", "VerifyCA", "VerifyFull"];

/// Conexão validada, sem a senha.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Connection {
    host: String,
    port: String,
    username: Option<String>,
    database: String,
}

/// Executa o comando.
pub async fn run(args: DatabaseRestoreArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    run_with(
        args,
        &PostgresBinaryLocator::new(),
        &stored_password_from_environment,
        token,
    )
    .await
}

/// Variante com o localizador de binários e a fonte da senha guardada injetados (testes).
pub(crate) async fn run_with(
    args: DatabaseRestoreArgs,
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
    args: DatabaseRestoreArgs,
    locator: &PostgresBinaryLocator,
    stored_password: &dyn Fn(&str, &str) -> Option<String>,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let connection = validate(&args).map_err(Failure::Printed)?;
    let password = resolve_password(&args, &connection, stored_password)?;

    if args.all {
        restore_all(&args, &connection, &password, locator, token).await
    } else {
        restore_single(&args, &connection, &password, locator, token).await
    }
}

/// Senha guardada do servidor que casa com `host:porta` (config + cofre reais).
fn stored_password_from_environment(host: &str, port: &str) -> Option<String> {
    let config = ConfigStore::with_default_path().ok()?;
    password::stored_password(&config, &SystemKeyring::new(), host, port)
}

/// Valida os argumentos na ordem e com as mensagens do legado.
fn validate(args: &DatabaseRestoreArgs) -> Result<Connection, Printed> {
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

    if let Some(ssl_mode) = args
        .ssl_mode
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        && !VALID_SSL_MODES
            .iter()
            .any(|valid| valid.eq_ignore_ascii_case(ssl_mode))
    {
        return Err(Printed::new(
            2,
            format!(
                "Error: Invalid SSL mode: '{ssl_mode}'. Valid values: Disable, Prefer, Require, \
                 VerifyCA, VerifyFull."
            ),
        ));
    }

    if args.timeout.is_some_and(|value| value <= 0) {
        return Err(Printed::new(
            2,
            "Error: Timeout must be a positive integer.",
        ));
    }
    if args.command_timeout.is_some_and(|value| value <= 0) {
        return Err(Printed::new(
            2,
            "Error: Command timeout must be a positive integer.",
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
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    stored_password: &dyn Fn(&str, &str) -> Option<String>,
) -> Result<String, Failure> {
    let stored = stored_password(&connection.host, &connection.port);

    Ok(password::resolve_password(
        args.password.as_deref(),
        stored.as_deref(),
        || prompts::password(PASSWORD_PROMPT),
    )?)
}

/// Restaura um banco a partir de um arquivo.
///
/// O caminho do arquivo é validado aqui (`IsValidPath` → existência), **como no legado**: o
/// `RestoreSingleDatabase` faz as duas checagens antes de procurar o binário.
async fn restore_single(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let input_path = args
        .input_file
        .clone()
        .unwrap_or_else(|| dump_file_name(&connection.database));
    let full_path = resolve_input_file(&input_path).map_err(Failure::Printed)?;

    pg_restore_file(args, connection, password, locator, &full_path, token).await
}

/// Caminho do arquivo já validado e existente, ou a falha do legado.
///
/// A ordem é a do legado: `Path.GetFullPath` → `IsValidPath` (→ exit 2) → existência (→ exit 2).
/// Repare que o legado constrói a exceção de travessia com o caminho **original** e a de arquivo
/// ausente com o caminho **absoluto** (`PathTraversalException(inputPath)` vs
/// `FurLabFileNotFoundException(fullPath)`).
fn resolve_input_file(input_path: &str) -> Result<String, Printed> {
    let full_path = absolute_path(input_path);

    if !validation::is_valid_path(&full_path) {
        return Err(Printed::new(
            2,
            format!("Error: Path traversal detected: {input_path}"),
        ));
    }

    if !Path::new(&full_path).is_file() {
        return Err(Printed::new(
            2,
            format!("Error: File not found: {full_path}"),
        ));
    }

    Ok(full_path)
}

/// Roda o `pg_restore` para um arquivo já resolvido.
///
/// O `--all` chama isto **direto**, sem passar por [`resolve_input_file`]: o legado também não
/// revalida o arquivo que veio da listagem do diretório (`RestoreSingleDatabaseInternal` não tem
/// `IsValidPath` nem `File.Exists`).
async fn pg_restore_file(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    full_path: &str,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let Some(pg_restore) = locator.find_pg_restore() else {
        return Err(Failure::Printed(Printed::new(
            1,
            format!("Error: {}", binary_not_found(PG_RESTORE)),
        )));
    };

    create_database_if_needed(
        args,
        connection,
        password,
        &connection.database,
        locator,
        token,
    )
    .await?;

    let options = ProcessOptions::new(&pg_restore)
        .with_args(build_pg_restore_args(args, connection, full_path))
        .with_env("PGPASSWORD", password);
    let options = with_connection_environment(options, args);

    let outcome = process::run(&options, token).await?;
    if !outcome.success() {
        return Err(Failure::Printed(Printed::new(
            1,
            format!(
                "Error: pg_restore failed with exit code {}. Error: {}",
                outcome.exit_code, outcome.stderr
            ),
        )));
    }

    Ok(0)
}

/// Restaura todos os `*.dump` de um diretório.
async fn restore_all(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<i32, Failure> {
    let directory = args.directory.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .display()
            .to_string()
    });
    let directory = PathBuf::from(directory);

    if !directory.is_dir() {
        // O `DirectoryNotFoundException` cai no `catch (Exception)` do comando → exit 1.
        return Err(Failure::Printed(Printed::new(
            1,
            format!(
                "Error: Directory not found: {}",
                absolute_path(&directory.display().to_string())
            ),
        )));
    }

    let dumps = dump_files(&directory);
    if dumps.is_empty() {
        // `LogWarning`: aparece (o nível mínimo é Warning).
        println!("No {DUMP_EXTENSION} files found in the directory.");
        return Ok(0);
    }

    for dump in dumps {
        if token.is_cancelled() {
            break;
        }

        let Some(database) = dump
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };

        if !validation::is_valid_postgres_identifier(&database) {
            // `LogWarning`: aparece.
            println!("Skipping invalid database name: '{database}'");
            continue;
        }

        let single = DatabaseRestoreArgs {
            database: Some(database.clone()),
            ..clone_args(args)
        };
        let single_connection = Connection {
            host: connection.host.clone(),
            port: connection.port.clone(),
            username: connection.username.clone(),
            database: database.clone(),
        };
        let input_path = dump.display().to_string();

        match pg_restore_file(
            &single,
            &single_connection,
            password,
            locator,
            &input_path,
            token,
        )
        .await
        {
            Ok(_) => {}
            // Uma falha por arquivo não derruba o comando: o legado embrulha a mensagem da exceção
            // no `LogError` (`Failed to restore database 'x': {ex.Message}`) e segue.
            Err(Failure::Printed(printed)) => println!(
                "Failed to restore database '{database}': {}",
                printed.bare_message()
            ),
            Err(failure) => return Err(failure),
        }
    }

    Ok(0)
}

/// Argumentos do `pg_restore`, na ordem do legado (o arquivo vem por último).
fn build_pg_restore_args(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    file_path: &str,
) -> Vec<String> {
    let mut argv = vec![
        "-h".to_string(),
        connection.host.clone(),
        "-p".to_string(),
        connection.port.clone(),
        "-U".to_string(),
        connection.username.clone().unwrap_or_default(),
        "-d".to_string(),
        connection.database.clone(),
    ];

    if !args.no_clean {
        argv.push("-c".to_string());
    }
    if args.verbose {
        argv.push("-v".to_string());
    }
    if args.no_owner {
        argv.push("--no-owner".to_string());
    }
    if args.no_acl {
        argv.push("--no-acl".to_string());
    }
    if args.single_transaction {
        argv.push("--single-transaction".to_string());
    }
    if let Some(jobs) = args.jobs.filter(|value| *value > 0) {
        argv.push("-j".to_string());
        argv.push(jobs.to_string());
    }

    argv.push(file_path.to_string());
    argv
}

/// Acrescenta ao ambiente as variáveis de conexão que o legado só passa ao `pg_restore`.
fn with_connection_environment(
    mut options: ProcessOptions,
    args: &DatabaseRestoreArgs,
) -> ProcessOptions {
    if let Some(ssl_mode) = args
        .ssl_mode
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        options = options.with_env("PGSSLMODE", ssl_mode);
    }
    if let Some(timeout) = args.timeout {
        options = options.with_env("PGCONNECT_TIMEOUT", timeout.to_string());
    }
    if let Some(command_timeout) = args.command_timeout {
        options = options.with_env("PGCOMMAND_TIMEOUT", command_timeout.to_string());
    }

    options
}

/// Cria o banco alvo quando ele não existe, como o legado faz por `psql`.
///
/// Com `--create-if-not-exists` (adição do Mustel), a criação é feita **pelo driver** e de fato
/// acontece: o passo por `psql` do legado é substituído, e nem o binário é exigido. Sem a flag, o
/// comportamento é o do legado — inclusive o `psql` que **nunca** cria (o código 3 não acontece).
async fn create_database_if_needed(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    password: &str,
    database: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<(), Failure> {
    if !validation::is_valid_postgres_identifier(database) {
        // O legado imprime a mensagem no `AnsiConsole` **e** deixa a exceção ser pega pelo `catch`,
        // que imprime de novo — duas linhas iguais, exit 1.
        let message = format!("Error: Invalid database name: '{database}'");
        return Err(Failure::Printed(Printed::new(
            1,
            format!("{message}\n{message}"),
        )));
    }

    if args.create_if_not_exists {
        return create_database_through_driver(args, connection, password, database).await;
    }

    let Some(psql) = locator.find_psql() else {
        return Err(Failure::Printed(Printed::new(
            1,
            format!("Error: {}", binary_not_found(PSQL)),
        )));
    };

    let exists_query = format!("SELECT 1 FROM pg_database WHERE datname = '{database}'");
    let check = psql_command(&psql, connection, password, &exists_query);
    let outcome = process::run(&check, token).await?;

    // O legado trata **só** o código 3 como "não existe"; qualquer outro código passa batido.
    if outcome.exit_code != 3 {
        return Ok(());
    }

    let create_query = format!("CREATE DATABASE \"{}\"", database.replace('"', "\"\""));
    let create = psql_command(&psql, connection, password, &create_query);
    let outcome = process::run(&create, token).await?;

    if !outcome.success() {
        return Err(Failure::Printed(Printed::new(
            1,
            format!("Error: Failed to create database: {}", outcome.stderr),
        )));
    }

    Ok(())
}

/// Criação de verdade pelo driver (`--create-if-not-exists`).
///
/// A falha vira `Failed to create database: ...` com exit **1**, como a falha de criação do legado.
/// `--ssl-mode` e `--timeout` são respeitados aqui (a flag é adição do Mustel; o `psql` do caminho do
/// legado os ignorava).
async fn create_database_through_driver(
    args: &DatabaseRestoreArgs,
    connection: &Connection,
    password: &str,
    database: &str,
) -> Result<(), Failure> {
    let settings = ConnectionSettings {
        host: connection.host.clone(),
        port: connection.port.parse().unwrap_or(5432),
        user: connection.username.clone().unwrap_or_default(),
        password: Some(password.to_string()),
        database: Some("postgres".to_string()),
        ssl_mode: args
            .ssl_mode
            .as_deref()
            .unwrap_or_default()
            .parse()
            .unwrap_or_default(),
        connect_timeout: std::time::Duration::from_secs(args.timeout.unwrap_or(30).max(1) as u64),
        ..ConnectionSettings::default()
    };

    client::create_database_if_missing(&settings, database)
        .await
        .map(|_created| ())
        .map_err(|error| {
            Failure::Printed(Printed::new(
                1,
                format!("Error: Failed to create database: {error}"),
            ))
        })
}

/// `psql` com o `SELECT`/`CREATE` do legado (só `PGPASSWORD` no ambiente).
fn psql_command(
    psql: &Path,
    connection: &Connection,
    password: &str,
    query: &str,
) -> ProcessOptions {
    ProcessOptions::new(psql)
        .with_args([
            "-h",
            &connection.host,
            "-p",
            &connection.port,
            "-U",
            &connection.username.clone().unwrap_or_default(),
            "-d",
            "postgres",
            "-c",
            query,
        ])
        .with_env("PGPASSWORD", password)
}

/// Arquivos `*.dump` do diretório (só o nível de cima), em ordem de nome.
fn dump_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case(&DUMP_EXTENSION[1..]))
        })
        .collect();
    files.sort();
    files
}

/// Caminho absoluto, como o `Path.GetFullPath` do legado.
fn absolute_path(path: &str) -> String {
    std::path::absolute(path)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

/// Mensagem do legado quando o binário não é encontrado.
fn binary_not_found(binary: &str) -> String {
    format!(
        "PostgreSQL binary '{binary}' not found. Please ensure PostgreSQL is installed and the \
         binaries are in your PATH."
    )
}

/// Cópia dos argumentos (o `--all` reaproveita quase tudo, trocando só o banco).
fn clone_args(args: &DatabaseRestoreArgs) -> DatabaseRestoreArgs {
    DatabaseRestoreArgs {
        database: args.database.clone(),
        input_file: args.input_file.clone(),
        host: args.host.clone(),
        port: args.port.clone(),
        username: args.username.clone(),
        password: args.password.clone(),
        ssl_mode: args.ssl_mode.clone(),
        timeout: args.timeout,
        command_timeout: args.command_timeout,
        all: false,
        directory: args.directory.clone(),
        verbose: args.verbose,
        no_clean: args.no_clean,
        no_owner: args.no_owner,
        no_acl: args.no_acl,
        single_transaction: args.single_transaction,
        jobs: args.jobs,
        create_if_not_exists: args.create_if_not_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::signal::cancellation_token;

    fn args(database: Option<&str>) -> DatabaseRestoreArgs {
        DatabaseRestoreArgs {
            database: database.map(str::to_string),
            input_file: None,
            host: None,
            port: None,
            username: None,
            password: Some("senha".to_string()),
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            all: false,
            directory: None,
            verbose: false,
            no_clean: false,
            no_owner: false,
            no_acl: false,
            single_transaction: false,
            jobs: None,
            create_if_not_exists: false,
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

    fn empty_locator() -> PostgresBinaryLocator {
        PostgresBinaryLocator::with_search_paths(None, Vec::new(), Vec::new())
    }

    fn always_stored(_host: &str, _port: &str) -> Option<String> {
        Some("guardada".to_string())
    }

    #[test]
    fn the_defaults_are_localhost_and_5432() {
        let connection = validate(&args(Some("mydb"))).expect("válido");

        assert_eq!(connection.host, "localhost");
        assert_eq!(connection.port, "5432");
        assert_eq!(connection.username, None);
        assert_eq!(connection.database, "mydb");
    }

    #[test]
    fn the_database_is_required_unless_all_is_used() {
        assert_eq!(
            validate(&args(None)).expect_err("recusa").message,
            "Error: Database name is required when --all is not specified."
        );

        let mut all = args(None);
        all.all = true;
        assert!(validate(&all).is_ok());
    }

    #[test]
    fn the_ssl_mode_list_is_the_legacy_one_and_allow_is_refused() {
        for valid in VALID_SSL_MODES {
            let mut command = args(Some("mydb"));
            command.ssl_mode = Some(valid.to_string());
            assert!(validate(&command).is_ok(), "deveria aceitar {valid}");
        }

        // `Allow` existe na tradução de TLS do núcleo, mas o legado **não** o aceita aqui.
        for invalid in ["Allow", "allow", "VerifyNone", "x"] {
            let mut command = args(Some("mydb"));
            command.ssl_mode = Some(invalid.to_string());
            assert_eq!(
                validate(&command).expect_err("recusa").message,
                format!(
                    "Error: Invalid SSL mode: '{invalid}'. Valid values: Disable, Prefer, Require, \
                     VerifyCA, VerifyFull."
                )
            );
        }
    }

    #[test]
    fn non_positive_timeouts_are_refused() {
        let mut timeout = args(Some("mydb"));
        timeout.timeout = Some(0);
        assert_eq!(
            validate(&timeout).expect_err("recusa").message,
            "Error: Timeout must be a positive integer."
        );

        let mut command_timeout = args(Some("mydb"));
        command_timeout.command_timeout = Some(-3);
        assert_eq!(
            validate(&command_timeout).expect_err("recusa").message,
            "Error: Command timeout must be a positive integer."
        );

        let mut valid = args(Some("mydb"));
        valid.timeout = Some(1);
        valid.command_timeout = Some(600);
        assert!(validate(&valid).is_ok());
    }

    #[test]
    fn argv_follows_the_legacy_order_and_puts_clean_and_the_file_in_place() {
        let argv =
            build_pg_restore_args(&args(Some("mydb")), &connection("mydb"), "C:\\d\\mydb.dump");

        assert_eq!(
            argv,
            vec![
                "-h",
                "localhost",
                "-p",
                "5432",
                "-U",
                "postgres",
                "-d",
                "mydb",
                "-c",
                "C:\\d\\mydb.dump",
            ]
        );
    }

    #[test]
    fn argv_puts_every_option_before_the_file() {
        let mut command = args(Some("mydb"));
        command.no_clean = true;
        command.verbose = true;
        command.no_owner = true;
        command.no_acl = true;
        command.single_transaction = true;
        command.jobs = Some(4);

        let argv = build_pg_restore_args(&command, &connection("mydb"), "mydb.dump");

        assert_eq!(
            argv[8..],
            [
                "-v",
                "--no-owner",
                "--no-acl",
                "--single-transaction",
                "-j",
                "4",
                "mydb.dump",
            ]
        );
        assert!(
            !argv.contains(&"-c".to_string()),
            "`--no-clean` tira o `-c`"
        );
    }

    #[test]
    fn a_non_positive_jobs_count_is_not_passed() {
        let mut command = args(Some("mydb"));
        command.jobs = Some(0);

        let argv = build_pg_restore_args(&command, &connection("mydb"), "mydb.dump");

        assert!(!argv.contains(&"-j".to_string()), "{argv:?}");
    }

    #[test]
    fn the_connection_environment_is_only_added_when_informed() {
        let plain =
            with_connection_environment(ProcessOptions::new("pg_restore"), &args(Some("mydb")));
        assert_eq!(plain.env.len(), 0);

        let mut command = args(Some("mydb"));
        command.ssl_mode = Some("Require".to_string());
        command.timeout = Some(30);
        command.command_timeout = Some(600);
        let full = with_connection_environment(ProcessOptions::new("pg_restore"), &command);

        assert_eq!(
            full.env,
            vec![
                ("PGSSLMODE".to_string(), "Require".to_string()),
                ("PGCONNECT_TIMEOUT".to_string(), "30".to_string()),
                ("PGCOMMAND_TIMEOUT".to_string(), "600".to_string()),
            ]
        );
    }

    #[test]
    fn a_missing_file_is_exit_2_with_the_absolute_path() {
        let error = resolve_input_file("nao-existe.dump").expect_err("não existe");

        assert_eq!(error.code, 2);
        assert!(
            error.message.starts_with("Error: File not found: ")
                && error.message.ends_with("nao-existe.dump"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_traversal_path_is_exit_2_with_the_original_path() {
        // Quem a checagem pega é o caminho **codificado**: o `Path.GetFullPath`/`absolute` já resolveu
        // `..` antes, então `../x.dump` cai em "File not found" (comportamento observado no oráculo).
        let error = resolve_input_file("%2e%2e%2fsegredo.dump").expect_err("travessia");

        assert_eq!(error.code, 2);
        assert_eq!(
            error.message,
            "Error: Path traversal detected: %2e%2e%2fsegredo.dump"
        );
    }

    #[test]
    fn a_dot_dot_path_is_resolved_before_the_traversal_check() {
        let error = resolve_input_file("../segredo.dump").expect_err("não existe");

        assert_eq!(error.code, 2);
        assert!(
            error.message.starts_with("Error: File not found: "),
            "o `..` é resolvido antes da checagem, como no `Path.GetFullPath`: {}",
            error.message
        );
        assert!(
            !error.message.contains(".."),
            "o caminho resolvido não mantém `..`: {}",
            error.message
        );
    }

    #[test]
    fn the_create_flag_is_off_by_default_and_not_part_of_the_legacy_surface() {
        // A flag é adição do Mustel (`known-divergences.md`): sozinha ela não muda a validação, e o
        // comportamento padrão continua sendo o do legado (falhar quando o banco não existe).
        let padrao = args(Some("mydb"));
        assert!(!padrao.create_if_not_exists);
        assert!(validate(&padrao).is_ok());

        let mut com_flag = args(Some("mydb"));
        com_flag.create_if_not_exists = true;
        assert!(validate(&com_flag).is_ok());
    }

    #[tokio::test]
    async fn a_present_dump_without_pg_restore_is_exit_1_not_3() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let dump = dir.path().join("mydb.dump");
        std::fs::write(&dump, b"nao sou um dump de verdade").expect("cria dump");

        let mut command = args(Some("mydb"));
        command.input_file = Some(dump.display().to_string());

        let exit = run_with(
            command,
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        // No `restore` o binário ausente sai com 1; no `backup` sai com 3.
        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn all_wraps_each_failure_like_the_legacy() {
        // Um `.dump` cujo banco não existe: o legado imprime (via `LogError`)
        // `Failed to restore database 'x': {ex.Message}` e **segue**, terminando com 0.
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(
            dir.path().join("banco_inexistente.dump"),
            b"nao sou um dump",
        )
        .expect("cria dump");

        let mut command = args(None);
        command.all = true;
        command.directory = Some(dir.path().display().to_string());

        // O localizador vazio faz o comando parar antes do `pg_restore`: a mensagem embrulhada
        // também vale para o binário ausente.
        let exit = run_with(
            command,
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 0, "falha por arquivo não derruba o `--all`");
    }

    #[tokio::test]
    async fn validation_failures_are_exit_2() {
        let exit = run_with(
            args(None),
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 2);
    }

    #[tokio::test]
    async fn all_with_a_missing_directory_is_exit_1() {
        let mut command = args(None);
        command.all = true;
        command.directory = Some("mustel-diretorio-que-nao-existe".to_string());

        let exit = run_with(
            command,
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn all_without_dump_files_is_exit_0() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let mut command = args(None);
        command.all = true;
        command.directory = Some(dir.path().display().to_string());

        let exit = run_with(
            command,
            &empty_locator(),
            &always_stored,
            &cancellation_token(),
        )
        .await
        .expect("roda");

        assert_eq!(exit, 0);
    }

    #[test]
    fn only_dump_files_of_the_top_directory_are_listed() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("b.dump"), b"x").expect("cria");
        std::fs::write(dir.path().join("a.dump"), b"x").expect("cria");
        std::fs::write(dir.path().join("outro.sql"), b"x").expect("cria");
        std::fs::create_dir(dir.path().join("sub")).expect("cria dir");
        std::fs::write(dir.path().join("sub").join("c.dump"), b"x").expect("cria");

        let files = dump_files(dir.path());
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            names,
            vec!["a.dump", "b.dump"],
            "ordem de nome e só o nível de cima"
        );
    }
}
