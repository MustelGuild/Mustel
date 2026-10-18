//! Testes de ponta a ponta de `settings db-servers add`.
//!
//! Contrato observado no oráculo (T9) e no código legado: validação recusada → 41, nome duplicado →
//! 40, sucesso → 0 com a mensagem de orientação do `set-password`. O artefato de configuração
//! escrito aqui é comparado com o do legado pelo `oracle-compare` (caso `settings-db-servers-add-flags`).

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

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
fn adds_a_server_with_flags_and_writes_the_config() {
    let dir = TempDir::new().expect("tempdir");

    let output = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "capturado",
            "-H",
            "localhost",
            "-p",
            "5432",
            "-U",
            "postgres",
            "-d",
            "mydb,app_dev",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {}",
        stdout_of(&output)
    );
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Server 'capturado' added successfully."),
        "{stdout}"
    );
    assert!(
        stdout.contains("mustel settings db-servers set-password capturado"),
        "a dica precisa apontar o binário do Mustel: {stdout}"
    );

    let written = std::fs::read_to_string(dir.path().join("mustel.jsonc")).expect("config escrita");
    for expected in [
        "\"name\": \"capturado\"",
        "\"host\": \"localhost\"",
        "\"port\": 5432",
        "\"username\": \"postgres\"",
        "\"mydb\"",
        "\"app_dev\"",
        "\"template*\"",
        "\"prefer\"",
    ] {
        let needle = expected.replace("prefer", "Prefer");
        assert!(
            written.contains(&needle),
            "config deveria conter {needle}:\n{written}"
        );
    }
    assert!(
        !written.contains("encryptedPassword"),
        "senha não é mais gravada em arquivo (D6):\n{written}"
    );
}

#[test]
fn duplicate_name_is_refused_with_exit_40() {
    let dir = TempDir::new().expect("tempdir");
    let first = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "dev",
            "-H",
            "localhost",
        ],
    );
    assert_eq!(first.status.code(), Some(0));

    let second = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "dev",
            "-H",
            "other.host",
        ],
    );
    assert_eq!(
        second.status.code(),
        Some(40),
        "conflito de estado é 40 no legado"
    );
    assert!(
        stdout_of(&second).contains("Server 'dev' already exists."),
        "{}",
        stdout_of(&second)
    );
}

#[test]
fn invalid_host_and_port_are_argument_errors() {
    let dir = TempDir::new().expect("tempdir");

    let bad_host = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "x",
            "-H",
            "host_invalido",
        ],
    );
    assert_eq!(bad_host.status.code(), Some(41));
    assert!(stdout_of(&bad_host).contains("Invalid host: 'host_invalido'."));

    let bad_port = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "y",
            "-H",
            "localhost",
            "-p",
            "70000",
        ],
    );
    assert_eq!(bad_port.status.code(), Some(41));
    assert!(
        stdout_of(&bad_port).contains("Invalid port: 70000. Must be between 1 and 65535."),
        "{}",
        stdout_of(&bad_port)
    );
}

#[test]
fn wildcards_are_accepted_like_the_legacy() {
    let dir = TempDir::new().expect("tempdir");

    let output = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            "wild",
            "-H",
            "*",
            "-U",
            "*",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "o legado aceita curinga em host e usuário: {}",
        stdout_of(&output)
    );
}

#[test]
fn storing_a_password_with_w_cleans_up_after_itself() {
    // O `-W` grava no cofre **real** do usuário, então o teste apaga no fim. Isto não é zelo
    // excessivo: uma execução anterior deixou um `dev.mustel` órfão no Windows Credential Manager,
    // encontrado no smoke do T20.
    if std::env::var("MUSTEL_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set MUSTEL_KEYRING_TESTS=1 to exercise the real keyring");
        return;
    }

    use mustel_core::credentials::{CredentialStore, SystemKeyring};

    let dir = TempDir::new().expect("tempdir");
    let keyring = SystemKeyring::new();
    let server = "t17-keyring-check";
    let _ = keyring.delete(server);

    let output = run(
        dir.path(),
        &[
            "settings",
            "db-servers",
            "add",
            "-n",
            server,
            "-H",
            "localhost",
            "-W",
            "segredo-t17",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stdout_of(&output));
    assert_eq!(
        keyring.get(server).expect("read"),
        Some("segredo-t17".to_string()),
        "o -W precisa guardar a senha no cofre"
    );

    assert!(
        keyring.delete(server).expect("cleanup"),
        "limpeza obrigatória"
    );
    assert_eq!(keyring.get(server).expect("read after"), None);
}
