//! `mustel database restore` como caso de uso do núcleo (T40).
//!
//! Paridade conferida no `DatabaseRestoreCommand` do legado:
//!
//! - **validação** ([`validate`], [`FailureKind::Usage`] → **2** no CLI): host, porta, usuário (só
//!   se informado), banco (a menos de `--all`), `--ssl-mode` restrito a
//!   `Disable|Prefer|Require|VerifyCA|VerifyFull` (**`Allow` é recusado** aqui, embora exista na
//!   tradução de TLS do núcleo), `--timeout` e `--command-timeout` positivos;
//! - **tipos de falha**: binário ausente e falha do `pg_restore` são
//!   [`FailureKind::MissingBinary`]/[`FailureKind::OperationFailed`] (o CLI mapeia os dois para **1** —
//!   no `backup` o binário ausente é **3**, porque os `catch` dos dois comandos são diferentes);
//!   caminho inválido e arquivo ausente são `Usage` (**2**); "nome de banco inválido" é
//!   `OperationFailed` (**1**) com a mensagem **duas vezes** (uma do `AnsiConsole`, outra do `catch`);
//! - arquivo do dump: `<banco>.dump` no diretório atual quando o posicional `[file]` não vem; o
//!   caminho passa por `SecurityUtils.IsValidPath` antes de existir checagem de existência;
//! - `CreateDatabaseIfNeeded` roda **antes** do `pg_restore`: um `psql` com `SELECT 1 FROM pg_database
//!   WHERE datname = '<banco>'`; se o `psql` sair com **3**, o banco é criado com
//!   `CREATE DATABASE "<banco>"` (aspas dobradas). Outros códigos do `SELECT` são **ignorados**;
//! - `pg_restore` recebe `-h -p -U -d`, depois `-c` (a menos de `--no-clean`), `-v`, `--no-owner`,
//!   `--no-acl`, `--single-transaction`, `-j N` (se `jobs > 0`) e o caminho do arquivo **por último**;
//! - o ambiente leva `PGPASSWORD` e, quando informados, `PGSSLMODE`, `PGCONNECT_TIMEOUT` e
//!   `PGCOMMAND_TIMEOUT` (só o `pg_restore` os recebe; o `psql` do `SELECT` só leva `PGPASSWORD`);
//! - `--all`: varre `*.dump` do diretório (só o nível de cima); diretório inexistente →
//!   `OperationFailed` (**1**); sem arquivos → [`CoreEvent::NoDumpFilesFound`] e termina **0**; um
//!   arquivo por banco, com o nome vindo do arquivo e pulado quando não é identificador válido
//!   ([`CoreEvent::SkippingDatabase`]); falha por banco → [`CoreEvent::DatabaseStepFailed`] e o comando
//!   segue, terminando **0**.

use std::path::PathBuf;

use crate::events::{CoreEvent, DatabaseStep, EventSink};
use crate::postgres::client;
use crate::postgres::locator::{PG_RESTORE, PSQL, PostgresBinaryLocator};
use crate::postgres::settings::ConnectionSettings;
use crate::process::{self, ProcessOptions};
use crate::signal::CancellationToken;
use crate::usecases::database::{
    CommandFailure, Connection, DatabaseFailure, FailureKind, absolute_path, binary_not_found,
    dump_file_name, dump_files,
};
use crate::validation;

/// Modos de `--ssl-mode` que o legado aceita neste comando.
const VALID_SSL_MODES: [&str; 5] = ["Disable", "Prefer", "Require", "VerifyCA", "VerifyFull"];

