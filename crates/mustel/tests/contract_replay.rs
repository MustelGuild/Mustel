//! Camada 1 do contrato: replay das capturas do oráculo dentro de `cargo test`.
//!
//! Cada caso de `fixtures/baseline/` vira um teste nomeado. A comparação usa
//! `mustel_testkit::compare::compare_case` — a **mesma** implementação que o harness usa — contra o
//! binário atual, em um sandbox isolado. Nenhum passo precisa do binário .NET.
//!
//! Regras: `Fail` reprova (violação de N1–N3) e `Skipped` **também** reprova (CD7 — o motivo de
//! existir `Skipped` era o stub de comando não migrado, que não existe mais).
//!
//! Dois casos do baseline são exclusivos por divergência deliberada: eles não são replay, e sim
//! asserções explícitas do comportamento Mustel (CD8), no fim deste arquivo.

mod support;

use mustel_testkit::compare::Level;

/// Verifica um caso do baseline.
fn assert_replay(slug: &str) {
    let comparison = support::replay(slug);
    let worst = comparison.worst();
    assert!(
        worst == Level::Pass || worst == Level::Warn,
        "{}",
        support::describe(&comparison)
    );
}

/// Gera um `#[test]` por captura, com nome derivado do slug.
macro_rules! replay_cases {
    ($($name:ident => $slug:literal),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                assert_replay($slug);
            }
        )*
    };
}

replay_cases! {
    replay_database_pgpass_add_appends_to_a_file_without_final_newline => "database-pgpass-add-appends-to-a-file-without-final-newline",
    replay_database_pgpass_add_duplicate => "database-pgpass-add-duplicate",
    replay_database_pgpass_add_new => "database-pgpass-add-new",
    replay_database_pgpass_add_wildcards => "database-pgpass-add-wildcards",
    replay_database_pgpass_list_empty => "database-pgpass-list-empty",
    replay_database_pgpass_list_seeded => "database-pgpass-list-seeded",
    replay_database_pgpass_remove_not_found => "database-pgpass-remove-not-found",
    replay_database_pgpass_remove_ok => "database-pgpass-remove-ok",
    replay_database_pgpass_remove_wildcard_host => "database-pgpass-remove-wildcard-host",
    replay_error_database_backup_invalid_host => "error-database-backup-invalid-host",
    replay_error_database_backup_invalid_port => "error-database-backup-invalid-port",
    replay_error_database_backup_invalid_username => "error-database-backup-invalid-username",
    replay_error_database_backup_missing_database => "error-database-backup-missing-database",
    replay_error_database_restore_file_not_found => "error-database-restore-file-not-found",
    replay_error_database_restore_invalid_ssl_mode => "error-database-restore-invalid-ssl-mode",
    replay_error_database_restore_path_traversal => "error-database-restore-path-traversal",
    replay_error_db_servers_rm_unknown => "error-db-servers-rm-unknown",
    replay_error_missing_required_argument => "error-missing-required-argument",
    replay_error_pgpass_add_invalid_host => "error-pgpass-add-invalid-host",
    replay_error_pgpass_add_invalid_port => "error-pgpass-add-invalid-port",
    replay_error_pgpass_add_missing_database => "error-pgpass-add-missing-database",
    replay_error_query_input_missing => "error-query-input-missing",
    replay_error_unknown_command => "error-unknown-command",
    replay_error_unknown_option => "error-unknown-option",
    replay_help_database_backup => "help-database-backup",
    replay_help_database_pgpass_add => "help-database-pgpass-add",
    replay_help_database_pgpass_list => "help-database-pgpass-list",
    replay_help_database_pgpass_remove => "help-database-pgpass-remove",
    replay_help_database_pgpass => "help-database-pgpass",
    replay_help_database_restore => "help-database-restore",
    replay_help_database => "help-database",
    replay_help_query_run => "help-query-run",
    replay_help_query => "help-query",
    replay_help_root => "help-root",
    replay_help_settings_db_servers_add => "help-settings-db-servers-add",
    replay_help_settings_db_servers_ls => "help-settings-db-servers-ls",
    replay_help_settings_db_servers_rm => "help-settings-db-servers-rm",
    replay_help_settings_db_servers_set_password => "help-settings-db-servers-set-password",
    replay_help_settings_db_servers_test => "help-settings-db-servers-test",
    replay_help_settings_db_servers => "help-settings-db-servers",
    replay_help_settings => "help-settings",
    replay_settings_db_servers_add_flags => "settings-db-servers-add-flags",
    replay_settings_db_servers_ls_empty_config => "settings-db-servers-ls-empty-config",
    replay_settings_db_servers_ls_seeded => "settings-db-servers-ls-seeded",
    replay_settings_db_servers_rm_ok => "settings-db-servers-rm-ok",
}

