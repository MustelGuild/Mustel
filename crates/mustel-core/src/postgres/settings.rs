//! Configuração de conexão: do cadastro/flags para o que o driver precisa.
//!
//! Reúne os parâmetros que o legado expõe em `query run` (`--pooling`, `--min-pool-size`,
//! `--max-pool-size`, `--keepalive`, `--connection-lifetime`, `--timeout`, `--command-timeout`,
//! `--ssl-mode`) e no cadastro do servidor, com os **defaults do legado**.

use std::fmt;
use std::time::Duration;

use crate::postgres::tls::SslMode;

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
#[derive(Clone, PartialEq, Eq)]
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
    /// Habilita pool de conexões. **Aceito por paridade; sem efeito** (não há pool).
    pub pooling: bool,
    /// Tamanho mínimo do pool. **Aceito por paridade; sem efeito** (não há pool).
    pub min_pool_size: u32,
    /// Tamanho máximo do pool. **Aceito por paridade; sem efeito** (não há pool).
    pub max_pool_size: u32,
    /// Keepalive (0 = desligado, como no legado).
    pub keepalive: Duration,
    /// Tempo de vida máximo de uma conexão (0 = ilimitado). **Aceito por paridade; sem efeito.**
    pub connection_lifetime: Duration,
}

impl fmt::Debug for ConnectionSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "<oculta>"))
            .field("database", &self.database)
            .field("ssl_mode", &self.ssl_mode)
            .field("connect_timeout", &self.connect_timeout)
            .field("command_timeout", &self.command_timeout)
            .field("pooling", &self.pooling)
            .field("min_pool_size", &self.min_pool_size)
            .field("max_pool_size", &self.max_pool_size)
            .field("keepalive", &self.keepalive)
            .field("connection_lifetime", &self.connection_lifetime)
            .finish()
    }
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
    /// Traduz para a configuração do `tokio-postgres`.
    ///
    /// A senha vai junto porque o driver a exige na configuração; ela **nunca** é registrada em log
    /// nem em mensagem de erro (D6).
    ///
    /// Das opções de pool do legado, **`--keepalive` tem efeito real**: é o keepalive de TCP do
    /// Npgsql, e vai para o socket da conexão. As outras (`--pooling`, `--min-pool-size`,
    /// `--max-pool-size`, `--connection-lifetime`) ficam guardadas em [`ConnectionSettings`] porque
    /// são parte da superfície do comando, mas **não** mudam nada: o Mustel abre uma conexão por
    /// banco e a fecha no fim, então não há pool para configurar (o `tokio-postgres` não tem um — ver
    /// `docs/rust-postgres-spike.md`).
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
        if self.keepalive.is_zero() {
            // O `KeepAlive = 0` do Npgsql (o default) deixa o default do SO, que é desligado — e o
            // driver, por conta própria, ligaria o keepalive com 2 horas de ociosidade.
            config.keepalives(false);
        } else {
            config.keepalives(true);
            config.keepalives_idle(self.keepalive);
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
    fn debug_never_shows_the_password() {
        let settings = ConnectionSettings {
            password: Some("s3cret".to_string()),
            ..ConnectionSettings::default()
        };

        let text = format!("{settings:?}");
        assert!(text.contains("<oculta>"), "{text}");
        assert!(!text.contains("s3cret"), "{text}");
    }

    #[test]
    /// CONTRACT: IF-024
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
    fn endpoint_reads_like_the_legacy_message() {
        let settings = ConnectionSettings {
            host: "localhost".to_string(),
            port: 5432,
            ..ConnectionSettings::default()
        };
        assert_eq!(settings.endpoint(), "localhost:5432");
    }

    #[test]
    fn the_keepalive_goes_to_the_socket_and_zero_leaves_the_default() {
        let with_keepalive = ConnectionSettings {
            keepalive: std::time::Duration::from_secs(30),
            ..ConnectionSettings::default()
        }
        .to_pg_config();
        assert!(with_keepalive.get_keepalives());
        assert_eq!(
            with_keepalive.get_keepalives_idle(),
            std::time::Duration::from_secs(30)
        );

        let without = ConnectionSettings::default().to_pg_config();
        assert!(
            !without.get_keepalives(),
            "sem `--keepalive` o socket fica como o Npgsql deixa"
        );
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
