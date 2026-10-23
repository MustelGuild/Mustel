//! Testes de ponta a ponta de `settings db-servers rm`.
//!
//! Contrato do legado: sem servidores → 0 com mensagem; `-n` inexistente → 41; `-n` válido → 0.
//! O artefato de config resultante é comparado pelo `oracle-compare` (caso `settings-db-servers-rm-ok`).

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const TWO_SERVERS: &str = r#"{
  "servers": [
    {
      "name": "dev",
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "sslMode": "Prefer",
      "timeout": 30,
      "maxParallelism": 4
    },
    {
      "name": "offline",
      "host": "127.0.0.1",
      "port": 1,
      "username": "postgres",
      "sslMode": "Disable",
      "timeout": 1
    }
  ],
  "defaults": { "outputDirectory": "./results", "requireConfirmation": true, "maxParallelism": 4 }
}
"#;

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mustel"))
        .args(args)
        .env("MUSTEL_CONFIG_DIR", dir)
        .output()
        .expect("binary must run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
/// CONTRACT: IF-122
fn removes_a_server_by_name_and_updates_the_config() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), TWO_SERVERS).expect("seed");

    let output = run(dir.path(), &["settings", "db-servers", "rm", "-n", "dev"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout_of(&output));
    assert!(
        stdout_of(&output).contains("Server 'dev' removed successfully."),
        "{}",
        stdout_of(&output)
    );

    let written = std::fs::read_to_string(dir.path().join("mustel.jsonc")).expect("config");
    assert!(
        !written.contains("\"dev\""),
        "o removido não pode sobrar:\n{written}"
    );
    assert!(
        written.contains("\"offline\""),
        "os demais ficam:\n{written}"
    );
}

#[test]
/// CONTRACT: IF-123
fn removing_an_unknown_server_is_an_argument_error() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), TWO_SERVERS).expect("seed");

    let output = run(
        dir.path(),
        &["settings", "db-servers", "rm", "-n", "nao-existe"],
    );
    assert_eq!(output.status.code(), Some(41));
    assert!(
        stdout_of(&output).contains("Server 'nao-existe' not found."),
        "{}",
        stdout_of(&output)
    );
}

#[test]
/// CONTRACT: IF-124
fn removing_from_an_empty_config_is_success_with_a_message() {
    let dir = TempDir::new().expect("tempdir");

    let output = run(dir.path(), &["settings", "db-servers", "rm", "-n", "dev"]);
    assert_eq!(output.status.code(), Some(0), "config vazia não é erro");
    assert!(
        stdout_of(&output).contains("No servers configured to remove."),
        "{}",
        stdout_of(&output)
    );
}
