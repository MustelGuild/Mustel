//! Testes de ponta a ponta de `settings db-servers ls` (pelo binário, com config isolada).
//!
//! O contrato observado no oráculo (T9) é conferido aqui sem depender do harness:
//! colunas, célula de bancos (`(auto)`/lista/`(default)`), `Auto-DB` e exit code 0 nos dois caminhos.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const SAMPLE_CONFIG: &str = r#"{
  "servers": [
    {
      "name": "dev",
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "databases": ["mydb", "app_dev"],
      "sslMode": "Prefer",
      "timeout": 30,
      "maxParallelism": 4
    },
    {
      "name": "auto",
      "host": "prod-db.company.com",
      "port": 5432,
      "username": "readonly",
      "fetchAllDatabases": true,
      "excludePatterns": ["template*", "postgres"],
      "sslMode": "Require",
      "timeout": 30,
      "maxParallelism": 2
    }
  ],
  "defaults": { "outputDirectory": "./results", "requireConfirmation": true, "maxParallelism": 4 }
}
"#;

/// Roda o binário com a config isolada em `dir`.
fn run_with_config(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mustel"))
        .args(args)
        .env("MUSTEL_CONFIG_DIR", dir)
        .output()
        .expect("binary must run")
}

#[test]
fn lists_configured_servers_with_the_legacy_columns() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), SAMPLE_CONFIG).expect("seed config");

    let output = run_with_config(dir.path(), &["settings", "db-servers", "ls"]);
    assert_eq!(output.status.code(), Some(0), "listar é sucesso");

    let stdout = String::from_utf8_lossy(&output.stdout);
    for header in ["Name", "Host", "Port", "Username", "Databases", "Auto-DB"] {
        assert!(
            stdout.contains(header),
            "faltou a coluna {header}:\n{stdout}"
        );
    }
    assert!(stdout.contains("dev"), "{stdout}");
    assert!(stdout.contains("localhost"), "{stdout}");
    assert!(
        stdout.contains("mydb, app_dev"),
        "lista com ', ' como o legado"
    );
    assert!(
        stdout.contains("(auto)"),
        "descoberta automática vira (auto)"
    );
    assert!(
        stdout.contains("Yes") && stdout.contains("No"),
        "coluna Auto-DB"
    );
}

#[test]
fn listing_an_empty_config_is_success_with_guidance() {
    let dir = TempDir::new().expect("tempdir");

    let output = run_with_config(dir.path(), &["settings", "db-servers", "ls"]);
    assert_eq!(output.status.code(), Some(0), "config vazia não é erro");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No servers configured"), "{stdout}");
    assert!(
        stdout.contains("mustel settings db-servers add"),
        "a mensagem precisa apontar o comando do próprio binário: {stdout}"
    );
}

#[test]
fn listing_creates_the_config_file_on_first_use() {
    let dir = TempDir::new().expect("tempdir");
    assert!(!dir.path().join("mustel.jsonc").exists());

    let output = run_with_config(dir.path(), &["settings", "db-servers", "ls"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        dir.path().join("mustel.jsonc").is_file(),
        "primeira execução cria a config vazia"
    );
}