/// Entrada do `restore`, como o frontend a tem (flags cruas, senha já resolvida).
#[derive(Debug, Clone, Default)]
pub struct RestoreOptions {
    /// Banco a restaurar; dispensado com `all`.
    pub database: Option<String>,
    /// Arquivo do dump (default: `<banco>.dump` no diretório atual).
    pub input_file: Option<String>,
    /// Host (default `localhost`).
    pub host: Option<String>,
    /// Porta (default `5432`), como texto.
    pub port: Option<String>,
    /// Usuário; `None`/branco não é validado, como no legado.
    pub username: Option<String>,
    /// `--ssl-mode`: vai para `PGSSLMODE` do `pg_restore`.
    pub ssl_mode: Option<String>,
    /// `--timeout`: vai para `PGCONNECT_TIMEOUT`.
    pub timeout: Option<i32>,
    /// `--command-timeout`: vai para `PGCOMMAND_TIMEOUT`.
    pub command_timeout: Option<i32>,
    /// `--all`: restaura todos os `*.dump` do diretório.
    pub all: bool,
    /// Diretório do `--all` (default: o atual).
    pub directory: Option<String>,
    /// `-v` do `pg_restore`.
    pub verbose: bool,
    /// `--no-clean`: não manda `-c`.
    pub no_clean: bool,
    /// `--no-owner`.
    pub no_owner: bool,
    /// `--no-acl`.
    pub no_acl: bool,
    /// `--single-transaction`.
    pub single_transaction: bool,
    /// `-j N` (só se `N > 0`).
    pub jobs: Option<i32>,
    /// `--create-if-not-exists` (adição do Mustel): cria pelo driver em vez do `psql`.
    pub create_if_not_exists: bool,
}

/// Valida os argumentos na ordem e com as mensagens do legado.
pub fn validate(options: &RestoreOptions) -> Result<Connection, CommandFailure> {
    let host = options
        .host
        .clone()
        .unwrap_or_else(|| "localhost".to_string());
    let port = options.port.clone().unwrap_or_else(|| "5432".to_string());
    let username = options
        .username
        .clone()
        .filter(|value| !value.trim().is_empty());

    if !validation::is_valid_host(&host) {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            format!("Error: Invalid host: '{host}'"),
        ));
    }
    if !validation::is_valid_port(&port) {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            format!("Error: Invalid port: '{port}'. Port must be between 1 and 65535."),
        ));
    }
    if let Some(username) = &username
        && !validation::is_valid_username(username)
    {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            format!("Error: Invalid username: '{username}'"),
        ));
    }

    let database = options.database.clone().unwrap_or_default();
    if !options.all && database.trim().is_empty() {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            "Error: Database name is required when --all is not specified.",
        ));
    }

    if let Some(ssl_mode) = options
        .ssl_mode
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        && !VALID_SSL_MODES
            .iter()
            .any(|valid| valid.eq_ignore_ascii_case(ssl_mode))
    {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            format!(
                "Error: Invalid SSL mode: '{ssl_mode}'. Valid values: Disable, Prefer, Require, \
                 VerifyCA, VerifyFull."
            ),
        ));
    }

    if options.timeout.is_some_and(|value| value <= 0) {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            "Error: Timeout must be a positive integer.",
        ));
    }
    if options.command_timeout.is_some_and(|value| value <= 0) {
        return Err(CommandFailure::new(
            FailureKind::Usage,
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

/// Executa o restore com a conexão e a senha já resolvidas.
pub async fn restore(
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    if options.all {
        restore_all(options, connection, password, locator, events, token).await
    } else {
        restore_single(options, connection, password, locator, token).await
    }
}

/// Restaura um banco a partir de um arquivo.
///
/// O caminho do arquivo é validado aqui (`IsValidPath` → existência), **como no legado**: o
/// `RestoreSingleDatabase` faz as duas checagens antes de procurar o binário.
async fn restore_single(
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    let input_path = options
        .input_file
        .clone()
        .unwrap_or_else(|| dump_file_name(&connection.database));
    let full_path = resolve_input_file(&input_path)?;

    pg_restore_file(options, connection, password, locator, &full_path, token).await
}

/// Caminho do arquivo já validado e existente, ou a falha do legado.
///
/// A ordem é a do legado: `Path.GetFullPath` → `IsValidPath` (→ exit 2) → existência (→ exit 2).
/// Repare que o legado constrói a exceção de travessia com o caminho **original** e a de arquivo
/// ausente com o caminho **absoluto** (`PathTraversalException(inputPath)` vs
/// `FurLabFileNotFoundException(fullPath)`).
fn resolve_input_file(input_path: &str) -> Result<String, CommandFailure> {
    let full_path = absolute_path(input_path);

    if !validation::is_valid_path(&full_path) {
        return Err(CommandFailure::new(
            FailureKind::Usage,
            format!("Error: Path traversal detected: {input_path}"),
        ));
    }

    if !std::path::Path::new(&full_path).is_file() {
        return Err(CommandFailure::new(
            FailureKind::Usage,
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
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    full_path: &str,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    let Some(pg_restore) = locator.find_pg_restore() else {
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::MissingBinary,
            format!("Error: {}", binary_not_found(PG_RESTORE)),
        )));
    };

    create_database_if_needed(
        options,
        connection,
        password,
        &connection.database,
        locator,
        token,
    )
    .await?;

    let process_options = ProcessOptions::new(&pg_restore)
        .with_args(build_pg_restore_args(options, connection, full_path))
        .with_env("PGPASSWORD", password);
    let process_options = with_connection_environment(process_options, options);

    let outcome = process::run(&process_options, token).await?;
    if !outcome.success() {
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::OperationFailed,
            format!(
                "Error: pg_restore failed with exit code {}. Error: {}",
                outcome.exit_code, outcome.stderr
            ),
        )));
    }

    Ok(())
}