/// Toda captura do diretório precisa estar listada acima.
///
/// Sem isto, um caso novo no baseline ficaria fora da suíte sem ninguém notar.
#[test]
fn every_baseline_capture_has_a_replay_test() {
    let slugs = support::baseline_slugs();
    let expected: Vec<&str> = slugs
        .iter()
        .map(String::as_str)
        .filter(|slug| !DIVERGENT.contains(slug))
        .collect();
    assert_eq!(
        expected.len(),
        REPLAYED.len(),
        "baseline tem {} casos comparáveis e a suíte lista {}",
        expected.len(),
        REPLAYED.len()
    );
    for slug in expected {
        assert!(
            REPLAYED.contains(&slug),
            "o caso '{slug}' do baseline não tem teste de replay"
        );
    }
}

/// Casos que **não** são replay: divergência deliberada, testados logo abaixo.
const DIVERGENT: &[&str] = &[
    "error-database-restore-file-missing",
    "error-db-servers-test-offline",
];

/// Slugs efetivamente cobertos por `replay_cases!` (mantido em sincronia pelo teste acima + pelo
/// número de testes gerados).
const REPLAYED: &[&str] = &[
    "database-pgpass-add-appends-to-a-file-without-final-newline",
    "database-pgpass-add-duplicate",
    "database-pgpass-add-new",
    "database-pgpass-add-wildcards",
    "database-pgpass-list-empty",
    "database-pgpass-list-seeded",
    "database-pgpass-remove-not-found",
    "database-pgpass-remove-ok",
    "database-pgpass-remove-wildcard-host",
    "error-database-backup-invalid-host",
    "error-database-backup-invalid-port",
    "error-database-backup-invalid-username",
    "error-database-backup-missing-database",
    "error-database-restore-file-not-found",
    "error-database-restore-invalid-ssl-mode",
    "error-database-restore-path-traversal",
    "error-db-servers-rm-unknown",
    "error-missing-required-argument",
    "error-pgpass-add-invalid-host",
    "error-pgpass-add-invalid-port",
    "error-pgpass-add-missing-database",
    "error-query-input-missing",
    "error-unknown-command",
    "error-unknown-option",
    "help-database-backup",
    "help-database-pgpass-add",
    "help-database-pgpass-list",
    "help-database-pgpass-remove",
    "help-database-pgpass",
    "help-database-restore",
    "help-database",
    "help-query-run",
    "help-query",
    "help-root",
    "help-settings-db-servers-add",
    "help-settings-db-servers-ls",
    "help-settings-db-servers-rm",
    "help-settings-db-servers-set-password",
    "help-settings-db-servers-test",
    "help-settings-db-servers",
    "help-settings",
    "settings-db-servers-add-flags",
    "settings-db-servers-ls-empty-config",
    "settings-db-servers-ls-seeded",
    "settings-db-servers-rm-ok",
];

// ---------------------------------------------------------------------------------------------------
// T4: sanidade do sandbox
// ---------------------------------------------------------------------------------------------------

