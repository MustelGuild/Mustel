use serde::{Deserialize, Serialize};

/// SSL connection modes for PostgreSQL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

impl std::str::FromStr for SslMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disable" => Ok(SslMode::Disable),
            "allow" => Ok(SslMode::Allow),
            "prefer" => Ok(SslMode::Prefer),
            "require" => Ok(SslMode::Require),
            "verifyca" | "verify-ca" => Ok(SslMode::VerifyCa),
            "verifyfull" | "verify-full" => Ok(SslMode::VerifyFull),
            _ => Err(format!("Unknown SSL mode: {}", s)),
        }
    }
}
impl std::fmt::Display for SslMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SslMode::Disable => write!(f, "Disable"),
            SslMode::Allow => write!(f, "Allow"),
            SslMode::Prefer => write!(f, "Prefer"),
            SslMode::Require => write!(f, "Require"),
            SslMode::VerifyCa => write!(f, "VerifyCa"),
            SslMode::VerifyFull => write!(f, "VerifyFull"),
        }
    }
}

/// Represents a single PostgreSQL server configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigEntry {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub databases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_all_databases: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub ssl_mode: Option<SslMode>,
    #[serde(default = "default_timeout")]
    pub timeout: Option<u64>,
    #[serde(default = "default_command_timeout")]
    pub command_timeout: Option<u64>,
    #[serde(default = "default_max_parallelism")]
    pub max_parallelism: Option<usize>,
}

fn default_port() -> u16 {
    5432
}

fn default_timeout() -> Option<u64> {
    Some(30)
}

fn default_command_timeout() -> Option<u64> {
    Some(300)
}

fn default_max_parallelism() -> Option<usize> {
    Some(4)
}

/// Represents default settings across the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDefaults {
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_output_directory")]
    pub output_directory: String,
    #[serde(default)]
    pub fetch_all_databases: bool,
    #[serde(default = "default_true")]
    pub require_confirmation: bool,
    #[serde(default = "default_max_parallelism_val")]
    pub max_parallelism: usize,
}

fn default_output_format() -> String {
    "csv".to_string()
}

fn default_output_directory() -> String {
    "./results".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_parallelism_val() -> usize {
    4
}

impl Default for UserDefaults {
    fn default() -> Self {
        Self {
            output_format: default_output_format(),
            output_directory: default_output_directory(),
            fetch_all_databases: false,
            require_confirmation: true,
            max_parallelism: 4,
        }
    }
}

/// Complete user configuration file root structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    #[serde(default)]
    pub servers: Vec<ServerConfigEntry>,
    #[serde(default)]
    pub defaults: UserDefaults,
}
