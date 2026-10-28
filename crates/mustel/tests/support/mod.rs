//! Suporte compartilhado dos testes de contrato.
//!
//! Cada caso do baseline (`fixtures/baseline/<slug>.json`) é reexecutado contra o binário atual em um
//! sandbox próprio: a configuração do Mustel e o `pgpass.conf` apontam para um diretório temporário, de
//! modo que nenhum arquivo real do usuário é lido ou escrito.
//!
//! A comparação em si **não** é reimplementada aqui: `mustel_testkit::compare::compare_case` é a
//! única implementação das regras (exit code e artefatos como contrato, texto como N4).
//!
//! Cada arquivo de teste de integração compila este módulo como crate próprio, então nem toda função
//! é usada em todo alvo — daí o `allow(dead_code, unreachable_pub)`.
#![allow(
    dead_code,
    unreachable_pub,
    reason = "módulo de suporte compartilhado pelos alvos de teste; cada alvo usa um subconjunto e o `pub` é necessário para os testes de integração"
)]

use std::path::PathBuf;
use std::process::Command;

use mustel_testkit::capture::{CommandCapture, ConfigSeed, PgPassSeed, RunOutput};
use mustel_testkit::config_sandbox::SAMPLE_CONFIG;
use mustel_testkit::paths::baseline_dir;

/// Sandbox de uma execução: diretório temporário com a config e o `pgpass.conf` isolados.
pub struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    /// Cria o sandbox vazio (nem config nem `pgpass.conf` existem ainda).
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temporary directory"),
        }
    }

    /// Diretório de configuração isolado (`MUSTEL_CONFIG_DIR`).
    pub fn config_dir(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    /// Caminho do `mustel.jsonc` do sandbox.
    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("mustel.jsonc")
    }

    /// Caminho do `pgpass.conf` do sandbox (`MUSTEL_PGPASS_FILE`).
    pub fn pgpass_file(&self) -> PathBuf {
        self.dir.path().join("postgresql").join("pgpass.conf")
    }

    /// Deixa a configuração no estado que o caso do baseline registrou.
    pub fn seed_config(&self, seed: ConfigSeed) {
        let path = self.config_file();
        match seed {
            ConfigSeed::Unchanged => {}
            ConfigSeed::Absent => {
                let _ = std::fs::remove_file(&path);
            }
            ConfigSeed::Fixture => {
                std::fs::create_dir_all(self.config_dir()).expect("config dir");
                std::fs::write(&path, SAMPLE_CONFIG).expect("seed config");
            }
        }
    }

    /// Deixa o `pgpass.conf` no estado que o caso do baseline registrou.
    pub fn seed_pgpass(&self, seed: PgPassSeed) {
        mustel_testkit::pgpass_sandbox::apply_seed(&self.pgpass_file(), seed).expect("seed pgpass");
    }

    /// Roda o binário do Mustel dentro do sandbox.
    pub fn run(&self, argv: &[String]) -> RunOutput {
        let output = Command::new(env!("CARGO_BIN_EXE_mustel"))
            .args(argv)
            .env("MUSTEL_CONFIG_DIR", self.config_dir())
            .env("MUSTEL_PGPASS_FILE", self.pgpass_file())
            .output()
            .expect("mustel binary must run");

        RunOutput {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Carrega uma captura do baseline pelo slug.
pub fn load_capture(slug: &str) -> CommandCapture {
    let path = baseline_dir().join(format!("{slug}.json"));
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("baseline {} não pôde ser lido: {error}", path.display()));
    serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("baseline {} inválido: {error}", path.display()))
}

/// Slugs de todas as capturas do baseline, em ordem.
pub fn baseline_slugs() -> Vec<String> {
    let mut slugs = Vec::new();
    for entry in std::fs::read_dir(baseline_dir()).expect("baseline directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            slugs.push(stem.to_string());
        }
    }
    slugs.sort();
    slugs
}

/// Executa um caso do baseline e devolve a comparação contra o binário atual.
pub fn replay(slug: &str) -> mustel_testkit::compare::CaseComparison {
    let capture = load_capture(slug);
    let sandbox = Sandbox::new();
    sandbox.seed_config(capture.config_seed);
    sandbox.seed_pgpass(capture.pgpass_seed);
    let output = sandbox.run(&capture.argv);

    let config = sandbox.config_file();
    let pgpass = sandbox.pgpass_file();
    mustel_testkit::compare::compare_case(
        &capture,
        &output,
        mustel_testkit::compare::MustelArtifacts {
            config: Some(&config),
            pgpass: Some(&pgpass),
        },
    )
}

/// Renderiza a comparação para a mensagem de falha do teste.
pub fn describe(comparison: &mustel_testkit::compare::CaseComparison) -> String {
    let mut text = format!("caso '{}' falhou:\n", comparison.slug);
    for check in &comparison.checks {
        text.push_str(&format!(
            "  [{:?}] {}: {}\n",
            check.level, check.name, check.detail
        ));
    }
    text
}
