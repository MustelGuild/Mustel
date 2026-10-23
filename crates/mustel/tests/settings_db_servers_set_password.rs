//! Testes de ponta a ponta de `settings db-servers set-password`.
//!
//! Contrato do legado: sem servidores cadastrados → mensagem e exit 0; nome inexistente → 41;
//! sucesso → `Password saved securely for 'x'.` e exit 0.
//!
//! O caminho que **grava** no cofre só roda com `MUSTEL_KEYRING_TESTS=1`, porque escreve no cofre
//! real do usuário (e apaga no fim). Sem a variável, valida-se apenas o que não toca o cofre.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const ONE_SERVER: &str = r#"{
  "servers": [
    { "name": "dev", "host": "localhost", "port": 5432, "username": "postgres",
      "sslMode": "Prefer", "timeout": 30, "maxParallelism": 4 }
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

fn keyring_tests_enabled() -> bool {
    std::env::var("MUSTEL_KEYRING_TESTS").as_deref() == Ok("1")
}

#[test]
/// CONTRACT: IF-127
fn without_a_name_and_without_servers_it_explains_and_succeeds() {
    let dir = TempDir::new().expect("tempdir");

    // Sem nome, o legado entra na seleção; sem servidores cadastrados, avisa e devolve 0.
    let output = run(dir.path(), &["settings", "db-servers", "set-password"]);
    assert_eq!(output.status.code(), Some(0), "config vazia não é erro");
    assert!(
        stdout_of(&output).contains("No servers configured"),
        "{}",
        stdout_of(&output)
    );
}

#[test]
fn a_name_with_an_empty_config_is_not_found() {
    let dir = TempDir::new().expect("tempdir");

    // Com nome informado o legado vai direto ao `GetServer` e falha — a mensagem de config vazia
    // **não** aparece nesse caminho.
    let output = run(
        dir.path(),
        &["settings", "db-servers", "set-password", "dev", "-W", "x"],
    );
    assert_eq!(output.status.code(), Some(41));
    assert!(
        stdout_of(&output).contains("Server 'dev' not found."),
        "{}",
        stdout_of(&output)
    );
}

#[test]
/// CONTRACT: IF-125
fn unknown_server_is_an_argument_error() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), ONE_SERVER).expect("seed");

    let output = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "set-password",
            "nao-existe",
            "-W",
            "segredo",
        ],
    );
    assert_eq!(output.status.code(), Some(41));
    assert!(
        stdout_of(&output).contains("Server 'nao-existe' not found."),
        "{}",
        stdout_of(&output)
    );
}

#[test]
/// CONTRACT: IF-128
fn without_a_terminal_and_without_the_flag_it_asks_for_a_flag() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), ONE_SERVER).expect("seed");

    // Sem `-W` o comando precisaria de prompt; sem terminal ele falha claro (40) em vez de travar.
    let output = run(
        dir.path(),
        &["settings", "db-servers", "set-password", "dev"],
    );
    assert_eq!(output.status.code(), Some(40));
    assert!(
        stdout_of(&output).contains("interactive terminal"),
        "{}",
        stdout_of(&output)
    );
}

#[test]
/// CONTRACT: IF-115
/// CONTRACT: IF-126
fn stores_the_password_in_the_keyring_when_enabled() {
    if !keyring_tests_enabled() {
        eprintln!("skipped: set MUSTEL_KEYRING_TESTS=1 to exercise the real keyring");
        return;
    }

    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("mustel.jsonc"), ONE_SERVER).expect("seed");

    let output = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "set-password",
            "dev",
            "-W",
            "segredo-t21",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stdout_of(&output));
    assert!(
        stdout_of(&output).contains("Password saved securely for 'dev'."),
        "{}",
        stdout_of(&output)
    );

    // A credencial existiu de fato (leitura direta do cofre) e é apagada em seguida.
    use mustel_core::credentials::{CredentialStore, SystemKeyring};
    let keyring = SystemKeyring::new();
    assert_eq!(
        keyring.get("dev").expect("read"),
        Some("segredo-t21".to_string()),
        "a senha precisa estar no cofre"
    );
    assert!(keyring.delete("dev").expect("cleanup"));
}