/// Restaura todos os `*.dump` de um diretório.
async fn restore_all(
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    let directory = options.directory.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .display()
            .to_string()
    });
    let directory = PathBuf::from(directory);

    if !directory.is_dir() {
        // O `DirectoryNotFoundException` cai no `catch (Exception)` do comando → exit 1.
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::OperationFailed,
            format!(
                "Error: Directory not found: {}",
                absolute_path(&directory.display().to_string())
            ),
        )));
    }

    let dumps = dump_files(&directory);
    if dumps.is_empty() {
        // `LogWarning`: aparece (o nível mínimo é Warning).
        events.emit(CoreEvent::NoDumpFilesFound);

        return Ok(());
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
            events.emit(CoreEvent::SkippingDatabase {
                database: &database,
            });
            continue;
        }

        let single = RestoreOptions {
            database: Some(database.clone()),
            ..clone_options(options)
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
            Ok(()) => {}
            // Uma falha por arquivo não derruba o comando: o legado embrulha a mensagem da exceção
            // no `LogError` (`Failed to restore database 'x': {ex.Message}`) e segue.
            Err(DatabaseFailure::Command(failure)) => {
                events.emit(CoreEvent::DatabaseStepFailed {
                    step: DatabaseStep::Restore,
                    database: &database,
                    error: failure.bare_message(),
                });
            }
            Err(failure) => return Err(failure),
        }
    }

    Ok(())
}

/// Argumentos do `pg_restore`, na ordem do legado (o arquivo vem por último).
pub fn build_pg_restore_args(
    options: &RestoreOptions,
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

    if !options.no_clean {
        argv.push("-c".to_string());
    }
    if options.verbose {
        argv.push("-v".to_string());
    }
    if options.no_owner {
        argv.push("--no-owner".to_string());
    }
    if options.no_acl {
        argv.push("--no-acl".to_string());
    }
    if options.single_transaction {
        argv.push("--single-transaction".to_string());
    }
    if let Some(jobs) = options.jobs.filter(|value| *value > 0) {
        argv.push("-j".to_string());
        argv.push(jobs.to_string());
    }

    argv.push(file_path.to_string());
    argv
}

/// Acrescenta ao ambiente as variáveis de conexão que o legado só passa ao `pg_restore`.
fn with_connection_environment(
    mut options: ProcessOptions,
    restore: &RestoreOptions,
) -> ProcessOptions {
    if let Some(ssl_mode) = restore
        .ssl_mode
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        options = options.with_env("PGSSLMODE", ssl_mode);
    }
    if let Some(timeout) = restore.timeout {
        options = options.with_env("PGCONNECT_TIMEOUT", timeout.to_string());
    }
    if let Some(command_timeout) = restore.command_timeout {
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
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    database: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    if !validation::is_valid_postgres_identifier(database) {
        // O legado imprime a mensagem no `AnsiConsole` **e** deixa a exceção ser pega pelo `catch`,
        // que imprime de novo — duas linhas iguais, exit 1.
        let message = format!("Error: Invalid database name: '{database}'");
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::OperationFailed,
            format!("{message}\n{message}"),
        )));
    }

    if options.create_if_not_exists {
        return create_database_through_driver(options, connection, password, database).await;
    }

    let Some(psql) = locator.find_psql() else {
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::MissingBinary,
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
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::OperationFailed,
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
    options: &RestoreOptions,
    connection: &Connection,
    password: &str,
    database: &str,
) -> Result<(), DatabaseFailure> {
    let settings = ConnectionSettings {
        host: connection.host.clone(),
        port: connection.port.parse().unwrap_or(5432),
        user: connection.username.clone().unwrap_or_default(),
        password: Some(password.to_string()),
        database: Some("postgres".to_string()),
        ssl_mode: options
            .ssl_mode
            .as_deref()
            .unwrap_or_default()
            .parse()
            .unwrap_or_default(),
        connect_timeout: std::time::Duration::from_secs(options.timeout.unwrap_or(30).max(1) as u64),
        ..ConnectionSettings::default()
    };

    client::create_database_if_missing(&settings, database)
        .await
        .map(|_created| ())
        .map_err(|error| {
            DatabaseFailure::Command(CommandFailure::new(
                FailureKind::OperationFailed,
                format!("Error: Failed to create database: {error}"),
            ))
        })
}

