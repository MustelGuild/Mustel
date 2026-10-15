//! Comandos do grupo `query`.

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{QueryArgs, QueryCommand};
use crate::commands::not_implemented;

/// Despacha `mustel query <subcomando>`.
pub async fn run(args: QueryArgs, _token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        QueryCommand::Run(_run_args) => not_implemented("query run"),
    }
}
