mod cli;
mod config;
mod database;
mod error;
mod security;

use clap::Parser;
use cli::CliApp;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing logger
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::WARN.into()))
        .init();

    let app = CliApp::parse();

    if let Err(e) = app.dispatch().await {
        eprintln!("\n{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}
