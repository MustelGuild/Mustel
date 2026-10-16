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

/// Todos os comandos do escopo, na ordem em que o oráculo os expõe.
const SCOPE_COMMANDS: &[&[&str]] = &[
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
    for path in SCOPE_COMMANDS {
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("not implemented yet"));

    let output = mustel(&["database", "pgpass", "add", "--database", "mydb"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stub must run for the option form"
    );

    // As duas formas juntas são ambíguas e devem ser recusadas pelo parser.
    // O oráculo devolve `1` para erro de uso (T9) — e a mensagem vai para stdout.
    let output = mustel(&["database", "pgpass", "add", "mydb", "--database", "other"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Error:"));
}

/// Invocações mínimas válidas de cada comando **ainda não migrado**.
///
/// Diferente de [`SCOPE_COMMANDS`], aqui os posicionais obrigatórios são preenchidos — sem isso o
/// clap falha antes de chegar ao stub (erro de uso, exit `1`, como o oráculo).
///
/// Ao migrar um comando, remova-o desta lista: ele deixa de ser stub e passa a ter comportamento
/// próprio (ver `settings db-servers ls`, migrado no T16).
const STUB_INVOCATIONS: &[&[&str]] = &[
    &["query", "run"],
    &["database", "backup"],
    &["database", "restore"],
    &["database", "pgpass", "add"],
    &["database", "pgpass", "list"],
    &["database", "pgpass", "remove", "mydb"],
    &["settings", "db-servers", "rm"],
    &["settings", "db-servers", "test"],
    &["settings", "db-servers", "set-password"],
];

/// Comandos já migrados: precisam executar de verdade (sem "not implemented").
const IMPLEMENTED_COMMANDS: &[&[&str]] = &[
    &["settings", "db-servers", "ls"],
    &["settings", "db-servers", "add"],
];

#[test]
fn migrated_commands_run_instead_of_reporting_not_implemented() {
    for path in IMPLEMENTED_COMMANDS {
        let output = mustel(path);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("not implemented yet"),
            "`mustel {}` já foi migrado e não pode ser stub: {stdout}",
            path.join(" ")
        );
    }
}

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
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("not implemented yet"),
            "stub must say it is not implemented: {stdout}"
        );
    }
}

#[test]
fn missing_required_argument_follows_the_oracle() {
    // `database pgpass remove` exige <database>. O oráculo (T9) devolve `1` e escreve a mensagem em
    // stdout — o clap devolveria `2` em stderr, então o `main` remapeia (ver `handle_parse_error`).
    let output = mustel(&["database", "pgpass", "remove"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "o oráculo não escreve erro em stderr"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("required arguments") || stdout.contains("Error:"),
        "mensagem de erro de uso em stdout: {stdout}"
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
