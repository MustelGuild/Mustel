use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use colored::Colorize;
use tokio::sync::Semaphore;

use crate::cli::commands::query::analyzer::{QueryType, SqlQueryAnalyzer};
use crate::cli::commands::query::exporter::{CsvExporter, ExecutionLogEntry};
use crate::cli::commands::query::settings::QueryRunArgs;
use crate::cli::ui::{ProgressTracker, UiPrompts};
use crate::config::models::ServerConfigEntry;
use crate::config::UserConfigService;
use crate::database::{DbExecutor, SecurityUtils};
use crate::error::{MustelError, Result};

pub struct QueryRunner {
    config_service: UserConfigService,
}

impl QueryRunner {
    pub fn new(config_service: UserConfigService) -> Self {
        Self { config_service }
    }

    pub async fn run(&self, args: QueryRunArgs) -> Result<()> {
        // Validate mutually exclusive flags -c and -i
        if args.command.is_some() && args.input.is_some() {
            return Err(MustelError::Query(
                "Options -c/--command and -i/--input are mutually exclusive. Use only one.".into(),
            ));
        }

        // Load SQL query content
        let (sql_query, query_source) = if let Some(cmd) = &args.command {
            (cmd.clone(), "inline query".to_string())
        } else if let Some(inp) = &args.input {
            SecurityUtils::validate_path(inp)?;
            let input_path = PathBuf::from(inp);
            if !input_path.exists() {
                return Err(MustelError::Query(format!(
                    "SQL input file not found: {:?}",
                    input_path
                )));
            }
            let content = fs::read_to_string(&input_path)?;
            (content, input_path.to_string_lossy().to_string())
        } else {
            return Err(MustelError::Query(
                "Must specify either an inline query (-c) or a SQL file (-i).".into(),
            ));
        };

        if sql_query.trim().is_empty() {
            return Err(MustelError::Query("SQL query cannot be empty.".into()));
        }

        // Destructive Query Detection
        match SqlQueryAnalyzer::analyze(&sql_query) {
            QueryType::Destructive(op_type) => {
                if !args.no_confirm {
                    let confirmed = UiPrompts::confirm_destructive_query(&op_type, &query_source)?;
                    if !confirmed {
                        println!("{}", "Execution cancelled by user.".yellow());
                        return Ok(());
                    }
                }
            }
            QueryType::ReadOnly => {}
        }

        // Resolve Output Directory
        let defaults = self.config_service.get_defaults()?;
        let output_dir_str = args.output.clone().unwrap_or(defaults.output_directory);
        let output_dir = PathBuf::from(&output_dir_str);

        // Target Resolution
        let targets = self.resolve_targets(&args).await?;

        println!(
            "{}",
            format!(
                "🚀 Running query on {} target database(s)...",
                targets.len()
            )
            .bright_cyan()
            .bold()
        );

        let progress = ProgressTracker::new();
        let max_parallelism = defaults.max_parallelism;
        let semaphore = Arc::new(Semaphore::new(max_parallelism));

        let mut tasks = Vec::new();

        for target in targets {
            let sem = Arc::clone(&semaphore);
            let sql = sql_query.clone();
            let src = query_source.clone();
            let out_dir = output_dir.clone();

            let spinner = progress.create_spinner(&format!(
                "Executing on {}/{}...",
                target.server.name, target.database
            ));

            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let start_time = Instant::now();

                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let filename = format!("{}_{}_{}.csv", target.server.name, target.database, timestamp);
                let output_file_path = out_dir.join(&target.server.name).join(&filename);

                match DbExecutor::connect(&target.server, Some(&target.database), target.password.as_deref()).await {
                    Ok(client) => match client.query(&sql, &[]).await {
                        Ok(rows) => {
                            match CsvExporter::export_rows(&rows, &output_file_path) {
                                Ok(row_count) => {
                                    let duration = start_time.elapsed().as_millis();
                                    spinner.finish_with_message(format!(
                                        "{} {}/{} ({} rows in {}ms) -> {:?}",
                                        "✔".green(),
                                        target.server.name,
                                        target.database,
                                        row_count,
                                        duration,
                                        output_file_path
                                    ));

                                    let log_entry = ExecutionLogEntry {
                                        timestamp: chrono::Local::now().to_rfc3339(),
                                        server_name: target.server.name.clone(),
                                        database_name: target.database.clone(),
                                        query_source: src,
                                        rows_affected: row_count,
                                        duration_ms: duration,
                                        output_file: output_file_path.to_string_lossy().to_string(),
                                        status: "SUCCESS".into(),
                                        error_message: None,
                                    };
                                    let _ = CsvExporter::log_execution(&out_dir, &log_entry);
                                }
                                Err(e) => {
                                    spinner.finish_with_message(format!(
                                        "{} {}/{} export error: {}",
                                        "✖".red(),
                                        target.server.name,
                                        target.database,
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            spinner.finish_with_message(format!(
                                "{} {}/{} query error: {}",
                                "✖".red(),
                                target.server.name,
                                target.database,
                                e
                            ));
                        }
                    },
                    Err(e) => {
                        spinner.finish_with_message(format!(
                            "{} Connection failed to {}/{}: {}",
                            "✖".red(),
                            target.server.name,
                            target.database,
                            e
                        ));
                    }
                }
            }));
        }

        for t in tasks {
            let _ = t.await;
        }

        println!("\n{}", "✨ Query execution completed!".bright_green().bold());
        Ok(())
    }

    async fn resolve_targets(&self, args: &QueryRunArgs) -> Result<Vec<TargetDb>> {
        let mut targets = Vec::new();

        // 1. Direct Connection String Mode
        if let Some(_conn_str) = &args.connection_string {
            let server = ServerConfigEntry {
                name: "direct".into(),
                host: "direct".into(),
                port: 5432,
                username: "postgres".into(),
                password: args.password.clone(),
                encrypted_password: None,
                databases: args.database.clone().map(|d| vec![d]),
                fetch_all_databases: None,
                exclude_patterns: None,
                ssl_mode: None,
                timeout: args.timeout,
                command_timeout: args.command_timeout,
                max_parallelism: None,
            };
            let db_name = args.database.clone().unwrap_or_else(|| "postgres".to_string());
            targets.push(TargetDb {
                server,
                database: db_name,
                password: args.password.clone(),
            });
            return Ok(targets);
        }

        // 2. Server Configuration Mode
        let configured_servers = self.config_service.get_servers()?;

        let selected_servers = if let Some(srv_names) = &args.servers {
            let names: Vec<&str> = srv_names.split(',').map(|s| s.trim()).collect();
            let mut matched = Vec::new();
            for n in names {
                if let Some(s) = configured_servers.iter().find(|s| s.name.eq_ignore_ascii_case(n)) {
                    matched.push(s.clone());
                } else {
                    return Err(MustelError::Config(format!("Server '{}' not found in configuration.", n)));
                }
            }
            matched
        } else if let Some(host) = &args.host {
            // Ad-hoc host flag
            vec![ServerConfigEntry {
                name: host.clone(),
                host: host.clone(),
                port: args.port.unwrap_or(5432),
                username: args.username.clone().unwrap_or_else(|| "postgres".into()),
                password: args.password.clone(),
                encrypted_password: None,
                databases: args.database.clone().map(|d| vec![d]),
                fetch_all_databases: None,
                exclude_patterns: None,
                ssl_mode: None,
                timeout: args.timeout,
                command_timeout: args.command_timeout,
                max_parallelism: None,
            }]
        } else {
            // Interactive Selection
            UiPrompts::select_servers(&configured_servers)?
        };

        let extra_excludes: Option<Vec<String>> = args.exclude.as_ref().map(|ex| {
            ex.split(',').map(|s| s.trim().to_string()).collect()
        });

        for srv in selected_servers {
            let fetch_all = args.all || srv.fetch_all_databases.unwrap_or(false);

            let dbs = if fetch_all {
                DbExecutor::fetch_active_databases(&srv, args.password.as_deref(), extra_excludes.as_deref()).await?
            } else if let Some(db_override) = &args.database {
                vec![db_override.clone()]
            } else if let Some(srv_dbs) = &srv.databases {
                srv_dbs.clone()
            } else {
                vec!["postgres".to_string()]
            };

            for db in dbs {
                targets.push(TargetDb {
                    server: srv.clone(),
                    database: db,
                    password: args.password.clone(),
                });
            }
        }

        Ok(targets)
    }
}

struct TargetDb {
    server: ServerConfigEntry,
    database: String,
    password: Option<String>,
}
