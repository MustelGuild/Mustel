use std::time::Duration;
use tokio_postgres::Config;
use rustls::ClientConfig;

use crate::config::models::ServerConfigEntry;
use crate::error::{MustelError, Result};
use crate::security::CredentialStore;

pub struct DbConnectionBuilder;

impl DbConnectionBuilder {
    /// Builds tokio_postgres::Config from ServerConfigEntry and target database override.
    pub fn build_config(
        server: &ServerConfigEntry,
        database_override: Option<&str>,
        password_override: Option<&str>,
    ) -> Result<Config> {
        let mut config = Config::new();
        config.host(&server.host);
        config.port(server.port);
        config.user(&server.username);

        // Determine database name
        let db_name = if let Some(db) = database_override {
            db.to_string()
        } else if let Some(dbs) = &server.databases {
            dbs.first().cloned().unwrap_or_else(|| "postgres".to_string())
        } else {
            "postgres".to_string()
        };
        config.dbname(&db_name);

        // Determine password
        if let Some(pass) = password_override {
            config.password(pass);
        } else if let Some(pass) = &server.password {
            config.password(pass);
        } else if let Some(enc_pass) = &server.encrypted_password {
            let decrypted = CredentialStore::decrypt_password(enc_pass)?;
            config.password(&decrypted);
        }

        // Connection timeout
        let timeout_secs = server.timeout.unwrap_or(30);
        config.connect_timeout(Duration::from_secs(timeout_secs));

        Ok(config)
    }

    /// Parses a complete Npgsql or LibPQ connection string into a tokio_postgres::Config.
    #[allow(dead_code)]
    pub fn parse_connection_string(conn_str: &str) -> Result<Config> {
        conn_str
            .parse::<Config>()
            .map_err(|e| MustelError::Database(format!("Invalid connection string: {}", e)))
    }

    /// Creates TLS connector for Rustls.
    pub fn create_tls_connector() -> Result<tokio_postgres_rustls::MakeRustlsConnect> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let tls_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(tokio_postgres_rustls::MakeRustlsConnect::new(tls_config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::ServerConfigEntry;
    use std::time::Duration;

    fn create_base_server() -> ServerConfigEntry {
        ServerConfigEntry {
            name: "test_server".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "test_user".to_string(),
            password: None,
            encrypted_password: None,
            databases: None,
            fetch_all_databases: None,
            exclude_patterns: None,
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            max_parallelism: None,
        }
    }

    #[test]
    fn test_build_config_database_resolution() {
        // Fallback to "postgres"
        let server = create_base_server();
        let config = DbConnectionBuilder::build_config(&server, None, None).unwrap();
        assert_eq!(config.get_dbname(), Some("postgres"));

        // Use database_override
        let config = DbConnectionBuilder::build_config(&server, Some("override_db"), None).unwrap();
        assert_eq!(config.get_dbname(), Some("override_db"));

        // Fallback to databases list
        let mut server_with_db = create_base_server();
        server_with_db.databases = Some(vec!["server_db".to_string()]);
        let config = DbConnectionBuilder::build_config(&server_with_db, None, None).unwrap();
        assert_eq!(config.get_dbname(), Some("server_db"));

        // database_override takes precedence over databases list
        let config = DbConnectionBuilder::build_config(&server_with_db, Some("override_db"), None).unwrap();
        assert_eq!(config.get_dbname(), Some("override_db"));
    }

    #[test]
    fn test_build_config_password_resolution() {
        // No password
        let server = create_base_server();
        let config = DbConnectionBuilder::build_config(&server, None, None).unwrap();
        assert_eq!(config.get_password(), None);

        // Password override
        let config = DbConnectionBuilder::build_config(&server, None, Some("override_pass")).unwrap();
        assert_eq!(config.get_password(), Some("override_pass".as_bytes()));

        // Server password
        let mut server_with_pass = create_base_server();
        server_with_pass.password = Some("server_pass".to_string());
        let config = DbConnectionBuilder::build_config(&server_with_pass, None, None).unwrap();
        assert_eq!(config.get_password(), Some("server_pass".as_bytes()));

        // Password override takes precedence over server password
        let config = DbConnectionBuilder::build_config(&server_with_pass, None, Some("override_pass")).unwrap();
        assert_eq!(config.get_password(), Some("override_pass".as_bytes()));

        // Server encrypted password
        let mut server_with_enc_pass = create_base_server();
        let enc_pass = crate::security::CredentialStore::encrypt_password("encrypted_pass").unwrap();
        server_with_enc_pass.encrypted_password = Some(enc_pass);
        let config = DbConnectionBuilder::build_config(&server_with_enc_pass, None, None).unwrap();
        assert_eq!(config.get_password(), Some("encrypted_pass".as_bytes()));
    }

    #[test]
    fn test_build_config_timeout() {
        // Default timeout is 30 seconds if None
        let server = create_base_server();
        let config = DbConnectionBuilder::build_config(&server, None, None).unwrap();
        assert_eq!(config.get_connect_timeout(), Some(&Duration::from_secs(30)));

        // Custom timeout
        let mut server_with_timeout = create_base_server();
        server_with_timeout.timeout = Some(60);
        let config = DbConnectionBuilder::build_config(&server_with_timeout, None, None).unwrap();
        assert_eq!(config.get_connect_timeout(), Some(&Duration::from_secs(60)));
    }

    #[test]
    fn test_build_config_basic() {
        let server = create_base_server();
        let config = DbConnectionBuilder::build_config(&server, None, None).unwrap();

        let hosts = config.get_hosts();
        assert_eq!(hosts.len(), 1);
        let tokio_postgres::config::Host::Tcp(host) = &hosts[0];
        assert_eq!(host, "localhost");

        assert_eq!(config.get_ports(), &[5432]);
        assert_eq!(config.get_user(), Some("test_user"));
    }
}
