//! Camada 2 do contrato: o texto onde o texto **é** o entregável.
//!
//! A camada 1 trata stdout/stderr como aviso N4 (a redação difere por framework). Aqui não: as
//! mensagens que o usuário lê e das quais depende (erro de uso, confirmação de sucesso, orientação) são
//! pinadas em snapshot. Deriva de redação passa a aparecer como diff, em vez de passar em silêncio.
//!
//! Normalização obrigatória antes de snapshotar: `\r\n` → `\n` (o Windows quebra linha diferente) e
//! `mustel.exe` → `mustel` (o nome do executável vem do `argv[0]`, não do contrato).

mod support;

/// Normaliza o texto para o snapshot: fim de linha e nome do binário.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace("mustel.exe", "mustel")
}

/// Roda um comando no sandbox e devolve `(exit, stdout normalizado, stderr normalizado)`.
fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let sandbox = support::Sandbox::new();
    sandbox.seed_config(mustel_testkit::capture::ConfigSeed::Absent);
    let command_args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let output = sandbox.run(&command_args);
    (
        output.exit_code,
        normalize(&output.stdout),
        normalize(&output.stderr),
    )
}

/// IF-001: `mustel` sem subcomando e sem TTY imprime a ajuda e sai 0 (D9 + divergência 18).
///
/// Com terminal interativo o binário abre a GUI; essa metade é `nao-coberta` no manifesto, porque
/// abrir janela não é verificável em teste. O que **é** contrato é este caminho: script, pipe, CI.
/// CONTRACT: IF-001
#[test]
fn root_without_a_tty_prints_help_and_exits_zero() {
    let (exit, stdout, stderr) = run(&[]);
    assert_eq!(exit, Some(0), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.is_empty(), "a ajuda sai em stdout: {stderr}");
    assert!(stdout.contains("Usage: mustel [COMMAND]"), "{}", stdout);
    assert!(stdout.contains("query"), "{}", stdout);
    insta::assert_snapshot!("root_without_a_tty", stdout);
}

/// IF-004: comando desconhecido é erro de uso, em **stdout**, com exit 1.
/// CONTRACT: IF-004
#[test]
fn unknown_command_is_a_usage_error_on_stdout() {
    let (exit, stdout, stderr) = run(&["nope"]);
    assert_eq!(exit, Some(1));
    assert!(
        stderr.is_empty(),
        "o legado não escreve erro em stderr: {stderr}"
    );
    insta::assert_snapshot!("unknown_command", stdout);
}

/// IF-005: opção desconhecida é erro de uso, em **stdout**, com exit 1.
/// CONTRACT: IF-005
#[test]
fn unknown_option_is_a_usage_error_on_stdout() {
    let (exit, stdout, stderr) = run(&["query", "run", "--nope"]);
    assert_eq!(exit, Some(1));
    assert!(
        stderr.is_empty(),
        "o legado não escreve erro em stderr: {stderr}"
    );
    insta::assert_snapshot!("unknown_option", stdout);
}

/// IF-170: o sucesso do `add` orienta o próximo passo (`set-password`), como no legado.
/// CONTRACT: IF-170
#[test]
fn adding_a_server_explains_the_password_step() {
    let (exit, stdout, _) = run(&[
        "settings",
        "db-servers",
        "add",
        "-n",
        "s1",
        "-H",
        "localhost",
        "-p",
        "5432",
        "-U",
        "postgres",
    ]);
    assert_eq!(exit, Some(0));
    insta::assert_snapshot!("server_added", stdout);
}

/// IF-171: a mensagem de binário ausente do `backup` é a do legado e sai em stdout.
///
/// O texto é produzido por `usecases::database::binary_not_found`; aqui pinamos a redação que o
/// usuário vê. O exit code correspondente é o IF-152.
/// CONTRACT: IF-171
#[test]
fn missing_binary_message_is_the_legacy_one() {
    let message = mustel_core::usecases::database::binary_not_found("pg_dump");
    insta::assert_snapshot!("missing_binary", message);
}

/// IF-172: `restore --all` sem nenhum dump avisa e tem sucesso (exit 0), sem abortar.
/// CONTRACT: IF-172
#[test]
fn restore_all_with_an_empty_directory_warns_and_succeeds() {
    let sandbox = support::Sandbox::new();
    sandbox.seed_config(mustel_testkit::capture::ConfigSeed::Absent);
    let dir = sandbox
        .config_dir()
        .parent()
        .expect("sandbox pai")
        .join("vazio");
    std::fs::create_dir_all(&dir).expect("diretório vazio");

    let output = sandbox.run(&[
        "database".to_string(),
        "restore".to_string(),
        "-a".to_string(),
        "-d".to_string(),
        dir.display().to_string(),
        "-W".to_string(),
        "x".to_string(),
    ]);

    assert_eq!(output.exit_code, Some(0), "stdout: {}", output.stdout);
    insta::assert_snapshot!("restore_all_empty", normalize(&output.stdout));
}

/// IF-173: o `pgpass add` confirma a entrada com o formato `host:porta:banco:usuario`.
/// CONTRACT: IF-173
#[test]
fn pgpass_add_confirms_the_entry_identity() {
    let (exit, stdout, _) = run(&["database", "pgpass", "add", "mydb", "--password", "s"]);
    assert_eq!(exit, Some(0));
    insta::assert_snapshot!("pgpass_add", stdout);
}

/// IF-113/IF-164: config vazia orienta o `add` e cita o binário novo (`mustel`, não `fur`).
/// CONTRACT: IF-164
#[test]
fn empty_config_guidance_names_the_mustel_binary() {
    let (exit, stdout, _) = run(&["settings", "db-servers", "ls"]);
    assert_eq!(exit, Some(0));
    assert!(
        stdout.contains("mustel settings db-servers add"),
        "{}",
        stdout
    );
    assert!(
        !stdout.contains("fur "),
        "o nome antigo não pode aparecer: {stdout}"
    );
    insta::assert_snapshot!("empty_config_guidance", stdout);
}

/// IF-051: sem banco e sem `--all`, a mensagem é a do legado e o exit é 2.
/// CONTRACT: IF-051
#[test]
fn backup_without_database_explains_the_requirement() {
    let (exit, stdout, _) = run(&["database", "backup"]);
    assert_eq!(exit, Some(2));
    insta::assert_snapshot!("backup_without_database", stdout);
}
