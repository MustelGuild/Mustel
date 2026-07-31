use tokio_postgres::{Client, NoTls};
use glob::Pattern;

use crate::config::models::ServerConfigEntry;
use crate::database::connection::DbConnectionBuilder;
use crate::error::{MustelError, Result};

pub struct DbExecutor;

impl DbExecutor {
    /// Connects to a PostgreSQL server with retry logic.
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
                    // Spawn connection task in background
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            tracing::error!("PostgreSQL connection error: {}", e);
                        }
                    });
                    return Ok(client);
                }
                Err(e) => {
                    // Try fallback to NoTls if TLS fails or is not supported by server
                    if attempts == max_attempts {
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
