//! Comandos do grupo `settings`.

pub mod db_servers;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{DbServersCommand, SettingsArgs, SettingsCommand};
use crate::commands::not_implemented;

/// Despacha `mustel settings <subcomando>`.
pub async fn run(args: SettingsArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        SettingsCommand::DbServers(db_servers_args) => {
            db_servers::run(db_servers_args, token).await
        }
    }
}

/// Despacha `mustel settings db-servers <subcomando>`.
pub(crate) async fn dispatch_db_servers(
    command: DbServersCommand,
    _token: &CancellationToken,
) -> Result<i32, CoreError> {
    match command {
        DbServersCommand::Ls => db_servers::list::run(),
        DbServersCommand::Add(args) => db_servers::add::run(args),
        DbServersCommand::Rm(_) => not_implemented("settings db-servers rm"),
        DbServersCommand::Test(_) => not_implemented("settings db-servers test"),
        DbServersCommand::SetPassword(_) => not_implemented("settings db-servers set-password"),
    }
}
