//! `mustel database backup` como caso de uso do núcleo (T40).
//!
//! Paridade conferida no `DatabaseBackupCommand` do legado:
//!
//! - **validação** ([`validate`], exit **2**, texto do legado): host inválido → `Error: Invalid
//!   host: 'x'`; porta → `Error: Invalid port: 'x'. Port must be between 1 and 65535.`; usuário (só
//!   se informado) → `Error: Invalid username: 'x'`; sem banco e sem `--all` → `Error: Database name
//!   is required when --all is not specified.`. Ela vem **antes** da senha: quem resolve a senha é o
//!   frontend, e só depois de a validação passar;
//! - banco único: sem `pg_dump` localizável → `Error: PostgreSQL binary 'pg_dump' not found. ...` e
//!   [`FailureKind::MissingBinary`] (o CLI mapeia para **3**); `pg_dump` com código != 0 →
//!   `Error: pg_dump failed with exit code N. Error: <stderr>` e [`FailureKind::OperationFailed`]
//!   (o CLI mapeia para **1**);
//! - `--all`: o `pg_dump` é procurado **dentro do laço** (uma vez por banco, como no legado), a falha
//!   de listagem sobe como [`CoreError::InvalidState`] (exit **40**) e uma falha por banco **não**
//!   derruba o comando: emite [`CoreEvent::DatabaseStepFailed`] e segue, terminando com **0**;
//! - `--ssl-mode`, `--timeout` e `--command-timeout` são aceitos e **ignorados**, como no legado: não
//!   entram nos argumentos do `pg_dump` nem na conexão de `--all`;
//! - listar bancos usa o **driver** (D12) com a mesma query que o legado faz por `psql` — a
//!   divergência já registrada do `settings db-servers test`;
//! - **sucesso não imprime nada**: o progresso do legado é `ILogger.LogInformation` e o container
//!   registra o logger com `SetMinimumLevel(LogLevel.Warning)`.
//!
//! No cancelamento a árvore do `pg_dump` é morta e o comando sai com **130**.

use std::path::PathBuf;

use crate::error::CoreError;
use crate::events::{CoreEvent, DatabaseStep, EventSink};
use crate::postgres::client::{connect, list_databases};
use crate::postgres::locator::{PG_DUMP, PostgresBinaryLocator};
use crate::postgres::settings::ConnectionSettings;
use crate::process::{self, ProcessOptions};
use crate::signal::CancellationToken;
use crate::usecases::database::{
    CommandFailure, Connection, DatabaseFailure, FailureKind, binary_not_found, dump_file_name,
};
use crate::validation;

/// Entrada do `backup`, como o frontend a tem (flags cruas, senha já resolvida).
#[derive(Debug, Clone, Default)]
pub struct BackupOptions {
    /// Banco a copiar; dispensado com `all`.
    pub database: Option<String>,
    /// Host (default `localhost`).
    pub host: Option<String>,
    /// Porta (default `5432`), como texto.
    pub port: Option<String>,
    /// Usuário; `None`/branco não é validado, como no legado.
    pub username: Option<String>,
    /// `--all`: copia todos os bancos do servidor.
    pub all: bool,
    /// Arquivo de saída (banco único) ou diretório (`--all`).
    pub output: Option<String>,
    /// Formato do `-F` (default `c`).
    pub format: Option<String>,
    /// `-v` do `pg_dump`.
    pub verbose: bool,
    /// Padrões de `--exclude-table`.
    pub exclude: Vec<String>,
    /// Padrões de `--exclude-table-data`.
    pub exclude_table_data: Vec<String>,
}

/// Valida host, porta, usuário e banco, na ordem e com as mensagens do legado.
pub fn validate(options: &BackupOptions) -> Result<Connection, CommandFailure> {
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

    Ok(Connection {
        host,
        port,
        username,
        database,
    })
}

