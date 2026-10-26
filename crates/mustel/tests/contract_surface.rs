//! Prova de cobertura de superfície — o portão da remoção do legado.
//!
//! Responde a duas perguntas que, sem teste, seriam opinião:
//!
//! 1. Todo comando migrado tem pelo menos um caso no baseline?
//! 2. Toda flag longa que o `--help` expõe é exercitada por algum caso do baseline?
//!
//! O que não dá para exercitar no harness fica em [`EXCEPTIONS`], com o motivo escrito. A lista é
//! impressa no teste para que quem revisa veja exatamente o que está fora.

mod support;

use std::collections::BTreeSet;

/// Comandos migrados: os 11 do escopo.
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

/// Flags aceitas que nenhum caso do baseline exercita, com o motivo.
///
/// Regra: só entra aqui o que é **impossível ou inútil** exercitar no harness. Qualquer outra flag
/// precisa de caso — é isso que impede a remoção do legado de perder comportamento.
const EXCEPTIONS: &[(&str, &str)] = &[
    (
        "--pooling",
        "aceita por paridade de superfície; o pool não está implementado (pendência de release), então não há efeito a observar",
    ),
    (
        "--min-pool-size",
        "idem --pooling: sem pool, sem efeito observável",
    ),
    (
        "--max-pool-size",
        "idem --pooling: sem pool, sem efeito observável",
    ),
    (
        "--keepalive",
        "idem --pooling: vai para o socket quando o pool existir; hoje não muda a saída",
    ),
    (
        "--connection-lifetime",
        "idem --pooling: sem pool, sem efeito observável",
    ),
    (
        "--fetch-all",
        "exige um Postgres vivo para descobrir bancos; coberto pela suíte gated de integração",
    ),
    (
        "--max-parallelism",
        "exige múltiplos bancos vivos; coberto pela suíte gated de integração",
    ),
    (
        "--create-if-not-exists",
        "exige banco alvo ausente num Postgres vivo; coberto pela suíte gated de integração",
    ),
    (
        "--single-transaction",
        "exige um dump real para restaurar; coberto pela suíte gated de integração",
    ),
    (
        "--jobs",
        "exige um dump real; coberto pela suíte gated de integração",
    ),
    (
        "--no-owner",
        "exige um dump real; coberto pela suíte gated de integração",
    ),
    (
        "--no-acl",
        "exige um dump real; coberto pela suíte gated de integração",
    ),
    (
        "--no-clean",
        "exige um dump real; coberto pela suíte gated de integração",
    ),
    (
        "--interactive",
        "exige terminal interativo (N3), coberto pelos testes de PTY",
    ),
    (
        "--verbose",
        "só muda a saída do `pg_dump`/`pg_restore`; exige servidor e binário reais",
    ),
    (
        "--exclude-table-data",
        "vai para o argv do `pg_dump`; o argv é testado na unidade, e executar exige servidor",
    ),
    (
        "--timeout",
        "aceito e usado no driver; o valor não muda a saída de um caso determinístico",
    ),
    ("--command-timeout", "idem --timeout"),
    (
        "--ssl-mode",
        "muda o handshake; testado na unidade (o modo vira configuração do driver)",
    ),
];

/// Flags exercitadas por testes que **não** são casos do baseline (unidade ou CLI), com o teste que
/// as prende.
///
/// Existe porque a captura do oráculo cobre o comportamento determinístico, não a superfície inteira:
/// um caso de baseline roda nos **dois** binários, então flags que só fazem sentido com servidor vivo
/// nunca entram nele. Aqui a flag é ligada ao teste que a exerce — e o teste é verificado.
const UNIT_COVERED: &[(&str, &str)] = &[
    (
        "--input",
        "crates/mustel/tests/cli_tree.rs::flags_follow_the_legacy_contract",
    ),
    (
        "--command",
        "crates/mustel/src/commands/query/run.rs::tests::command_and_input_are_mutually_exclusive",
    ),
    (
        "--output",
        "crates/mustel/tests/cli_tree.rs::flags_follow_the_legacy_contract",
    ),
    (
        "--npgsql-connection-string",
        "crates/mustel/src/commands/query/settings.rs::the_connection_string_is_parsed_like_npgsql",
    ),
    (
        "--exclude",
        "crates/mustel-core/src/query/discovery.rs::tests::exclude_names_are_trimmed_case_insensitive_and_ignore_blanks",
    ),
    (
        "--all",
        "crates/mustel/tests/cli_tree.rs::flags_follow_the_legacy_contract",
    ),
    (
        "--format",
        "crates/mustel-core/src/usecases/database_backup.rs::tests::argv_glues_the_format_and_keeps_an_empty_username",
    ),
    (
        "--directory",
        "crates/mustel/tests/cli_tree.rs::flags_follow_the_legacy_contract",
    ),
    (
        "--name",
        "crates/mustel/src/commands/settings/db_servers/add.rs::tests::missing_required_flags_report_the_legacy_text",
    ),
    (
        "--databases",
        "crates/mustel/src/commands/settings/db_servers/add.rs::tests::split_list_trims_and_drops_empties",
    ),
    (
        "--exclude-patterns",
        "crates/mustel/src/commands/settings/db_servers/add.rs::tests::exclude_patterns_fall_back_to_the_defaults",
    ),
    (
        "--csv-escape-formulas",
        "crates/mustel-core/src/query/csv_writer.rs::tests::formula_escape_prefixes_dangerous_fields_only_when_enabled",
    ),
    (
        "--strict",
        "crates/mustel-core/src/usecases/database_backup.rs::tests::strict_only_fails_when_a_database_failed",
    ),
];

