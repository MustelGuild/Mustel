use inquire::{Confirm, MultiSelect};
use colored::Colorize;

use crate::config::models::ServerConfigEntry;
use crate::error::{MustelError, Result};

pub struct UiPrompts;

impl UiPrompts {
    /// Interactive prompt to select one or more database servers when not specified via CLI.
    pub fn select_servers(available_servers: &[ServerConfigEntry]) -> Result<Vec<ServerConfigEntry>> {
        if available_servers.is_empty() {
            return Err(MustelError::Config(
                "No database servers configured. Use 'mustel settings db-servers add' to configure a server.".into()
            ));
        }

        if available_servers.len() == 1 {
            return Ok(vec![available_servers[0].clone()]);
        }

        let options: Vec<String> = available_servers
            .iter()
            .map(|s| format!("{} ({}:{})", s.name, s.host, s.port))
            .collect();

        let selected = MultiSelect::new("Select database server(s) to query:", options)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)?;

        let mut result = Vec::new();
        for sel in selected {
            if let Some(srv) = available_servers.iter().find(|s| sel.starts_with(&s.name)) {
                result.push(srv.clone());
            }
        }

        if result.is_empty() {
            return Err(MustelError::UserCancelled);
        }

        Ok(result)
    }

    /// Interactive prompt to confirm destructive query execution.
    pub fn confirm_destructive_query(action_type: &str, target_summary: &str) -> Result<bool> {
        println!("\n{}", "⚠️  DESTRUCTIVE QUERY WARNING ⚠️".bright_red().bold());
        println!(
            "The query contains a {} operation targeting: {}",
            action_type.bold().red(),
            target_summary.yellow()
        );

        Confirm::new("Are you sure you want to execute this destructive query?")
            .with_default(false)
            .prompt()
            .map_err(|_| MustelError::UserCancelled)
    }
}
