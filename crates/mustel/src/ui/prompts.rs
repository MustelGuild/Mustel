//! Prompts interativos (adaptador de terminal sobre `inquire`).
//!
//! Cada função aqui corresponde a um prompt do Spectre usado pelo FurLab. `Ctrl+C` durante o prompt
//! vira [`CoreError::Cancelled`] — o mesmo `130` que o CLI legado devolve quando o usuário cancela.

use inquire::{Confirm, InquireError, MultiSelect, Password, Select, Text};

use mustel_core::error::CoreError;

use super::ensure_interactive;

/// Converte erro do `inquire` no erro do núcleo.
fn map_error(error: InquireError) -> CoreError {
    match error {
        // Usuário abortou (Ctrl+C ou Esc): o CLI sai com 130, como o legado.
        InquireError::OperationInterrupted | InquireError::OperationCanceled => {
            CoreError::Cancelled
        }
        other => CoreError::InvalidState(format!("prompt failed: {other}")),
    }
}

/// Seleção simples: devolve o item escolhido.
pub fn select_one(
    message: &str,
    options: &[String],
    default_index: Option<usize>,
) -> Result<String, CoreError> {
    ensure_interactive(message)?;

    let mut prompt = Select::new(message, options.to_vec());
    if let Some(index) = default_index {
        prompt = prompt.with_starting_cursor(index);
    }
    prompt.prompt().map_err(map_error)
}

/// Multi-seleção: devolve os itens marcados (os `preselected` já vêm marcados, como o FurLab faz ao
/// pré-selecionar todos os servidores).
pub fn select_many(
    message: &str,
    options: &[String],
    preselected: &[usize],
) -> Result<Vec<String>, CoreError> {
    ensure_interactive(message)?;

    MultiSelect::new(message, options.to_vec())
        .with_default(preselected)
        .prompt()
        .map_err(map_error)
}

/// Confirmação com default (equivalente ao `ConfirmationPrompt` do Spectre).
pub fn confirm(message: &str, default: bool) -> Result<bool, CoreError> {
    ensure_interactive(message)?;

    Confirm::new(message)
        .with_default(default)
        .prompt()
        .map_err(map_error)
}

/// Texto livre, com valor default opcional.
pub fn text(message: &str, default: Option<&str>) -> Result<String, CoreError> {
    ensure_interactive(message)?;

    let mut prompt = Text::new(message);
    if let Some(value) = default {
        prompt = prompt.with_default(value);
    }
    prompt.prompt().map_err(map_error)
}

/// Senha com máscara, sem confirmação duplicada (o legado pede uma vez).
pub fn password(message: &str) -> Result<String, CoreError> {
    ensure_interactive(message)?;

    Password::new(message)
        .without_confirmation()
        .prompt()
        .map_err(map_error)
}