/// Flags longas que aparecem em um texto de ajuda.
fn long_flags(help: &str) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    for token in help.split_whitespace() {
        let token = token.trim_end_matches(',');
        if let Some(rest) = token.strip_prefix("--")
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            flags.insert(format!("--{rest}"));
        }
    }
    flags
}

/// Todos os `argv` do baseline.
fn baseline_argvs() -> Vec<Vec<String>> {
    support::baseline_slugs()
        .iter()
        .map(|slug| support::load_capture(slug).argv)
        .collect()
}

/// Caminho do comando exercitado por um `argv`, se for um dos do escopo.
fn command_of(argv: &[String]) -> Option<&'static [&'static str]> {
    SCOPE_COMMANDS.iter().copied().find(|command| {
        argv.len() >= command.len()
            && command
                .iter()
                .zip(argv.iter())
                .all(|(expected, actual)| expected == actual)
    })
}

/// IF-190: todo comando migrado tem pelo menos um caso no baseline.
/// CONTRACT: IF-190
#[test]
fn every_migrated_command_has_a_baseline_case() {
    let argvs = baseline_argvs();
    let mut missing = Vec::new();

    for command in SCOPE_COMMANDS {
        if !argvs.iter().any(|argv| command_of(argv) == Some(*command)) {
            missing.push(command.join(" "));
        }
    }

    assert!(
        missing.is_empty(),
        "comando(s) migrado(s) sem caso no baseline: {missing:?}"
    );
}

/// O `fn` citado existe no arquivo citado?
fn test_exists(reference: &str) -> bool {
    let parts: Vec<&str> = reference.split("::").collect();
    let (Some(file), Some(function)) = (parts.first(), parts.last()) else {
        return false;
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(file);
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.contains(&format!("fn {function}"))
}

/// IF-191: toda flag longa da ajuda é exercitada por um caso ou está declarada em `EXCEPTIONS`.
/// CONTRACT: IF-191
#[test]
fn every_long_flag_is_exercised_or_declared() {
    let argvs = baseline_argvs();
    let exercised: BTreeSet<String> = argvs
        .iter()
        .flat_map(|argv| argv.iter())
        .filter(|arg| arg.starts_with("--"))
        .map(|arg| arg.split('=').next().unwrap_or(arg).to_string())
        .collect();
    let declared: BTreeSet<&str> = EXCEPTIONS.iter().map(|(flag, _)| *flag).collect();
    let unit_covered: BTreeSet<&str> = UNIT_COVERED.iter().map(|(flag, _)| *flag).collect();

    let sandbox = support::Sandbox::new();
    let mut undeclared = Vec::new();

    for command in SCOPE_COMMANDS {
        let mut argv: Vec<String> = command.iter().map(|part| (*part).to_string()).collect();
        argv.push("--help".to_string());
        let output = sandbox.run(&argv);
        assert_eq!(output.exit_code, Some(0), "`--help` de {command:?} falhou");

        for flag in long_flags(&output.stdout) {
            // `--help` e `--version` são do clap e existem em todo comando.
            if flag == "--help" || flag == "--version" {
                continue;
            }
            if exercised.contains(&flag) || declared.contains(flag.as_str()) {
                continue;
            }
            if unit_covered.contains(flag.as_str()) {
                continue;
            }
            undeclared.push(format!("{}: {flag}", command.join(" ")));
        }
    }

    println!(
        "flags não exercitadas pelo baseline: {} declaradas + {} cobertas por teste de unidade/CLI",
        EXCEPTIONS.len(),
        UNIT_COVERED.len()
    );

    // Cada flag "unit-covered" aponta para um teste que precisa existir — senão a lista mente.
    let stale: Vec<&str> = UNIT_COVERED
        .iter()
        .filter(|(_, test)| !test_exists(test))
        .map(|(flag, test)| {
            println!("  {flag} aponta para teste inexistente: {test}");
            *test
        })
        .collect();
    assert!(
        stale.is_empty(),
        "entradas de UNIT_COVERED inválidas: {stale:?}"
    );

    assert!(
        undeclared.is_empty(),
        "flag(s) sem caso no baseline e sem declaração em EXCEPTIONS: {undeclared:?}"
    );
}
