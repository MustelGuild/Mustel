pub mod commands;
pub mod ui;

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use commands::query::{QueryRunArgs, QueryRunner};
use commands::settings::{DbServersHandler, DbServersSubcommand};
use crate::config::UserConfigService;
use crate::error::Result;

/// Mustel - Personal CLI toolkit for database management, query execution, and automation.
#[derive(Parser, Debug)]
#[command(name = "mustel", author, version, about, long_about = None)]
pub struct CliApp {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Execute SQL queries and export results to CSV.
    Query {
        #[command(subcommand)]
        subcommand: Option<QuerySubcommands>,
    },

    /// Manage Mustel settings and server configurations.
    Settings {
        #[command(subcommand)]
        subcommand: Option<SettingsSubcommands>,
    },
}

#[derive(Subcommand, Debug)]
pub enum QuerySubcommands {
    /// Execute a SQL query across target servers/databases and export to CSV.
    Run(QueryRunArgs),
}

#[derive(Subcommand, Debug)]
pub enum SettingsSubcommands {
    /// Manage configured database servers.
    #[command(name = "db-servers", alias = "servers")]
    DbServers {
        #[command(subcommand)]
        subcommand: Option<DbServersSubcommand>,
    },
}

impl CliApp {
    pub async fn dispatch(self) -> Result<()> {
        let config_service = UserConfigService::new();

        let command = match self.command {
            Some(cmd) => cmd,
            None => {
                let mut cmd = CliApp::command();
                cmd.print_help()?;
                println!();
                return Ok(());
            }
        };

        match command {
            Commands::Query { subcommand } => match subcommand {
                Some(QuerySubcommands::Run(run_args)) => {
                    let runner = QueryRunner::new(config_service);
                    runner.run(run_args).await?;
                }
                None => {
                    println!("\n{}", "Available 'query' subcommands:".bright_cyan().bold());
                    println!("  {:<15} Execute a SQL query across target servers/databases and export to CSV", "run".bold());
                    println!("\nUsage: mustel query run [OPTIONS]\n");
                }
            },
            Commands::Settings { subcommand } => match subcommand {
                Some(SettingsSubcommands::DbServers { subcommand }) => {
                    let handler = DbServersHandler::new(config_service);
                    handler.handle(subcommand).await?;
                }
                None => {
                    println!("\n{}", "Available 'settings' subcommands:".bright_cyan().bold());
                    println!("  {:<20} Manage configured database servers", "db-servers (servers)".bold());
                    println!("\nUsage: mustel settings db-servers <COMMAND>\n");
                }
            },
        }

        Ok(())
    }
}
