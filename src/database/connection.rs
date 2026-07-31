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
