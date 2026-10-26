//! Árvore de comandos (clap) — a superfície N1 do contrato de paridade.
//!
//! Nomes de comandos, flags, curtos/longos, posicionais e defaults foram copiados das classes
//! `*Settings` do FurLab (`legacy/FurLab.CLI/Commands/`), que são o contrato observado. Onde a
//! documentação divergia do código (ex.: `database pgpass add <banco>` nos docs vs `--database` no
//! código), vale o código.

use clap::{Args, Parser, Subcommand};

/// Mustel — automação de tarefas de desenvolvimento (query, database, settings).
///
/// **Sem subcomando** o Mustel abre a GUI quando há terminal interativo (o caso de quem digita
/// `mustel` ou dá duplo-clique); fora dele continua imprimindo a ajuda com exit 0, que é o contrato
/// do oráculo — scripts e o harness de paridade nunca abrem janela por acidente.
#[derive(Debug, Parser)]
#[command(
    name = "mustel",
    version,
    about = "Mustel — automation CLI for PostgreSQL queries, database backups and server settings",
    long_about = None,
    propagate_version = true
)]
pub struct MustelCli {
    /// Comando escolhido pelo usuário (`None` = abrir a GUI, em terminal interativo).
    #[command(subcommand)]
    pub command: Option<TopCommand>,
}

/// Grupos de topo do Mustel.
#[derive(Debug, Subcommand)]
pub enum TopCommand {
    /// Execute SQL queries and export results to CSV.
    Query(QueryArgs),
    /// Database utilities.
    Database(DatabaseArgs),
    /// Manage Mustel settings and server configurations.
    Settings(SettingsArgs),
    /// Open the graphical interface.
    Gui(GuiArgs),
}

/// Argumentos de `mustel gui`.
#[derive(Debug, Args)]
pub struct GuiArgs {}

// ---------------------------------------------------------------------------------------------
// query
// ---------------------------------------------------------------------------------------------

/// Argumentos do grupo `query`.
#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Subcomando de `query`.
    #[command(subcommand)]
    pub command: QueryCommand,
}

/// Subcomandos de `query`.
#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    /// Execute SQL queries and export results to CSV.
    Run(QueryRunArgs),
}

/// Argumentos de `mustel query run`.
#[derive(Debug, Args)]
pub struct QueryRunArgs {
    /// Path to the SQL input file.
    #[arg(short = 'i', long = "input", value_name = "file")]
    pub input: Option<String>,

    /// Inline SQL query to execute (alternative to --input).
    #[arg(short = 'c', long = "command", value_name = "sql")]
    pub command: Option<String>,

    /// Path to the output directory.
    #[arg(short = 'o', long = "output", value_name = "path")]
    pub output: Option<String>,

    /// Complete Npgsql connection string. Bypasses server selection when provided.
    #[arg(long = "npgsql-connection-string", value_name = "string")]
    pub npgsql_connection_string: Option<String>,

    /// Database host address. Overrides the configured server host.
    #[arg(short = 'H', long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Database port. Overrides the configured server port.
    #[arg(short = 'p', long = "port", value_name = "port")]
    pub port: Option<String>,

    /// Database name. Overrides the configured server database.
    #[arg(short = 'd', long = "database", value_name = "database")]
    pub database: Option<String>,

    /// Database username. Overrides the configured server username.
    #[arg(short = 'U', long = "username", value_name = "username")]
    pub username: Option<String>,

    /// Database password. Overrides the stored server password.
    #[arg(short = 'W', long = "password", value_name = "password")]
    pub password: Option<String>,

    /// SSL mode (Disable, Allow, Prefer, Require, VerifyCA, VerifyFull). Default: Prefer.
    #[arg(long = "ssl-mode", value_name = "mode")]
    pub ssl_mode: Option<String>,

    /// Connection timeout in seconds. Default: 30.
    #[arg(long = "timeout", value_name = "seconds")]
    pub timeout: Option<i32>,

    /// Command timeout in seconds. Default: 300.
    #[arg(long = "command-timeout", value_name = "seconds")]
    pub command_timeout: Option<i32>,

    /// Enable connection pooling. Default: true.
    #[arg(
        long = "pooling",
        value_name = "bool",
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub pooling: Option<bool>,

