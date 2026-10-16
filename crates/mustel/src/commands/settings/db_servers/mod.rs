//! Comandos de `mustel settings db-servers`.

pub mod add;
pub mod list;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::DbServersArgs;

/// Despacha `mustel settings db-servers <subcomando>`.
pub async fn run(args: DbServersArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    super::dispatch_db_servers(args.command, token).await
}
