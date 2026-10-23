//! Compara o Mustel com o baseline do oráculo e gera `oracle/reports/<ts>/report.md`.
//!
//! Uso:
//! ```text
//! cargo run -p mustel-testkit --bin oracle-compare             # todos os casos do baseline
//! cargo run -p mustel-testkit --bin oracle-compare -- --scope settings
//! ```
//!
//! Sai com código != 0 quando há violação de contrato (N1–N3).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use mustel_testkit::capture::{ConfigSeed, RunOutput};
use mustel_testkit::compare::{
    CaseComparison, Check, Level, MIGRATED_COMMANDS, MustelArtifacts, compare_case,
};
use mustel_testkit::config_sandbox::SAMPLE_CONFIG;
use mustel_testkit::paths::{
    baseline_dir, mustel_binary, mustel_config_dir, mustel_config_file, mustel_pgpass_file,
    workspace_dir,
};
use mustel_testkit::pgpass_sandbox::apply_seed;
use mustel_testkit::report::{Summary, render};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let scope = arg_value(&args, "--scope");

    let binary = mustel_binary()
        .context("mustel binary not found; run `cargo build -p mustel` or set MUSTEL_BIN")?;
    println!("mustel: {}", binary.display());

    let workspace = workspace_dir();
    std::fs::create_dir_all(&workspace)?;
    let config_dir = mustel_config_dir();
    std::fs::create_dir_all(&config_dir)?;
    let pgpass_file = mustel_pgpass_file();

    let baseline = baseline_dir();
    if !baseline.is_dir() {
        bail!(
            "baseline not found at {}; run oracle-capture first",
            baseline.display()
        );
    }

    let mut comparisons = Vec::new();
    let mut total = 0usize;

    for entry in std::fs::read_dir(&baseline)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let json = std::fs::read_to_string(&path)?;
        let capture: mustel_testkit::capture::CommandCapture =
            serde_json::from_str(&json).with_context(|| format!("parse {}", path.display()))?;

        if let Some(scope) = &scope
            && !capture.slug.contains(scope.as_str())
        {
            continue;
        }
        total += 1;

        if !should_compare_behaviour(&capture.slug) {
            let detail = deliberate_divergence(&capture.slug)
                .unwrap_or("comando ainda não migrado (stub)")
                .to_string();
            comparisons.push(CaseComparison {
                slug: capture.slug.clone(),
                checks: vec![Check {
                    name: "comportamento".to_string(),
                    level: Level::Skipped,
                    detail,
                }],
            });
            continue;
        }

        seed_mustel_config(capture.config_seed)?;
        apply_seed(&pgpass_file, capture.pgpass_seed)
            .with_context(|| format!("seed mustel pgpass for {}", capture.slug))?;
        let argv: Vec<&str> = capture.argv.iter().map(String::as_str).collect();
        let output = run_mustel(&binary, &workspace, &config_dir, &pgpass_file, &argv)?;

        comparisons.push(compare_case(
            &capture,
            &output,
            MustelArtifacts {
                config: Some(&mustel_config_file()),
                pgpass: Some(&pgpass_file),
            },
        ));
    }

    let generated_at = timestamp();
    let report = render(&comparisons, &generated_at);
    let report_dir = workspace
        .parent()
        .unwrap_or(Path::new("."))
        .join("reports")
        .join(&generated_at);
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("report.md");
    std::fs::write(&report_path, &report)?;

    let summary = Summary::of(&comparisons);
    println!(
        "casos: {total} | passou: {} | avisos N4: {} | falhas: {} | pulados: {}",
        summary.passed, summary.warned, summary.failed, summary.skipped
    );
    println!("relatório: {}", report_path.display());

    if summary.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

/// Casos de ajuda e de erro determinístico sempre são comparados; os demais só quando o comando
/// correspondente já foi migrado (caso contrário o stub do Mustel daria diferença legítima).
fn should_compare_behaviour(slug: &str) -> bool {
    if slug.starts_with("help-") || is_deterministic_error(slug) {
        return true;
    }
    migrated_command_for_case(slug).is_some()
}