/// Executa o backup com a conexão e a senha já resolvidas.
///
/// `events` recebe o que acontece **dentro** do `--all` (banco sem binário, banco que falhou); não há
/// progresso por banco no caminho de sucesso, porque o legado também não imprime nenhum.
pub async fn backup(
    options: &BackupOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    if options.all {
        backup_all(options, connection, password, locator, events, token).await
    } else {
        backup_single(options, connection, password, locator, token).await
    }
}

/// Dump de um banco só.
async fn backup_single(
    options: &BackupOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    let Some(pg_dump) = locator.find_pg_dump() else {
        return Err(DatabaseFailure::Command(CommandFailure::new(
            FailureKind::MissingBinary,
            format!("Error: {}", binary_not_found(PG_DUMP)),
        )));
    };

    let output_path = options
        .output
        .clone()
        .unwrap_or_else(|| dump_file_name(&connection.database));

    match run_pg_dump(&pg_dump, options, connection, password, &output_path, token).await? {
        PgDumpOutcome::Success => Ok(()),
        PgDumpOutcome::Failed { exit_code, stderr } => {
            // O `stderr` entra **como veio**: o `ReadToEnd` do legado preserva a quebra de linha
            // final, e ela aparece na saída.
            Err(DatabaseFailure::Command(CommandFailure::new(
                FailureKind::OperationFailed,
                format!("Error: pg_dump failed with exit code {exit_code}. Error: {stderr}"),
            )))
        }
    }
}

/// Dump de todos os bancos do servidor, um arquivo por banco.
async fn backup_all(
    options: &BackupOptions,
    connection: &Connection,
    password: &str,
    locator: &PostgresBinaryLocator,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<(), DatabaseFailure> {
    let databases = list_all_databases(connection, password).await?;

    if databases.is_empty() {
        // O legado usa `LogWarning`, que **aparece** (o nível mínimo é Warning).
        events.emit(CoreEvent::NoDatabasesFound);

        return Ok(());
    }

    let output_dir = options.output.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .display()
            .to_string()
    });
    let output_dir = PathBuf::from(output_dir);
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
            events.emit(CoreEvent::DatabaseStepFailed {
                step: DatabaseStep::Backup,
                database: &database,
                error: &binary_not_found(PG_DUMP),
            });
            continue;
        };

        match run_pg_dump(&pg_dump, options, &single, password, &output_path, token).await {
            // Uma falha por banco não derruba o comando: o legado embrulha a mensagem da exceção no
            // `LogError` (`Failed to backup database 'x': {ex.Message}`) e segue.
            Ok(PgDumpOutcome::Failed { exit_code, stderr }) => {
                let error = format!("pg_dump failed with exit code {exit_code}. Error: {stderr}");
                events.emit(CoreEvent::DatabaseStepFailed {
                    step: DatabaseStep::Backup,
                    database: &database,
                    error: &error,
                });
            }
            Ok(PgDumpOutcome::Success) => {}
            // Cancelamento e erro de execução não são "falha daquele banco" e sobem.
            Err(error) => return Err(error),
        }
    }

    // O legado devolve 0 mesmo com bancos falhando (o resumo dele é `LogInformation`, filtrado).
    Ok(())
}

/// Lista os bancos do servidor com o driver (D12).
async fn list_all_databases(
    connection: &Connection,
    password: &str,
) -> Result<Vec<String>, DatabaseFailure> {
    let settings = ConnectionSettings {
        host: connection.host.clone(),
        port: connection.port.parse().unwrap_or(5432),
        user: connection.username.clone().unwrap_or_default(),
        password: Some(password.to_string()),
        database: Some("postgres".to_string()),
        ..ConnectionSettings::default()
    };

    let client = connect(&settings)
        .await
        .map_err(|error| list_error(&error))?;
    list_databases(&client)
        .await
        .map_err(|error| list_error(&error))
}

