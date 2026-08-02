mod cli;
mod config;
mod database;
mod error;
mod gui;
mod security;

use clap::Parser;
use cli::CliApp;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing logger with fontdb noise filter
    let env_filter = EnvFilter::from_default_env()
        .add_directive(tracing::Level::WARN.into())
        .add_directive("fontdb=error".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    let app = CliApp::parse();

    if let Err(e) = app.dispatch().await {
        eprintln!("\n{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}
