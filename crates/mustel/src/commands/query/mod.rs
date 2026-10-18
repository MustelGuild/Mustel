//! Comandos do grupo `query`.

pub mod run;
pub mod settings;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{QueryArgs, QueryCommand};

/// Despacha `mustel query <subcomando>`.
pub async fn run(args: QueryArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        QueryCommand::Run(run_args) => run::run(run_args, token).await,
    }
}