/// O sandbox isola os dois caminhos de ambiente e não toca arquivo do usuário.
#[test]
fn support_isolates_the_environment() {
    let sandbox = support::Sandbox::new();
    let config_dir = sandbox.config_dir();
    let pgpass = sandbox.pgpass_file();

    assert!(
        config_dir.starts_with(std::env::temp_dir()),
        "MUSTEL_CONFIG_DIR deveria ficar no temporário, veio {}",
        config_dir.display()
    );
    assert!(
        pgpass.starts_with(std::env::temp_dir()),
        "MUSTEL_PGPASS_FILE deveria ficar no temporário, veio {}",
        pgpass.display()
    );

    // Um comando que só lê o cadastro: exercita o isolamento sem tocar rede.
    let output = sandbox.run(&[
        "settings".to_string(),
        "db-servers".to_string(),
        "ls".to_string(),
    ]);
    assert_eq!(output.exit_code, Some(0));
    assert!(
        sandbox.config_file().starts_with(&config_dir),
        "o arquivo de config precisa cair dentro do sandbox"
    );
}

// ---------------------------------------------------------------------------------------------------
// T6: divergências deliberadas — asserção explícita do comportamento Mustel (CD8)
// ---------------------------------------------------------------------------------------------------

/// `error-database-restore-file-missing`: no legado o comando quebrava no prompt de senha antes de
/// validar o arquivo (e em modo não-interativo morria com "failed to read input"). O Mustel resolve a
/// senha pela via normal e, **tendo** senha, valida o arquivo e falha com exit 2 e o caminho absoluto.
/// Divergência deliberada registrada em `fixtures/known-divergences.md` item 11.
///
/// A senha vai por `-W` de propósito: sem ela o comando pararia no prompt, que é o caminho que o
/// harness não consegue comparar (e que o teste abaixo cobre explicitamente).
#[test]
fn divergent_restore_missing_file_is_a_clear_usage_error() {
    let sandbox = support::Sandbox::new();
    sandbox.seed_config(mustel_testkit::capture::ConfigSeed::Absent);
    let output = sandbox.run(&[
        "database".to_string(),
        "restore".to_string(),
        "db".to_string(),
        "nao-existe.dump".to_string(),
        "-W".to_string(),
        "x".to_string(),
    ]);

    assert_eq!(output.exit_code, Some(2), "stdout: {}", output.stdout);
    assert!(
        output.stdout.contains("nao-existe.dump"),
        "a mensagem deve citar o arquivo: {}",
        output.stdout
    );
    assert!(
        output.stdout.contains("File not found"),
        "mensagem do legado para arquivo ausente: {}",
        output.stdout
    );
    assert!(
        output.stderr.is_empty(),
        "o contrato escreve erro em stdout: {}",
        output.stderr
    );
}

/// `error-db-servers-test-offline`: no legado o `test` sem senha gravada quebrava no prompt
/// ("failed to read input in non-interactive mode"). O Mustel falha claro: sem terminal e sem senha
/// no cofre, ele **não trava** e sai 40 explicando o que fazer.
///
/// Divergência deliberada registrada em `known-divergences.md` item 13. O que este teste prende é o
/// "não trava" — se alguém reintroduzir um prompt bloqueante, o processo do teste nunca retorna.
#[test]
fn divergent_test_offline_fails_without_prompting() {
    let sandbox = support::Sandbox::new();
    sandbox.seed_config(mustel_testkit::capture::ConfigSeed::Fixture);
    let output = sandbox.run(&[
        "settings".to_string(),
        "db-servers".to_string(),
        "test".to_string(),
        "-n".to_string(),
        "offline".to_string(),
    ]);

    assert_eq!(output.exit_code, Some(40), "stdout: {}", output.stdout);
    assert!(
        output.stdout.contains("interactive terminal"),
        "deve explicar que falta terminal em vez de travar: {}",
        output.stdout
    );
    assert!(
        !output.stdout.contains("failed to read input"),
        "não deve reproduzir a quebra do legado: {}",
        output.stdout
    );
}