/// `psql` com o `SELECT`/`CREATE` do legado (só `PGPASSWORD` no ambiente).
fn psql_command(
    psql: &std::path::Path,
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

/// Cópia das opções (o `--all` reaproveita quase tudo, trocando só o banco).
fn clone_options(options: &RestoreOptions) -> RestoreOptions {
    RestoreOptions {
        database: options.database.clone(),
        input_file: options.input_file.clone(),
        host: options.host.clone(),
        port: options.port.clone(),
        username: options.username.clone(),
        ssl_mode: options.ssl_mode.clone(),
        timeout: options.timeout,
        command_timeout: options.command_timeout,
        all: false,
        directory: options.directory.clone(),
        verbose: options.verbose,
        no_clean: options.no_clean,
        no_owner: options.no_owner,
        no_acl: options.no_acl,
        single_transaction: options.single_transaction,
        jobs: options.jobs,
        create_if_not_exists: options.create_if_not_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Escuta que guarda os eventos em texto, para os testes conferirem a sequência.
    struct Collector<'a>(&'a Mutex<Vec<String>>);

    impl EventSink for Collector<'_> {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.lock().expect("collector").push(format!("{event:?}"));
        }
    }

    fn options(database: Option<&str>) -> RestoreOptions {
        RestoreOptions {
            database: database.map(str::to_string),
            ..RestoreOptions::default()
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

    #[test]
    fn validation_refuses_the_legacy_cases_with_the_usage_kind() {
        let mut bad_ssl = options(Some("mydb"));
        bad_ssl.ssl_mode = Some("Allow".to_string());
        let error = validate(&bad_ssl).expect_err("Allow é recusado no restore");
        assert_eq!(error.kind, FailureKind::Usage);
        assert_eq!(
            error.message,
            "Error: Invalid SSL mode: 'Allow'. Valid values: Disable, Prefer, Require, VerifyCA, \
             VerifyFull."
        );

        let mut zero_timeout = options(Some("mydb"));
        zero_timeout.timeout = Some(0);
        assert_eq!(
            validate(&zero_timeout).expect_err("recusa").message,
            "Error: Timeout must be a positive integer."
        );

        let mut zero_command_timeout = options(Some("mydb"));
        zero_command_timeout.command_timeout = Some(0);
        assert_eq!(
            validate(&zero_command_timeout).expect_err("recusa").message,
            "Error: Command timeout must be a positive integer."
        );
    }

    #[test]
    fn a_missing_file_is_usage_with_the_absolute_path_in_the_message() {
        let error = resolve_input_file("nao-existe.dump").expect_err("recusa");

        assert_eq!(error.kind, FailureKind::Usage, "o CLI mapeia para 2");
        assert!(
            error.message.starts_with("Error: File not found: "),
            "{error:?}"
        );
        assert!(error.message.ends_with("nao-existe.dump"));
    }

    #[tokio::test]
    async fn a_missing_pg_restore_is_a_missing_binary_failure_and_the_cli_maps_it_to_1() {
        let directory = tempfile::tempdir().expect("tempdir");
        let dump = directory.path().join("mydb.dump");
        std::fs::write(&dump, b"").expect("escreve");
        let mut command = options(Some("mydb"));
        command.input_file = Some(dump.display().to_string());

        let error = restore(
            &command,
            &connection("mydb"),
            "senha",
            &empty_locator(),
            &crate::events::NoopSink,
            &crate::signal::cancellation_token(),
        )
        .await
        .expect_err("sem binário");

        match error {
            DatabaseFailure::Command(failure) => {
                assert_eq!(failure.kind, FailureKind::MissingBinary);
                assert_eq!(
                    failure.message,
                    format!("Error: {}", binary_not_found(PG_RESTORE))
                );
            }
            DatabaseFailure::Fatal(error) => panic!("não é erro do núcleo: {error}"),
        }
    }

    #[tokio::test]
    async fn an_missing_directory_is_an_operation_failure_with_exit_1_in_the_cli() {
        let mut command = options(None);
        command.all = true;
        command.directory = Some("diretorio-que-nao-existe".to_string());

        let error = restore(
            &command,
            &connection("mydb"),
            "senha",
            &empty_locator(),
            &crate::events::NoopSink,
            &crate::signal::cancellation_token(),
        )
        .await
        .expect_err("diretório não existe");

        match error {
            DatabaseFailure::Command(failure) => {
                assert_eq!(failure.kind, FailureKind::OperationFailed);
                assert!(
                    failure.message.starts_with("Error: Directory not found: "),
                    "{}",
                    failure.message
                );
            }
            DatabaseFailure::Fatal(error) => panic!("não é erro do núcleo: {error}"),
        }
    }

    #[tokio::test]
    async fn an_empty_all_directory_emits_the_warning_event_and_succeeds() {
        let directory = tempfile::tempdir().expect("tempdir");
        let mut command = options(None);
        command.all = true;
        command.directory = Some(directory.path().display().to_string());

        let events = Mutex::new(Vec::new());
        restore(
            &command,
            &connection("mydb"),
            "senha",
            &empty_locator(),
            &Collector(&events),
            &crate::signal::cancellation_token(),
        )
        .await
        .expect("sem arquivos não é erro");

        assert_eq!(
            events.lock().expect("collector").len(),
            1,
            "só o aviso: {:?}",
            events.lock().expect("collector")
        );
        assert!(events.lock().expect("collector")[0].contains("NoDumpFilesFound"));
    }

    #[tokio::test]
    async fn an_invalid_database_name_in_the_directory_is_skipped_with_an_event() {
        let directory = tempfile::tempdir().expect("tempdir");
        std::fs::write(directory.path().join("nome com espaço.dump"), b"").expect("escreve");
        let mut command = options(None);
        command.all = true;
        command.directory = Some(directory.path().display().to_string());

        let events = Mutex::new(Vec::new());
        restore(
            &command,
            &connection("mydb"),
            "senha",
            &empty_locator(),
            &Collector(&events),
            &crate::signal::cancellation_token(),
        )
        .await
        .expect("nome inválido não é erro no --all");

        let seen = events.lock().expect("collector");
        assert!(
            seen.iter().any(|event| event.contains("SkippingDatabase")),
            "{seen:?}"
        );
    }

    #[test]
    fn argv_follows_the_legacy_order_with_the_file_last() {
        let mut command = options(Some("mydb"));
        command.verbose = true;
        command.no_owner = true;
        command.no_acl = true;
        command.single_transaction = true;
        command.jobs = Some(4);

        let argv = build_pg_restore_args(&command, &connection("mydb"), "mydb.dump");

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
                "-v",
                "--no-owner",
                "--no-acl",
                "--single-transaction",
                "-j",
                "4",
                "mydb.dump",
            ]
        );
    }

    #[test]
    fn argv_omits_clean_and_non_positive_jobs() {
        let mut command = options(Some("mydb"));
        command.no_clean = true;
        command.jobs = Some(0);

        let argv = build_pg_restore_args(&command, &connection("mydb"), "mydb.dump");

        assert!(
            !argv.contains(&"-c".to_string()),
            "`--no-clean` tira o `-c`"
        );
        assert!(!argv.contains(&"-j".to_string()), "jobs 0 não entra");
        assert_eq!(argv.last().map(String::as_str), Some("mydb.dump"));
    }
}