    /// Minimum pool size. Default: 1.
    #[arg(long = "min-pool-size", value_name = "size")]
    pub min_pool_size: Option<i32>,

    /// Maximum pool size. Default: 100.
    #[arg(long = "max-pool-size", value_name = "size")]
    pub max_pool_size: Option<i32>,

    /// Keepalive interval in seconds. Default: 0.
    #[arg(long = "keepalive", value_name = "seconds")]
    pub keepalive: Option<i32>,

    /// Connection lifetime in seconds. Default: 0.
    #[arg(long = "connection-lifetime", value_name = "seconds")]
    pub connection_lifetime: Option<i32>,

    /// Execute the query on all databases on the server.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Comma-separated list of database names to exclude.
    #[arg(long = "exclude", value_name = "databases")]
    pub exclude: Option<String>,

    /// Skip confirmation prompt for destructive queries.
    #[arg(long = "no-confirm")]
    pub no_confirm: bool,

    /// Escape spreadsheet formulas in the CSV (`=`, `+`, `-`, `@`, TAB, CR).
    #[arg(long = "csv-escape-formulas")]
    pub csv_escape_formulas: bool,
}

// ---------------------------------------------------------------------------------------------
// database
// ---------------------------------------------------------------------------------------------

/// Argumentos do grupo `database`.
#[derive(Debug, Args)]
pub struct DatabaseArgs {
    /// Subcomando de `database`.
    #[command(subcommand)]
    pub command: DatabaseCommand,
}

/// Subcomandos de `database`.
#[derive(Debug, Subcommand)]
pub enum DatabaseCommand {
    /// Back up a PostgreSQL database (or all of them) using pg_dump.
    Backup(DatabaseBackupArgs),
    /// Restore a .dump file using pg_restore.
    Restore(DatabaseRestoreArgs),
    /// Manage the pgpass.conf file.
    Pgpass(PgpassArgs),
}

/// Argumentos de `mustel database backup`.
#[derive(Debug, Args)]
pub struct DatabaseBackupArgs {
    /// Name of the database to back up. Not required when using --all.
    #[arg(value_name = "database")]
    pub database: Option<String>,

    /// Database host address.
    #[arg(short = 'H', long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Database port.
    #[arg(short = 'p', long = "port", value_name = "port")]
    pub port: Option<String>,

    /// Database username.
    #[arg(short = 'U', long = "username", value_name = "username")]
    pub username: Option<String>,

    /// Database password.
    #[arg(short = 'W', long = "password", value_name = "password")]
    pub password: Option<String>,

    /// SSL mode for the database connection.
    ///
    /// Aceito por compatibilidade: o legado **não** usa este valor no `pg_dump`.
    #[arg(long = "ssl-mode", value_name = "mode")]
    pub ssl_mode: Option<String>,

    /// Timeout in seconds for the backup operation.
    ///
    /// Aceito por compatibilidade: o legado **não** usa este valor no `pg_dump`.
    #[arg(long = "timeout", value_name = "seconds")]
    pub timeout: Option<i32>,

    /// Command timeout in seconds for the backup operation.
    ///
    /// Aceito por compatibilidade: o legado **não** usa este valor no `pg_dump`.
    #[arg(long = "command-timeout", value_name = "seconds")]
    pub command_timeout: Option<i32>,

    /// Back up all databases on the server.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Fail with a non-zero exit code when any database fails (default: warn and exit 0).
    #[arg(long = "strict")]
    pub strict: bool,

    /// Table patterns to exclude entirely from the backup (repeat the flag for multiple values).
    #[arg(long = "exclude", value_name = "pattern")]
    pub exclude: Vec<String>,

    /// Table patterns whose data should be excluded (repeat the flag for multiple values).
    #[arg(long = "exclude-table-data", value_name = "pattern")]
    pub exclude_table_data: Vec<String>,

    /// Enable verbose output from pg_dump.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Output file or directory.
    #[arg(short = 'o', long = "output", value_name = "path")]
    pub output: Option<String>,

    /// Archive format passed to pg_dump (-F).
    #[arg(short = 'F', long = "format", value_name = "format")]
    pub format: Option<String>,
}

/// Argumentos de `mustel database restore`.
#[derive(Debug, Args)]
pub struct DatabaseRestoreArgs {
    /// Name of the database to restore. Not required when using --all.
    #[arg(value_name = "database")]
    pub database: Option<String>,

