//! Testes da árvore de comandos (superfície N1 do contrato de paridade).
//!
//! Usam `std::process::Command` com `CARGO_BIN_EXE_mustel` em vez de `assert_cmd` — menos uma
//! dependência e sem API depreciada.

use std::process::{Command, Output};

fn mustel(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mustel"))
        .args(args)
        .output()
        .expect("binary must run")
}

fn stdout_of(args: &[&str]) -> String {
    let output = mustel(args);
    assert!(
        output.status.success(),
        "`mustel {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Todos os comandos do escopo migrado, na ordem em que o oráculo os expõe.
const MIGRATED_COMMANDS: &[&[&str]] = &[
    &["query", "run"],
    &["database", "backup"],
    &["database", "restore"],
    &["database", "pgpass", "add"],
    &["database", "pgpass", "list"],
    &["database", "pgpass", "remove"],
    &["settings", "db-servers", "ls"],
    &["settings", "db-servers", "add"],
    &["settings", "db-servers", "rm"],
    &["settings", "db-servers", "test"],
    &["settings", "db-servers", "set-password"],
];

#[test]
fn root_help_lists_the_migrated_groups_and_the_gui() {
    let help = stdout_of(&["--help"]);
    for group in ["query", "database", "settings", "gui"] {
        assert!(
            help.contains(group),
            "root help must list `{group}`:\n{help}"
        );
    }
}

#[test]
fn every_migrated_command_renders_its_own_help() {
    for path in MIGRATED_COMMANDS {
        let mut args = path.to_vec();
        args.push("--help");
        let help = stdout_of(&args);
        assert!(
            help.contains("Usage") && help.contains("mustel"),
            "`mustel {}` must render usage:\n{help}",
            path.join(" ")
        );
    }
}

#[test]
fn flags_follow_the_legacy_contract() {
    let query_help = stdout_of(&["query", "run", "--help"]);
    for flag in [
        "-i, --input",
        "-c, --command",
        "-o, --output",
        "--npgsql-connection-string",
        "-H, --host",
        "-p, --port",
        "-d, --database",
        "-U, --username",
        "-W, --password",
        "--ssl-mode",
        "--command-timeout",
        "--pooling",
        "--max-pool-size",
        "-a, --all",
        "--exclude",
        "--no-confirm",
    ] {
        assert!(
            query_help.contains(flag),
            "missing `{flag}` in query run:\n{query_help}"
        );
    }

    let backup_help = stdout_of(&["database", "backup", "--help"]);
    for flag in ["--exclude-table-data", "-F, --format", "-v, --verbose"] {
        assert!(
            backup_help.contains(flag),
            "missing `{flag}` in database backup"
        );
    }

    let restore_help = stdout_of(&["database", "restore", "--help"]);
    for flag in [
        "--no-clean",
        "--no-owner",
        "--no-acl",
        "--single-transaction",
        "-j, --jobs",
        "-d, --directory",
    ] {
        assert!(
            restore_help.contains(flag),
            "missing `{flag}` in database restore"
        );
    }

    let pgpass_add_help = stdout_of(&["database", "pgpass", "add", "--help"]);
    for flag in [
        "--database",
        "--host",
        "--port",
        "--username",
        "--password",
        "[database]",
    ] {
        assert!(
            pgpass_add_help.contains(flag),
            "missing `{flag}` in pgpass add"
        );
    }

    let db_add_help = stdout_of(&["settings", "db-servers", "add", "--help"]);
    for flag in [
        "-n, --name",
        "-i, --interactive",
        "-H, --host",
        "-p, --port",
        "-U, --username",
        "-W, --password",
        "-d, --databases",
        "--max-parallelism",
        "--fetch-all",
        "--exclude-patterns",
    ] {
        assert!(
            db_add_help.contains(flag),
            "missing `{flag}` in db-servers add"
        );
    }
}

#[test]
fn pgpass_add_accepts_the_database_as_a_positional_argument() {
    // Decisão do usuário: `pgpass add <banco>` (documentado) é a forma correta, e `--database` fica
    // por compatibilidade. Aqui só se verifica que a árvore aceita as duas formas sem erro de uso
    // (o comando em si ainda é stub).
    let output = mustel(&["database", "pgpass", "add", "mydb"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stub must run for the positional form"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented yet"));

    let output = mustel(&["database", "pgpass", "add", "--database", "mydb"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stub must run for the option form"
    );

    // As duas formas juntas são ambíguas e devem ser recusadas pelo parser (exit 2).
    let output = mustel(&["database", "pgpass", "add", "mydb", "--database", "other"]);
    assert_eq!(output.status.code(), Some(2));
}

/// Invocações mínimas válidas de cada comando ainda não migrado.
///
/// Diferente de [`MIGRATED_COMMANDS`], aqui os posicionais obrigatórios são preenchidos — sem isso o
/// clap falha antes de chegar ao stub (erro de uso, exit `2`), que é justamente o comportamento
/// esperado de um erro de parse.
const STUB_INVOCATIONS: &[&[&str]] = &[
    &["query", "run"],
    &["database", "backup"],
    &["database", "restore"],
    &["database", "pgpass", "add"],
    &["database", "pgpass", "list"],
    &["database", "pgpass", "remove", "mydb"],
    &["settings", "db-servers", "ls"],
    &["settings", "db-servers", "add"],
    &["settings", "db-servers", "rm"],
    &["settings", "db-servers", "test"],
    &["settings", "db-servers", "set-password"],
];

#[test]
fn not_yet_migrated_commands_fail_loudly_with_exit_code_one() {
    for path in STUB_INVOCATIONS {
        let output = mustel(path);
        assert_eq!(
            output.status.code(),
            Some(1),
            "`mustel {}` stub must exit 1",
            path.join(" ")
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("not implemented yet"),
            "stub must say it is not implemented: {stderr}"
        );
    }
}

#[test]
fn missing_required_argument_is_a_usage_error() {
    // `database pgpass remove` exige <database>; o clap devolve 2 (erro de uso).
    // O código real do oráculo para erro de parse é verificado no T9 — Spectre pode usar outro valor.
    let output = mustel(&["database", "pgpass", "remove"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required arguments"),
        "clap must explain the missing argument: {stderr}"
    );
}

#[test]
fn version_uses_the_workspace_version() {
    let output = stdout_of(&["--version"]);
    assert!(output.contains("mustel 1.0.0"), "got {output}");
}

#[test]
fn root_help_snapshot() {
    insta::assert_snapshot!(stdout_of(&["--help"]));
}

#[test]
fn query_run_help_snapshot() {
    insta::assert_snapshot!(stdout_of(&["query", "run", "--help"]));
}
