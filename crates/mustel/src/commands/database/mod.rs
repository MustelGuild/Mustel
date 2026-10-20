//! Comandos do grupo `database`.
//!
//! `backup` e `restore` são adaptadores: a regra mora em `mustel_core::usecases::database_*` e cada
//! um tem aqui a sua **tabela tipo de falha → código de saída** (os códigos do legado vêm do corpo do
//! comando e mudam entre os dois — binário ausente é 3 no `backup` e 1 no `restore`).

pub mod backup;
pub mod pgpass;
pub mod restore;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{DatabaseArgs, DatabaseCommand, PgpassCommand};

/// Despacha `mustel database <subcomando>`.
pub async fn run(args: DatabaseArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        DatabaseCommand::Backup(args) => backup::run(args, token).await,
        DatabaseCommand::Restore(args) => restore::run(args, token).await,
        DatabaseCommand::Pgpass(pgpass) => match pgpass.command {
            PgpassCommand::Add(args) => pgpass::add::run(args),
            PgpassCommand::List => pgpass::list::run(),
            PgpassCommand::Remove(args) => pgpass::remove::run(args),
        },
    }
}