    /// Path to the dump file. Defaults to <database>.dump in the current directory.
    #[arg(value_name = "file")]
    pub input_file: Option<String>,

    /// Database host address.
    #[arg(short = 'H', long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Database port.
    #[arg(short = 'p', long = "port", value_name = "port")]
    pub port: Option<String>,

    /// Database username.
    #[arg(short = 'U', long = "username", value_name = "username")]
    pub username: Option<String>,

    /// Database password. Resolved from the keyring or prompted when omitted.
    #[arg(short = 'W', long = "password", value_name = "password")]
    pub password: Option<String>,

    /// SSL mode for the database connection.
    #[arg(long = "ssl-mode", value_name = "mode")]
    pub ssl_mode: Option<String>,

    /// Timeout in seconds for the restore operation.
    #[arg(long = "timeout", value_name = "seconds")]
    pub timeout: Option<i32>,

    /// Command timeout in seconds for the restore operation.
    #[arg(long = "command-timeout", value_name = "seconds")]
    pub command_timeout: Option<i32>,

    /// Restore all databases from dump files in the specified directory.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Directory containing .dump files (for --all). Defaults to the current directory.
    #[arg(short = 'd', long = "directory", value_name = "path")]
    pub directory: Option<String>,

    /// Enable verbose output from pg_restore.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Do not drop database objects before creating them (default uses --clean).
    #[arg(long = "no-clean")]
    pub no_clean: bool,

    /// Skip restoration of object ownership.
    #[arg(long = "no-owner")]
    pub no_owner: bool,

    /// Skip restoration of access privileges.
    #[arg(long = "no-acl")]
    pub no_acl: bool,

    /// Execute the restore as a single transaction.
    #[arg(long = "single-transaction")]
    pub single_transaction: bool,

    /// Number of parallel jobs for restore (custom-format only).
    #[arg(short = 'j', long = "jobs", value_name = "count")]
    pub jobs: Option<i32>,

    /// Create the target database when it does not exist.
    ///
    /// **Adição do Mustel**: o legado tentava criar o banco por `psql`, mas a checagem dele nunca
    /// dava certo (o `psql` sai com 0 existindo ou não o banco), então restaurar para um banco
    /// inexistente sempre falhou. Sem esta flag o comportamento continua o do legado (falha); com
    /// ela, o banco é criado pelo driver antes do `pg_restore`.
    #[arg(long = "create-if-not-exists")]
    pub create_if_not_exists: bool,
}

/// Argumentos do grupo `database pgpass`.
#[derive(Debug, Args)]
pub struct PgpassArgs {
    /// Subcomando de `pgpass`.
    #[command(subcommand)]
    pub command: PgpassCommand,
}

/// Subcomandos de `database pgpass`.
#[derive(Debug, Subcommand)]
pub enum PgpassCommand {
    /// Add an entry to pgpass.conf.
    Add(PgpassAddArgs),
    /// List pgpass.conf entries.
    List,
    /// Remove an entry from pgpass.conf.
    Remove(PgpassRemoveArgs),
}

/// Argumentos de `mustel database pgpass add`.
///
/// O banco pode vir posicionalmente (`pgpass add mydb`, forma documentada) ou por`--database`
/// (forma herdada); as duas juntas são ambíguas e o parser recusa. A resolução de qual forma veio é
/// feita pelo comando (T23).
#[derive(Debug, Args)]
pub struct PgpassAddArgs {
    /// Database name (positional form: `mustel database pgpass add mydb`).
    #[arg(value_name = "database", conflicts_with = "database")]
    pub database_arg: Option<String>,

    /// Database name (option form, kept for compatibility with the legacy CLI).
    #[arg(long = "database", value_name = "database")]
    pub database: Option<String>,

    /// Host name (use * as wildcard).
    #[arg(long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Port number (use * as wildcard).
    #[arg(long = "port", value_name = "port")]
    pub port: Option<String>,

    /// User name (use * as wildcard).
    #[arg(long = "username", value_name = "username")]
    pub username: Option<String>,

    /// Password to store for this entry.
    #[arg(long = "password", value_name = "password")]
    pub password: Option<String>,
}

