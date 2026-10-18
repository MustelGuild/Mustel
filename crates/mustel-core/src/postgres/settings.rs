//! Configuração de conexão: do cadastro/flags para o que o driver precisa.
//!
//! Reúne os parâmetros que o legado expõe em `query run` (`--pooling`, `--min-pool-size`,
//! `--max-pool-size`, `--keepalive`, `--connection-lifetime`, `--timeout`, `--command-timeout`,
//! `--ssl-mode`) e no cadastro do servidor, com os **defaults do legado**.

use std::str::FromStr;
use std::time::Duration;

use crate::postgres::tls::{SslMode, SslModeError};

/// Defaults herdados do FurLab.
pub const DEFAULT_PORT: u16 = 5432;
/// Default herdado do FurLab (`--timeout`).
pub const DEFAULT_CONNECT_TIMEOUT_SECONDS: u32 = 30;
/// Default herdado do FurLab (`--command-timeout`).
pub const DEFAULT_COMMAND_TIMEOUT_SECONDS: u32 = 300;
/// Default herdado do FurLab (`--min-pool-size`).
pub const DEFAULT_MIN_POOL_SIZE: u32 = 1;
/// Default herdado do FurLab (`--max-pool-size`).
pub const DEFAULT_MAX_POOL_SIZE: u32 = 100;

/// O que é preciso para abrir uma conexão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    /// Host do servidor.
    pub host: String,
    /// Porta do servidor.
    pub port: u16,
    /// Usuário.
    pub user: String,
    /// Senha (resolvida da flag ou do cofre, nunca do arquivo).
    pub password: Option<String>,
    /// Banco a abrir.
    pub database: Option<String>,
    /// Modo de SSL.
    pub ssl_mode: SslMode,
    /// Timeout de conexão.
    pub connect_timeout: Duration,
    /// Timeout de comando.
    pub command_timeout: Duration,
    /// Habilita pool de conexões.
    pub pooling: bool,
    /// Tamanho mínimo do pool.
    pub min_pool_size: u32,
    /// Tamanho máximo do pool.
    pub max_pool_size: u32,
    /// Keepalive (0 = desligado, como no legado).
    pub keepalive: Duration,
    /// Tempo de vida máximo de uma conexão (0 = ilimitado, como no legado).
    pub connection_lifetime: Duration,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_PORT,
            user: String::new(),
            password: None,
            database: None,
            ssl_mode: SslMode::default(),
            connect_timeout: Duration::from_secs(u64::from(DEFAULT_CONNECT_TIMEOUT_SECONDS)),
            command_timeout: Duration::from_secs(u64::from(DEFAULT_COMMAND_TIMEOUT_SECONDS)),
            pooling: true,
            min_pool_size: DEFAULT_MIN_POOL_SIZE,
            max_pool_size: DEFAULT_MAX_POOL_SIZE,
            keepalive: Duration::ZERO,
            connection_lifetime: Duration::ZERO,
        }
    }
}

impl ConnectionSettings {
    /// Aplica um texto de `sslMode` vindo do cadastro ou da flag.
    pub fn with_ssl_mode_text(mut self, text: &str) -> Result<Self, SslModeError> {
        self.ssl_mode = SslMode::from_str(text)?;
        Ok(self)
    }

    /// Traduz para a configuração do `tokio-postgres`.
    ///
    /// A senha vai junto porque o driver a exige na configuração; ela **nunca** é registrada em log
    /// nem em mensagem de erro (D6).
    pub fn to_pg_config(&self) -> tokio_postgres::Config {
        let mut config = tokio_postgres::Config::new();
        config.host(&self.host);
        config.port(self.port);
        config.user(&self.user);
        config.connect_timeout(self.connect_timeout);
        if let Some(password) = &self.password {
            config.password(password);
            config.application_name("mustel");
        }
        if let Some(database) = &self.database {
            config.dbname(database);
        }
        config
    }

    /// Endereço legível `host:porta`, usado nas mensagens (mesmo formato do legado).
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_legacy() {
        let settings = ConnectionSettings::default();
        assert_eq!(settings.port, 5432);
        assert_eq!(settings.connect_timeout.as_secs(), 30);
        assert_eq!(settings.command_timeout.as_secs(), 300);
        assert_eq!(settings.min_pool_size, 1);
        assert_eq!(settings.max_pool_size, 100);
        assert!(settings.pooling);
        assert_eq!(settings.ssl_mode, SslMode::Prefer);
        assert_eq!(settings.keepalive, Duration::ZERO);
        assert_eq!(settings.connection_lifetime, Duration::ZERO);
    }

    #[test]
    fn ssl_mode_text_is_parsed_into_the_settings() {
        let settings = ConnectionSettings::default()
            .with_ssl_mode_text("Require")
            .expect("valid mode");
        assert_eq!(settings.ssl_mode, SslMode::Require);

        assert!(
            ConnectionSettings::default()
                .with_ssl_mode_text("nope")
                .is_err()
        );
    }

    #[test]
    fn endpoint_reads_like_the_legacy_message() {
        let settings = ConnectionSettings {
            host: "localhost".to_string(),
            port: 5432,
            ..ConnectionSettings::default()
        };
        assert_eq!(settings.endpoint(), "localhost:5432");
    }

    #[test]
    fn driver_config_carries_host_port_and_user() {
        let settings = ConnectionSettings {
            host: "db.local".to_string(),
            port: 5433,
            user: "postgres".to_string(),
            password: Some("segredo".to_string()),
            database: Some("mydb".to_string()),
            ..ConnectionSettings::default()
        };

        let config = settings.to_pg_config();
        assert_eq!(config.get_hosts().len(), 1);
        assert_eq!(config.get_ports(), vec![5433]);
        assert_eq!(config.get_user(), Some("postgres"));
        assert_eq!(config.get_password(), Some(b"segredo".as_slice()));
        assert_eq!(config.get_dbname(), Some("mydb"));
    }
}
