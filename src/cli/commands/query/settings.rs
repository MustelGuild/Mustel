use clap::Args;

/// Settings and CLI options for `mustel query run`.
#[derive(Args, Debug, Clone)]
pub struct QueryRunArgs {
    /// Path to SQL input file.
    #[arg(short = 'i', long = "input")]
    pub input: Option<String>,

    /// Inline SQL query string to execute (alternative to --input).
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,

    /// Path to output directory for CSV results.
    #[arg(short = 'o', long = "output")]
    pub output: Option<String>,

    /// Direct PostgreSQL connection string. Bypasses server selection.
    #[arg(long = "connection-string")]
    pub connection_string: Option<String>,

    /// Target server names (comma-separated).
    #[arg(long = "servers")]
    pub servers: Option<String>,

    /// Database host address override.
    #[arg(short = 'H', long = "host")]
    pub host: Option<String>,

    /// Database port override.
    #[arg(short = 'p', long = "port")]
    pub port: Option<u16>,

    /// Target database name override.
    #[arg(short = 'd', long = "database")]
    pub database: Option<String>,

    /// Database username override.
    #[arg(short = 'U', long = "username")]
    pub username: Option<String>,

    /// Database password override.
    #[arg(short = 'W', long = "password")]
    pub password: Option<String>,

    /// SSL mode override (Disable, Allow, Prefer, Require, VerifyCa, VerifyFull).
    #[arg(long = "ssl-mode")]
    pub ssl_mode: Option<String>,

    /// Connection timeout in seconds.
    #[arg(long = "timeout")]
    pub timeout: Option<u64>,

    /// Command execution timeout in seconds.
    #[arg(long = "command-timeout")]
    pub command_timeout: Option<u64>,

    /// Execute query on all databases on the target server(s).
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Comma-separated list of database names or patterns to exclude.
    #[arg(long = "exclude")]
    pub exclude: Option<String>,

    /// Skip confirmation prompt for destructive queries (DELETE, DROP, TRUNCATE, etc.).
    #[arg(long = "no-confirm")]
    pub no_confirm: bool,
}
