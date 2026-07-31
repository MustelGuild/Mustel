use clap::Subcommand;
use colored::Colorize;

use crate::config::models::ServerConfigEntry;
use crate::config::UserConfigService;
use crate::database::DbExecutor;
use crate::error::{MustelError, Result};
use crate::security::CredentialStore;

#[derive(Subcommand, Debug, Clone)]
pub enum DbServersSubcommand {
    /// List all configured database servers.
    #[command(alias = "ls")]
    List,

    /// Add or update a database server configuration.
    Add {
        /// Server configuration name.
        name: String,
        /// Host address.
        #[arg(long)]
        host: String,
        /// Port number.
        #[arg(long, default_value_t = 5432)]
        port: u16,
        /// Username.
        #[arg(long)]
        username: String,
        /// Target database name.
        #[arg(long)]
        database: Option<String>,
        /// Plaintext password (optional, set-password recommended).
        #[arg(long)]
        password: Option<String>,
    },

    /// Remove a database server configuration.
    #[command(alias = "rm")]
    Remove {
        /// Server configuration name to remove.
        name: String,
    },

    /// Test connection to a database server.
    Test {
        /// Server configuration name to test.
        name: String,
    },

    /// Securely set/encrypt password for a database server using DPAPI.
    SetPassword {
        /// Server configuration name.
        name: String,
    },
}

pub struct DbServersHandler {
    config_service: UserConfigService,
}

impl DbServersHandler {
    pub fn new(config_service: UserConfigService) -> Self {
        Self { config_service }
    }

    pub async fn handle(&self, subcommand: DbServersSubcommand) -> Result<()> {
        match subcommand {
            DbServersSubcommand::List => self.list_servers(),
            DbServersSubcommand::Add { name, host, port, username, database, password } => {
                self.add_server(name, host, port, username, database, password)
            }
            DbServersSubcommand::Remove { name } => self.remove_server(&name),
            DbServersSubcommand::Test { name } => self.test_server(&name).await,
            DbServersSubcommand::SetPassword { name } => self.set_password(&name),
        }
    }

    fn list_servers(&self) -> Result<()> {
        let servers = self.config_service.get_servers()?;
        if servers.is_empty() {
            println!("{}", "No database servers configured.".yellow());
            return Ok(());
        }

        println!("\n{}", "Configured Database Servers:".bright_cyan().bold());
        println!("{:<15} {:<25} {:<8} {:<15} {:<15}", "NAME", "HOST", "PORT", "USER", "STATUS");
        println!("{}", "-".repeat(80));

        for s in servers {
            let pass_status = if s.encrypted_password.is_some() {
                "[Encrypted]".green()
            } else if s.password.is_some() {
                "[Plaintext]".yellow()
            } else {
                "[No Pass]".dimmed()
            };

            println!(
                "{:<15} {:<25} {:<8} {:<15} {}",
                s.name.bold(),
                s.host,
                s.port,
                s.username,
                pass_status
            );
        }
        println!();
        Ok(())
    }

    fn add_server(
        &self,
        name: String,
        host: String,
        port: u16,
        username: String,
        database: Option<String>,
        password: Option<String>,
    ) -> Result<()> {
        let entry = ServerConfigEntry {
            name: name.clone(),
            host,
            port,
            username,
            password,
            encrypted_password: None,
            databases: database.map(|d| vec![d]),
            fetch_all_databases: None,
            exclude_patterns: None,
            ssl_mode: None,
            timeout: Some(30),
            command_timeout: Some(300),
            max_parallelism: Some(4),
        };

        self.config_service.add_or_update_server(entry)?;
        println!("{} Server '{}' added/updated successfully.", "✔".green(), name.bold());
        Ok(())
    }

    fn remove_server(&self, name: &str) -> Result<()> {
        let removed = self.config_service.remove_server(name)?;
        if removed {
            println!("{} Server '{}' removed successfully.", "✔".green(), name.bold());
        } else {
            println!("{} Server '{}' not found.", "✖".red(), name);
        }
        Ok(())
    }

    async fn test_server(&self, name: &str) -> Result<()> {
        let server = self.config_service.get_server(name)?
            .ok_or_else(|| MustelError::Config(format!("Server '{}' not found.", name)))?;

        print!("Testing connection to '{}' ({}:{})... ", server.name, server.host, server.port);
        match DbExecutor::connect(&server, None, None).await {
            Ok(_client) => {
                println!("{}", "SUCCESS ✔".green().bold());
                Ok(())
            }
            Err(e) => {
                println!("{}", "FAILED ✖".red().bold());
                println!("Error: {}", e);
                Ok(())
            }
        }
    }

    fn set_password(&self, name: &str) -> Result<()> {
        let _server = self.config_service.get_server(name)?
            .ok_or_else(|| MustelError::Config(format!("Server '{}' not found.", name)))?;

        let password = inquire::Password::new(&format!("Enter password for server '{}':", name))
            .without_confirmation()
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let encrypted = CredentialStore::encrypt_password(&password)?;
        self.config_service.set_encrypted_password(name, encrypted)?;

        println!("{} Encrypted password stored securely for server '{}'.", "✔".green(), name.bold());
        Ok(())
    }
}