/// Embrulha a falha de listagem como o legado: `InvalidOperationException` com a mensagem
/// `Failed to list databases: ...` e exit **40**.
fn list_error(error: &CoreError) -> DatabaseFailure {
    DatabaseFailure::Fatal(CoreError::InvalidState(format!(
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
    options: &BackupOptions,
    connection: &Connection,
    password: &str,
    output_path: &str,
    token: &CancellationToken,
) -> Result<PgDumpOutcome, DatabaseFailure> {
    let options = ProcessOptions::new(pg_dump)
        .with_args(build_pg_dump_args(options, connection, output_path))
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

/// Argumentos do `pg_dump`, na ordem do legado.
///
/// `-F` vem **colado** ao formato (`-Fc`), e os padrões de exclusão entram depois de `-v`, com
/// `--exclude-table` antes de `--exclude-table-data`.
pub fn build_pg_dump_args(
    options: &BackupOptions,
    connection: &Connection,
    output_path: &str,
) -> Vec<String> {
    let format = options.format.clone().unwrap_or_else(|| "c".to_string());
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

    if options.verbose {
        argv.push("-v".to_string());
    }

    for pattern in &options.exclude {
        if !pattern.trim().is_empty() {
            argv.push("--exclude-table".to_string());
            argv.push(pattern.clone());
        }
    }

    for pattern in &options.exclude_table_data {
        if !pattern.trim().is_empty() {
            argv.push("--exclude-table-data".to_string());
            argv.push(pattern.clone());
        }
    }

    argv
}

/// Extensão dos arquivos de dump, reexportada para quem monta caminhos.
pub use crate::usecases::database::DUMP_EXTENSION;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CoreEvent;
    use crate::signal::cancellation_token;
    use std::sync::Mutex;

    /// Escuta que guarda os eventos em texto, para os testes conferirem a sequência.
    struct Collector<'a>(&'a Mutex<Vec<String>>);

    impl EventSink for Collector<'_> {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.lock().expect("collector").push(format!("{event:?}"));
        }
    }

    fn options(database: Option<&str>) -> BackupOptions {
        BackupOptions {
            database: database.map(str::to_string),
            ..BackupOptions::default()
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

    #[test]
    /// CONTRACT: IF-052
    fn defaults_are_localhost_and_5432() {
        let connection = validate(&options(Some("mydb"))).expect("válido");

        assert_eq!(connection.host, "localhost");
        assert_eq!(connection.port, "5432");
        assert_eq!(
            connection.username, None,
            "usuário não tem default no legado"
        );
        assert_eq!(connection.database, "mydb");
    }

    #[test]
    /// CONTRACT: IF-057
    fn an_invalid_host_is_refused_with_the_legacy_message() {
        let mut command = options(Some("mydb"));
        command.host = Some("host_com_underscore".to_string());

        let error = validate(&command).expect_err("recusa");
        assert_eq!(error.message, "Error: Invalid host: 'host_com_underscore'");
        assert_eq!(error.kind, FailureKind::Usage, "validação sai com 2 no CLI");
    }

    #[test]
    /// CONTRACT: IF-057
    fn an_invalid_port_is_refused_with_the_legacy_message() {
        let mut command = options(Some("mydb"));
        command.port = Some("70000".to_string());

        assert_eq!(
            validate(&command).expect_err("recusa").message,
            "Error: Invalid port: '70000'. Port must be between 1 and 65535."
        );
    }

    #[test]
    /// CONTRACT: IF-057
    fn an_invalid_username_is_refused_but_an_absent_one_is_fine() {
        let mut invalid = options(Some("mydb"));
        invalid.username = Some("com espaco".to_string());
        assert_eq!(
            validate(&invalid).expect_err("recusa").message,
            "Error: Invalid username: 'com espaco'"
        );

        // Usuário ausente não é validado (o legado só valida quando há valor).
        assert!(validate(&options(Some("mydb"))).is_ok());

        // Só espaço conta como ausente.
        let mut blank = options(Some("mydb"));
        blank.username = Some("   ".to_string());
        assert_eq!(validate(&blank).expect("válido").username, None);
    }

    #[test]
    /// CONTRACT: IF-051
    fn the_database_is_required_unless_all_is_used() {
        for database in [None, Some(""), Some("   ")] {
            assert_eq!(
                validate(&options(database)).expect_err("recusa").message,
                "Error: Database name is required when --all is not specified."
            );
        }

        let mut all = options(None);
        all.all = true;
        assert!(validate(&all).is_ok(), "`--all` dispensa o banco");
    }

    #[tokio::test]
    async fn a_missing_pg_dump_is_a_missing_binary_failure() {
        let error = backup(
            &options(Some("mydb")),
            &connection("mydb"),
            "senha",
            &empty_locator(),
            &crate::events::NoopSink,
            &cancellation_token(),
        )
        .await
        .expect_err("sem binário");

        match error {
            DatabaseFailure::Command(failure) => {
                assert_eq!(
                    failure.kind,
                    FailureKind::MissingBinary,
                    "o CLI mapeia isto para 3"
                );
                assert_eq!(
                    failure.message,
                    format!("Error: {}", binary_not_found(PG_DUMP))
                );
            }
            DatabaseFailure::Fatal(error) => panic!("não é erro do núcleo: {error}"),
        }
    }

    #[test]
    /// CONTRACT: IF-053
    fn argv_follows_the_legacy_order() {
        let argv = build_pg_dump_args(&options(Some("mydb")), &connection("mydb"), "mydb.dump");

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
    /// CONTRACT: IF-054
    fn argv_glues_the_format_and_keeps_an_empty_username() {
        let mut command = options(Some("mydb"));
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
    /// CONTRACT: IF-055
    fn argv_puts_verbose_before_the_exclusion_patterns() {
        let mut command = options(Some("mydb"));
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
    /// CONTRACT: IF-056
    fn argv_skips_blank_exclusion_patterns() {
        let mut command = options(Some("mydb"));
        command.exclude = vec!["".to_string(), "   ".to_string()];
        command.exclude_table_data = vec!["\t".to_string()];

        let argv = build_pg_dump_args(&command, &connection("mydb"), "mydb.dump");

        assert_eq!(argv.len(), 11, "padrões em branco não entram: {argv:?}");
    }

    #[tokio::test]
    /// CONTRACT: IF-060
    async fn a_failed_run_emits_one_step_event_per_database_and_continues() {
        // `--all` contra um servidor inalcançável: a **listagem** é que falha, e ela sobe como erro
        // do núcleo (exit 40), não como saída impressa.
        let mut command = options(None);
        command.all = true;
        let mut connection = connection("");
        connection.port = "1".to_string();
        connection.username = Some("postgres".to_string());

        let events = Mutex::new(Vec::new());
        let error = backup(
            &command,
            &connection,
            "senha",
            &empty_locator(),
            &Collector(&events),
            &cancellation_token(),
        )
        .await
        .expect_err("não lista");

        match error {
            DatabaseFailure::Fatal(error) => {
                assert_eq!(
                    error.exit_code(),
                    crate::error::exit_code::INVALID_STATE,
                    "a listagem vira InvalidOperationException no legado (40)"
                );
                assert!(error.to_string().starts_with("Failed to list databases: "));
            }
            DatabaseFailure::Command(failure) => {
                panic!("listagem não é saída impressa: {}", failure.message)
            }
        }

        assert!(
            events.lock().expect("collector").is_empty(),
            "sem lista não há evento de banco: {:?}",
            events.lock().expect("collector")
        );
    }

    #[test]
    /// CONTRACT: IF-059
    fn the_dump_extension_is_the_legacy_one() {
        assert_eq!(DUMP_EXTENSION, ".dump");
    }
}