/// Caminho do comando que um caso exercita, quando ele já está migrado.
///
/// Dois formatos de slug convivem no baseline: casos nomeados a partir do caminho do comando
/// (`settings-db-servers-rm-ok`, `settings-db-servers-ls-seeded`) e casos de erro nomeados
/// descritivamente, cujo exit code vem do **corpo** do comando (`error-db-servers-rm-unknown`).
/// Sem este mapa, o erro do `rm` continuaria "pulado" mesmo com o comando migrado.
fn migrated_command_for_case(slug: &str) -> Option<&'static [&'static str]> {
    const ERROR_CASE_COMMANDS: &[(&str, &[&str])] = &[
        (
            "error-db-servers-rm-unknown",
            &["settings", "db-servers", "rm"],
        ),
        ("error-query-input-missing", &["query", "run"]),
        (
            "error-pgpass-add-missing-database",
            &["database", "pgpass", "add"],
        ),
        (
            "error-pgpass-add-invalid-host",
            &["database", "pgpass", "add"],
        ),
        (
            "error-pgpass-add-invalid-port",
            &["database", "pgpass", "add"],
        ),
        (
            "error-database-backup-missing-database",
            &["database", "backup"],
        ),
        (
            "error-database-backup-invalid-host",
            &["database", "backup"],
        ),
        (
            "error-database-backup-invalid-port",
            &["database", "backup"],
        ),
        (
            "error-database-backup-invalid-username",
            &["database", "backup"],
        ),
        (
            "error-database-restore-file-not-found",
            &["database", "restore"],
        ),
        (
            "error-database-restore-path-traversal",
            &["database", "restore"],
        ),
        (
            "error-database-restore-invalid-ssl-mode",
            &["database", "restore"],
        ),
    ];

    if let Some((_, command)) = ERROR_CASE_COMMANDS.iter().find(|(case, _)| *case == slug) {
        return MIGRATED_COMMANDS.contains(command).then_some(*command);
    }

    MIGRATED_COMMANDS
        .iter()
        .find(|command| {
            let path = command.join("-");
            slug == path || slug.starts_with(&format!("{path}-"))
        })
        .copied()
}

/// Casos que **nunca** serão comparados, com o motivo (divergência deliberada).
fn deliberate_divergence(slug: &str) -> Option<&'static str> {
    match slug {
        "error-database-restore-file-missing" | "error-db-servers-test-offline" => Some(
            "divergência deliberada: sem senha em modo não-interativo o legado quebra no prompt; o \
             Mustel falha claro (ver oracle/known-divergences.md)",
        ),
        _ => None,
    }
}

/// Erros cujo código é decidido **pela camada de parsing/superfície**, não pela implementação do
/// comando — esses podem ser comparados antes de o comando migrar.
///
/// `error-query-input-missing` e `error-db-servers-rm-unknown` ficam de fora de propósito: os exit
/// codes deles (1 e 41) vêm do corpo do comando legado (exceção própria / servidor não encontrado) e
/// só fazem sentido quando o comando correspondente migrar.
fn is_deterministic_error(slug: &str) -> bool {
    matches!(
        slug,
        "error-unknown-command" | "error-unknown-option" | "error-missing-required-argument"
    )
}

/// Deixa a config isolada do Mustel no estado pedido pelo caso.
fn seed_mustel_config(seed: ConfigSeed) -> Result<()> {
    let path = mustel_config_file();
    match seed {
        ConfigSeed::Unchanged => Ok(()),
        ConfigSeed::Absent => match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("remove mustel config"),
        },
        ConfigSeed::Fixture => std::fs::write(&path, SAMPLE_CONFIG).context("seed mustel config"),
    }
}

fn run_mustel(
    binary: &Path,
    cwd: &Path,
    config_dir: &Path,
    pgpass_file: &Path,
    argv: &[&str],
) -> Result<RunOutput> {
    let output = Command::new(binary)
        .args(argv)
        .current_dir(cwd)
        .env("MUSTEL_CONFIG_DIR", config_dir)
        .env("MUSTEL_PGPASS_FILE", pgpass_file)
        .output()
        .with_context(|| format!("failed to run {} {}", binary.display(), argv.join(" ")))?;

    Ok(RunOutput {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1).cloned())
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("run-{seconds}")
}
