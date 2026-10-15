//! Comandos do grupo `database`.

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{DatabaseArgs, DatabaseCommand, PgpassCommand};
use crate::commands::not_implemented;

/// Despacha `mustel database <subcomando>`.
pub async fn run(args: DatabaseArgs, _token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        DatabaseCommand::Backup(_) => not_implemented("database backup"),
        DatabaseCommand::Restore(_) => not_implemented("database restore"),
        DatabaseCommand::Pgpass(pgpass) => match pgpass.command {
            PgpassCommand::Add(_) => not_implemented("database pgpass add"),
            PgpassCommand::List => not_implemented("database pgpass list"),
            PgpassCommand::Remove(_) => not_implemented("database pgpass remove"),
        },
    }
}
