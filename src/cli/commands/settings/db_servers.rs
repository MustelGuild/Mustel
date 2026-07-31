use clap::Subcommand;
use colored::Colorize;

use crate::config::models::{ServerConfigEntry, SslMode};
use crate::config::UserConfigService;
use crate::database::DbExecutor;
use crate::error::{MustelError, Result};
use crate::security::CredentialStore;

#[derive(Subcommand, Debug, Clone)]
pub enum DbServersSubcommand {
    /// List all configured database servers.
    #[command(alias = "ls")]
    List,

    /// Add or update a database server configuration (enters interactive mode if arguments are missing).
    Add {
        /// Server configuration name (positional or -n/--name).
        #[arg(short = 'n', long = "name")]
        name: Option<String>,

        /// Host address.
        #[arg(short = 'H', long = "host")]
        host: Option<String>,

        /// Port number.
        #[arg(short = 'p', long = "port")]
        port: Option<u16>,

        /// Username.
        #[arg(short = 'U', long = "username")]
        username: Option<String>,

        /// Target database name.
        #[arg(short = 'd', long = "database")]
        database: Option<String>,

        /// Plaintext password (optional, set-password recommended).
        #[arg(short = 'W', long = "password")]
        password: Option<String>,

        /// Auto-discover all databases on server.
        #[arg(long = "fetch-all")]
        fetch_all: bool,

        /// Comma-separated patterns to exclude from auto-discovery.
        #[arg(long = "exclude-patterns")]
        exclude_patterns: Option<String>,

        /// SSL mode (Disable, Allow, Prefer, Require, VerifyCa, VerifyFull).
        #[arg(long = "ssl-mode")]
        ssl_mode: Option<String>,

        /// Connection timeout in seconds.
        #[arg(long = "timeout")]
        timeout: Option<u64>,

        /// Max degree of parallelism.
        #[arg(long = "max-parallelism")]
        max_parallelism: Option<usize>,
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
            DbServersSubcommand::Add {
                name,
                host,
                port,
                username,
                database,
                password,
                fetch_all,
                exclude_patterns,
                ssl_mode,
                timeout,
                max_parallelism,
            } => self.add_server(
                name,
                host,
                port,
                username,
                database,
                password,
                fetch_all,
                exclude_patterns,
                ssl_mode,
                timeout,
                max_parallelism,
            ),
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
        name: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        username: Option<String>,
        database: Option<String>,
        password: Option<String>,
        fetch_all: bool,
        exclude_patterns: Option<String>,
        ssl_mode: Option<String>,
        timeout: Option<u64>,
        max_parallelism: Option<usize>,
    ) -> Result<()> {
        // Direct Non-Interactive Mode: when both name and host are provided via flags
        if let (Some(n), Some(h)) = (&name, &host) {
            let mut enc_pass = None;
            if let Some(p) = &password {
                if !p.is_empty() {
                    enc_pass = Some(CredentialStore::encrypt_password(p)?);
                }
            }

            let ssl_enum = ssl_mode.as_deref().and_then(|s| s.parse::<SslMode>().ok());
            let dbs = database.map(|d| d.split(',').map(|s| s.trim().to_string()).collect());
            let excludes = exclude_patterns.map(|ex| ex.split(',').map(|s| s.trim().to_string()).collect());

            let entry = ServerConfigEntry {
                name: n.clone(),
                host: h.clone(),
                port: port.unwrap_or(5432),
                username: username.unwrap_or_else(|| "postgres".into()),
                password: None,
                encrypted_password: enc_pass,
                databases: dbs,
                fetch_all_databases: if fetch_all { Some(true) } else { None },
                exclude_patterns: excludes,
                ssl_mode: ssl_enum,
                timeout: timeout.or(Some(30)),
                command_timeout: Some(300),
                max_parallelism: max_parallelism.or(Some(4)),
            };

            self.config_service.add_or_update_server(entry)?;
            println!("{} Server '{}' added/updated successfully.", "✔".green(), n.bold());
            return Ok(());
        }

        // Interactive Mode (when name or host are missing)
        println!("\n{}", "🌐 Interactive Database Server Setup".bright_cyan().bold());

        let initial_name = name.unwrap_or_default();
        let server_name = inquire::Text::new("Server name*:")
            .with_default(&initial_name)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        if server_name.trim().is_empty() {
            return Err(MustelError::Config("Server name is required.".into()));
        }

        let initial_host = host.unwrap_or_else(|| "localhost".to_string());
        let server_host = inquire::Text::new("Host*:")
            .with_default(&initial_host)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        if server_host.trim().is_empty() {
            return Err(MustelError::Config("Host address is required.".into()));
        }

        let default_port_str = port.unwrap_or(5432).to_string();
        let port_str = inquire::Text::new("Port [5432]:")
            .with_default(&default_port_str)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;
        let server_port = port_str.parse::<u16>().unwrap_or(5432);

        let default_user = username.unwrap_or_else(|| "postgres".to_string());
        let server_user = inquire::Text::new("Username [postgres]:")
            .with_default(&default_user)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let server_pass = inquire::Password::new("Password (optional, encrypted with DPAPI):")
            .without_confirmation()
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let enc_pass = if !server_pass.is_empty() {
            Some(CredentialStore::encrypt_password(&server_pass)?)
        } else {
            None
        };

        let db_input = inquire::Text::new("Databases (comma-separated, optional):")
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let databases = if !db_input.trim().is_empty() {
            Some(db_input.split(',').map(|s| s.trim().to_string()).collect())
        } else {
            None
        };

        let auto_discover = inquire::Confirm::new("Auto-discover databases on server?")
            .with_default(fetch_all)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let exclude_patterns = if auto_discover {
            let excludes_input = inquire::Text::new("Exclude patterns [template*,postgres]:")
                .with_default("template*,postgres")
                .prompt()
                .map_err(|_| MustelError::UserCancelled)?;
            Some(excludes_input.split(',').map(|s| s.trim().to_string()).collect())
        } else {
            None
        };

        let ssl_options = vec!["Prefer", "Disable", "Allow", "Require", "VerifyCA", "VerifyFull"];
        let selected_ssl = inquire::Select::new("SSL Mode [Prefer]:", ssl_options)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let ssl_enum = selected_ssl.parse::<SslMode>().ok();

        let timeout_str = inquire::Text::new("Connection timeout (seconds) [30]:")
            .with_default("30")
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;
        let timeout_val = timeout_str.parse::<u64>().unwrap_or(30);

        let entry = ServerConfigEntry {
            name: server_name.trim().to_string(),
            host: server_host.trim().to_string(),
            port: server_port,
            username: server_user.trim().to_string(),
            password: None,
            encrypted_password: enc_pass,
            databases,
            fetch_all_databases: if auto_discover { Some(true) } else { None },
            exclude_patterns,
            ssl_mode: ssl_enum,
            timeout: Some(timeout_val),
            command_timeout: Some(300),
            max_parallelism: Some(4),
        };

        self.config_service.add_or_update_server(entry.clone())?;
        println!("\n{} Server '{}' added successfully!", "✔".green().bold(), entry.name.bold());
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
