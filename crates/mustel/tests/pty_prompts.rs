//! Prompts sob pseudo-terminal (paridade de teclado).
//!
//! Por que assim: as primitivas de UI só interagem de verdade quando existe terminal, e o processo de
//! teste não tem um. O padrão é "auto-execução": o próprio binário de teste é reexecutado sob um PTY
//! com `MUSTEL_PTY_CHILD` definida; o filho chama o prompt de verdade e imprime o resultado, que o
//! pai verifica.
//!
//! **Cobertura**: só Unix — no Windows o `expectrl` depende de `conpty` e não é confiável. Estes
//! testes são exercitados pelo job `compat` do CI (ubuntu) e **não** foram executados localmente no
//! Windows durante a migração; no Windows a paridade interativa é coberta por smoke manual.
#![cfg(unix)]

use std::process::Command;

use expectrl::{Expect, spawn};

const CHILD_ENV: &str = "MUSTEL_PTY_CHILD";

/// Linha de comando que reexecuta este mesmo teste sob um PTY.
fn child_command(test_name: &str, body: &str) -> String {
    let exe = std::env::current_exe().expect("current test binary");
    Command::new(exe)
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD_ENV, body)
        .env("TERM", "xterm-256color")
        .to_string_lossy()
        .to_string()
}

/// IF-140: a confirmação destrutiva aceita o default (sim) ao apertar Enter.
/// CONTRACT: IF-140
#[test]
fn confirm_accepts_default_on_enter() {
    if let Ok(body) = std::env::var(CHILD_ENV) {
        assert_eq!(body, "confirm");
        let answer = mustel::ui::prompts::confirm("Continuar?", true).expect("prompt");
        println!("ANSWER={answer}");
        return;
    }

    let mut session = spawn(child_command("confirm_accepts_default_on_enter", "confirm"))
        .expect("spawn under a pty");
    session.send_line("").expect("send enter");
    session
        .expect("ANSWER=true")
        .expect("prompt must accept the default");
}

#[test]
fn confirm_denies_with_n() {
    if let Ok(body) = std::env::var(CHILD_ENV) {
        assert_eq!(body, "confirm");
        let answer = mustel::ui::prompts::confirm("Continuar?", true).expect("prompt");
        println!("ANSWER={answer}");
        return;
    }

    let mut session =
        spawn(child_command("confirm_denies_with_n", "confirm")).expect("spawn under a pty");
    session.send_line("n").expect("send n");
    session
        .expect("ANSWER=false")
        .expect("prompt must read n as false");
}

#[test]
fn password_is_masked_and_returned_intact() {
    if let Ok(body) = std::env::var(CHILD_ENV) {
        assert_eq!(body, "password");
        let secret = mustel::ui::prompts::password("Password").expect("prompt");
        println!("SECRET={secret}");
        return;
    }

    let mut session = spawn(child_command(
        "password_is_masked_and_returned_intact",
        "password",
    ))
    .expect("spawn under a pty");
    session.send_line("s3cret").expect("send password");
    session
        .expect("SECRET=s3cret")
        .expect("password must arrive intact");
}

#[test]
fn select_one_moves_with_arrow_keys() {
    if let Ok(body) = std::env::var(CHILD_ENV) {
        assert_eq!(body, "select");
        let options = vec!["dev".to_string(), "prod".to_string()];
        let chosen = mustel::ui::prompts::select_one("Servidor", &options, None).expect("prompt");
        println!("CHOSEN={chosen}");
        return;
    }

    let mut session = spawn(child_command("select_one_moves_with_arrow_keys", "select"))
        .expect("spawn under a pty");
    session.send("\u{1b}[B").expect("arrow down");
    session.send_line("").expect("enter");
    session
        .expect("CHOSEN=prod")
        .expect("second option must be chosen");
}
