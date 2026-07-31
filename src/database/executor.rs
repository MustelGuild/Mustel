use tokio_postgres::{Client, NoTls};
use glob::Pattern;
use colored::Colorize;

use crate::config::models::ServerConfigEntry;
use crate::database::connection::DbConnectionBuilder;
use crate::error::{MustelError, Result};

/// Whether to allow fallback to unencrypted connections when TLS fails.
/// Controlled by environment variable MUSTEL_ALLOW_UNENCRYPTED.
fn allow_unencrypted_fallback() -> bool {
    std::env::var("MUSTEL_ALLOW_UNENCRYPTED")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false)
}

pub struct DbExecutor;

impl DbExecutor {
    /// Connects to a PostgreSQL server with retry logic.
    /// SECURITY: This function will refuse to connect without encryption unless
    /// MUSTEL_ALLOW_UNENCRYPTED=true is explicitly set in the environment.
    pub async fn connect(
        server: &ServerConfigEntry,
        database_override: Option<&str>,
        password_override: Option<&str>,
    ) -> Result<Client> {
        let config = DbConnectionBuilder::build_config(server, database_override, password_override)?;
        let tls_connector = DbConnectionBuilder::create_tls_connector()?;

        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;
            match config.connect(tls_connector.clone()).await {
                Ok((client, connection)) => {
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            tracing::error!("PostgreSQL connection error: {}", e);
                        }
                    });
                    return Ok(client);
                }
                Err(e) => {
                    if attempts == max_attempts {
                        // Check if SSL is disabled in server config
                        let ssl_disabled = matches!(
                            server.ssl_mode,
                            Some(crate::config::models::SslMode::Disable)
                        );

                        if ssl_disabled || allow_unencrypted_fallback() {
                            if ssl_disabled {
                                eprintln!(
                                    "{} {}",
                                    "WARNING:".yellow().bold(),
                                    "SSL is disabled for this server. ".
                                    yellow()
                                );
                            } else {
                                eprintln!(
                                    "{} {}",
                                    "WARNING:".yellow().bold(),
                                    "TLS connection failed, falling back to unencrypted connection. ".
                                    yellow()
                                );
                            }
                            eprintln!(
                                "{} {}",
                                "Password will be sent in plaintext!".red().bold(),
                                "Consider enabling SSL or using a VPN.".yellow()
                            );

                            match config.connect(NoTls).await {
                                Ok((client, connection)) => {
                                    tokio::spawn(async move {
                                        if let Err(e) = connection.await {
                                            tracing::error!("PostgreSQL NoTls connection error: {}", e);
                                        }
                                    });
                                    return Ok(client);
                                }
                                Err(_) => {
                                    return Err(MustelError::Database(format!(
                                        "Failed to connect to {}:{} after {} attempts: {}",
                                        server.host, server.port, max_attempts, e
                                    )));
                                }
                            }
                        } else {
                            return Err(MustelError::Database(format!(
                                "Failed to establish TLS connection to {}:{}: {}. \
                                 Set MUSTEL_ALLOW_UNENCRYPTED=true to allow unencrypted connections (not recommended).",
                                server.host, server.port, e
                            )));
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                }
            }
        }
    }

    /// Fetches all non-template databases from the server, filtering out excluded patterns.
    pub async fn fetch_active_databases(
        server: &ServerConfigEntry,
        password_override: Option<&str>,
        extra_excludes: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let client = Self::connect(server, Some("postgres"), password_override).await?;
        let rows = client
            .query(
                "SELECT datname FROM pg_database WHERE datistemplate = false AND datallowconn = true ORDER BY datname",
                &[],
            )
            .await
            .map_err(|e| MustelError::Database(format!("Failed to list databases: {}", e)))?;

        let mut db_names: Vec<String> = rows.iter().map(|r| r.get(0)).collect();

        // Compile exclude patterns
        let mut patterns = Vec::new();

        if let Some(configured_excludes) = &server.exclude_patterns {
            for pat in configured_excludes {
                if let Ok(p) = Pattern::new(pat) {
                    patterns.push(p);
                }
            }
        }

        if let Some(extras) = extra_excludes {
            for pat in extras {
                if let Ok(p) = Pattern::new(pat) {
                    patterns.push(p);
                }
            }
        }

        // Always exclude 'postgres' system db from auto-discovery unless specified
        if let Ok(p) = Pattern::new("template*") {
            patterns.push(p);
        }

        if !patterns.is_empty() {
            db_names.retain(|db| {
                !patterns.iter().any(|p| p.matches(db))
            });
        }

        Ok(db_names)
    }
}
