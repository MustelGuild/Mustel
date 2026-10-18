//! Despacho dos comandos do CLI.
//!
//! Cada grupo tem seu módulo; os comandos em si são finos (parse → resolve → caso de uso do core).
//! **Todos os comandos do escopo estão migrados** — não há mais stub, e por isso não há mais o
//! helper `not_implemented` que existia nesta fase.

pub mod database;
pub mod query;
pub mod settings;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{MustelCli, TopCommand};

/// Executa o comando pedido e devolve o exit code (`Ok`) ou o erro para o chamador reportar.
pub async fn dispatch(cli: MustelCli, token: &CancellationToken) -> Result<i32, CoreError> {
    match cli.command {
        TopCommand::Query(args) => query::run(args, token).await,
        TopCommand::Database(args) => database::run(args, token).await,
        TopCommand::Settings(args) => settings::run(args, token).await,
        TopCommand::Gui(_) => Err(CoreError::Other("`gui` is not implemented yet".to_string())),
    }
}
