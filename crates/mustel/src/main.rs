//! Mustel — CLI de automação.
//!
//! O binário é uma casca fina: parseia argumentos, monta o token de cancelamento e delega à
//! biblioteca (`mustel::commands`). A árvore de comandos e as primitivas de UI vivem na lib para
//! serem testáveis sem processo.

use clap::Parser;

use mustel::cli::MustelCli;
use mustel::commands;
use mustel::exit;
use mustel_core::error::{CoreError, exit_code};
use mustel_core::signal::{cancellation_token, spawn_ctrl_c_listener};

#[tokio::main]
async fn main() {
    warn_if_password_is_on_the_command_line();

    let parsed = match MustelCli::try_parse() {
        Ok(parsed) => parsed,
        Err(error) => std::process::exit(handle_parse_error(&error)),
    };

    let token = cancellation_token();
    // Ctrl+C cancela o token; quem observa o token é o caso de uso em execução.
    let _listener = spawn_ctrl_c_listener(token.clone());

    let code = tokio::select! {
        result = commands::dispatch(parsed, &token) => match result {
            Ok(code) => code,
            Err(error) => exit::report_error(&error),
        },
        () = token.cancelled() => exit::report_error(&CoreError::Cancelled),
    };

    std::process::exit(code);
}

/// `-W` deixa a senha visível em `ps` e no histórico do shell; avisa quando há terminal.
fn warn_if_password_is_on_the_command_line() {
    use std::io::IsTerminal;

    let uses_flag = std::env::args().any(|argument| {
        argument == "-W" || argument == "--password" || argument.starts_with("--password=")
    });
    if uses_flag && std::io::stdin().is_terminal() {
        eprintln!(
            "Warning: -W exposes the password in the process list and shell history; prefer \
             MUSTEL_PASSWORD or the keyring."
        );
    }
}

/// Trata erro de parse com o contrato do oráculo (descoberto no T9).
///
/// O legado devolve `1` e escreve a mensagem em **stdout** para erro de uso (comando desconhecido,
/// opção desconhecida, argumento obrigatório ausente), enquanto o clap usaria `2` e `stderr`.
/// `--help`/`--version` continuam saindo com `0` em stdout, como no legado.
fn handle_parse_error(error: &clap::Error) -> i32 {
    use clap::error::ErrorKind;

    match error.kind() {
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayVersion
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            print!("{error}");
            exit_code::SUCCESS
        }
        _ => {
            println!("Error: {error}");
            exit_code::GENERAL
        }
    }
}
