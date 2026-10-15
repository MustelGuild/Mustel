//! Mustel — CLI de automação.
//!
//! Este binário é o frontend de terminal: parseia argumentos ([`cli`]), delega aos casos de uso de
//! `mustel-core` e traduz o resultado em mensagem + exit code ([`exit`]).

mod cli;
mod commands;
mod exit;

use clap::Parser;

use mustel_core::error::CoreError;
use mustel_core::signal::{cancellation_token, spawn_ctrl_c_listener};

#[tokio::main]
async fn main() {
    let parsed = cli::MustelCli::parse();

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
