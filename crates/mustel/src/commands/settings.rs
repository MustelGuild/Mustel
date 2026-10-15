//! Comandos do grupo `settings`.

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{DbServersCommand, SettingsArgs, SettingsCommand};
use crate::commands::not_implemented;

/// Despacha `mustel settings <subcomando>`.
pub async fn run(args: SettingsArgs, _token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        SettingsCommand::DbServers(db_servers) => match db_servers.command {
            DbServersCommand::Ls => not_implemented("settings db-servers ls"),
            DbServersCommand::Add(_) => not_implemented("settings db-servers add"),
            DbServersCommand::Rm(_) => not_implemented("settings db-servers rm"),
            DbServersCommand::Test(_) => not_implemented("settings db-servers test"),
            DbServersCommand::SetPassword(_) => not_implemented("settings db-servers set-password"),
        },
    }
}
