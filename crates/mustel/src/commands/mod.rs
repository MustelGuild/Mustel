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

/// O que o binário faz quando não recebe subcomando.
///
/// É a decisão do binário único (D9 + divergência 18): quem digita `mustel` num terminal espera a
/// interface; em script, pipe ou CI a ajuda é o contrato e o exit é 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootAction {
    /// Abre a janela (só faz sentido com terminal interativo).
    OpenGui,
    /// Imprime a ajuda e sai 0.
    PrintUsage,
}

/// Decide o que fazer sem subcomando. Função pura: o teste cobre as duas pontas sem abrir janela.
pub fn root_action(interactive: bool) -> RootAction {
    if interactive {
        RootAction::OpenGui
    } else {
        RootAction::PrintUsage
    }
}

/// Executa o comando pedido e devolve o exit code (`Ok`) ou o erro para o chamador reportar.
pub async fn dispatch(cli: MustelCli, token: &CancellationToken) -> Result<i32, CoreError> {
    let Some(command) = cli.command else {
        // Sem subcomando: a GUI é o padrão **de quem tem terminal**; sem ele, a ajuda é o contrato
        // (o oráculo imprime a ajuda e sai 0, e é isso que scripts e o harness esperam).
        return match root_action(ui::is_interactive()) {
            RootAction::OpenGui => gui(token),
            RootAction::PrintUsage => Ok(print_usage()),
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

#[cfg(test)]
mod tests {
    use super::{RootAction, root_action};

    /// IF-180: com terminal interativo o binário sem subcomando abre a GUI; sem terminal, imprime a ajuda.
    ///
    /// O que o teste prende é a **decisão**; abrir a janela em si não é verificável em teste (a outra
    /// metade é `nao-coberta` no manifesto).
    /// CONTRACT: IF-180
    #[test]
    fn without_a_subcommand_the_terminal_decides_between_gui_and_usage() {
        assert_eq!(root_action(true), RootAction::OpenGui);
        assert_eq!(root_action(false), RootAction::PrintUsage);
    }
}