/// Argumentos de `mustel database pgpass remove`.
#[derive(Debug, Args)]
pub struct PgpassRemoveArgs {
    /// Database name of the entry to remove.
    #[arg(value_name = "database")]
    pub database: String,

    /// Host name of the entry to remove.
    #[arg(long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Port number of the entry to remove.
    #[arg(long = "port", value_name = "port")]
    pub port: Option<String>,

    /// User name of the entry to remove.
    #[arg(long = "username", value_name = "username")]
    pub username: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------------------------

/// Argumentos do grupo `settings`.
#[derive(Debug, Args)]
pub struct SettingsArgs {
    /// Subcomando de `settings`.
    #[command(subcommand)]
    pub command: SettingsCommand,
}

/// Subcomandos de `settings`.
#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Manage configured database servers.
    DbServers(DbServersArgs),
}

/// Argumentos do grupo `settings db-servers`.
#[derive(Debug, Args)]
pub struct DbServersArgs {
    /// Subcomando de `db-servers`.
    #[command(subcommand)]
    pub command: DbServersCommand,
}

/// Subcomandos de `settings db-servers`.
#[derive(Debug, Subcommand)]
pub enum DbServersCommand {
    /// List configured servers.
    Ls,
    /// Add or update a server.
    Add(DbServersAddArgs),
    /// Remove a server.
    Rm(DbServersRemoveArgs),
    /// Test the connection to a configured server.
    Test(DbServersTestArgs),
    /// Store a server password in the system keyring.
    SetPassword(DbServersSetPasswordArgs),
}

/// Argumentos de `mustel settings db-servers add`.
#[derive(Debug, Args)]
pub struct DbServersAddArgs {
    /// Unique server name.
    #[arg(short = 'n', long = "name", value_name = "name")]
    pub name: Option<String>,

    /// Ask for the missing values interactively.
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Database host address.
    #[arg(short = 'H', long = "host", value_name = "host")]
    pub host: Option<String>,

    /// Database port.
    #[arg(
        short = 'p',
        long = "port",
        value_name = "port",
        default_value_t = 5432
    )]
    pub port: i32,

    /// Database username.
    #[arg(short = 'U', long = "username", value_name = "username")]
    pub username: Option<String>,

    /// Database password, stored in the system keyring.
    #[arg(short = 'W', long = "password", value_name = "password")]
    pub password: Option<String>,

    /// Comma-separated list of databases.
    #[arg(short = 'd', long = "databases", value_name = "databases")]
    pub databases: Option<String>,

    /// SSL mode (Disable, Allow, Prefer, Require, VerifyCA, VerifyFull).
    #[arg(long = "ssl-mode", value_name = "mode")]
    pub ssl_mode: Option<String>,

    /// Connection timeout in seconds.
    #[arg(long = "timeout", value_name = "seconds")]
    pub timeout: Option<i32>,

    /// Maximum degree of parallelism for this server.
    #[arg(long = "max-parallelism", value_name = "count")]
    pub max_parallelism: Option<i32>,

    /// Discover all databases on the server automatically.
    #[arg(long = "fetch-all")]
    pub fetch_all_databases: bool,

    /// Comma-separated patterns excluded during discovery.
    #[arg(long = "exclude-patterns", value_name = "patterns")]
    pub exclude_patterns: Option<String>,
}

/// Argumentos de `mustel settings db-servers rm`.
#[derive(Debug, Args)]
pub struct DbServersRemoveArgs {
    /// Name of the server to remove.
    #[arg(short = 'n', long = "name", value_name = "name")]
    pub name: Option<String>,
}

/// Argumentos de `mustel settings db-servers test`.
#[derive(Debug, Args)]
pub struct DbServersTestArgs {
    /// Name of the server to test.
    #[arg(short = 'n', long = "name", value_name = "name")]
    pub name: Option<String>,
}

/// Argumentos de `mustel settings db-servers set-password`.
#[derive(Debug, Args)]
pub struct DbServersSetPasswordArgs {
    /// Name of the server whose password will be stored.
    #[arg(value_name = "name")]
    pub name: Option<String>,

    /// Password to store, instead of prompting (Mustel addition, like `add -W`).
    #[arg(short = 'W', long = "password", value_name = "password")]
    pub password: Option<String>,
}
