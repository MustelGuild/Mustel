//! `database pgpass add` recusa CR/LF/NUL sem tocar o arquivo.
//!
//! O teste de núcleo prova que `PgPassStore::add` recusa; este prova o caminho do usuário: o CLI
//! devolve **2** (argumento inválido) e o `pgpass.conf` nem chega a ser criado.

mod support;

use support::Sandbox;

/// CONTRACT: IF-106
#[test]
fn a_database_with_a_line_break_is_an_argument_error_and_writes_nothing() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&[
        "database".to_string(),
        "pgpass".to_string(),
        "add".to_string(),
        "mydb\n*:*:*:*:injetado".to_string(),
        "--password".to_string(),
        "senha".to_string(),
    ]);

    assert_eq!(output.exit_code, Some(2));
    assert!(
        output.stdout.contains("line breaks"),
        "stdout inesperado: {}",
        output.stdout
    );
    assert!(
        !sandbox.pgpass_file().exists(),
        "o arquivo não pode ser criado"
    );
}

/// CONTRACT: IF-106
#[test]
fn a_password_with_a_line_break_is_an_argument_error_and_writes_nothing() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&[
        "database".to_string(),
        "pgpass".to_string(),
        "add".to_string(),
        "mydb".to_string(),
        "--password".to_string(),
        "senha\n*:*:*:*:injetado".to_string(),
    ]);

    assert_eq!(output.exit_code, Some(2));
    assert!(
        output.stdout.contains("line breaks"),
        "stdout inesperado: {}",
        output.stdout
    );
    assert!(
        !sandbox.pgpass_file().exists(),
        "o arquivo não pode ser criado"
    );
}
