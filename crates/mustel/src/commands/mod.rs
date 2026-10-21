//! Despacho dos comandos do CLI.
//!
//! Cada grupo tem seu módulo; os comandos em si são finos (parse → resolve → caso de uso do core).
//! **Todos os comandos do escopo estão migrados** — não há mais stub, e por isso não há mais o
//! helper `not_implemented` que existia nesta fase.

pub mod database;
pub mod query;
pub mod settings;

use mustel_core::error::{CoreError, exit_code};
use mustel_core::signal::CancellationToken;

use crate::cli::{MustelCli, TopCommand};
use crate::ui;

/// Executa o comando pedido e devolve o exit code (`Ok`) ou o erro para o chamador reportar.
pub async fn dispatch(cli: MustelCli, token: &CancellationToken) -> Result<i32, CoreError> {
    let Some(command) = cli.command else {
        // Sem subcomando: a GUI é o padrão **de quem tem terminal**; sem ele, a ajuda é o contrato
        // (o oráculo imprime a ajuda e sai 0, e é isso que scripts e o harness esperam).
        return match ui::is_interactive() {
            true => gui(token),
            false => Ok(print_usage()),
        };
    };

    match command {
        TopCommand::Query(args) => query::run(args, token).await,
        TopCommand::Database(args) => database::run(args, token).await,
        TopCommand::Settings(args) => settings::run(args, token).await,
        TopCommand::Gui(_) => gui(token),
    }
}

/// Imprime a ajuda da raiz e devolve 0, como o oráculo faz sem argumentos.
fn print_usage() -> i32 {
    let cli = <MustelCli as clap::CommandFactory>::command();
    let mut cli = cli;

    // `print_help` só falha se a saída padrão falhar (pipe quebrado); aí o código não importa mais.
    let _ = cli.print_help();
    println!();

    exit_code::SUCCESS
}

/// Abre a janela do `mustel gui`.
///
/// Sem a feature `gui` (build `--no-default-features`), o subcomando existe na árvore mas explica a
/// ausência em vez de abrir a janela — assim o CLI sem `egui` continua com a mesma superfície.
#[cfg(feature = "gui")]
fn gui(_token: &CancellationToken) -> Result<i32, CoreError> {
    mustel_gui::run().map_err(|error| CoreError::Other(error.to_string()))?;

    Ok(0)
}

#[cfg(not(feature = "gui"))]
fn gui(_token: &CancellationToken) -> Result<i32, CoreError> {
    Err(CoreError::InvalidState(
        "this build has no GUI support; rebuild without --no-default-features to get `mustel gui`"
            .to_string(),
    ))
}
