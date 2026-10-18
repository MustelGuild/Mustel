//! Execução e registro de uma captura do oráculo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config_sandbox::LegacyConfigSandbox;
use crate::paths::workspace_dir;

/// Placeholder do diretório de trabalho do harness.
pub const WORKDIR_PLACEHOLDER: &str = "<WORKDIR>";
/// Placeholder do caminho da configuração do legado.
pub const LEGACY_CONFIG_PLACEHOLDER: &str = "<LEGACY_CONFIG>";
/// Placeholder do caminho da configuração do Mustel.
pub const MUSTEL_CONFIG_PLACEHOLDER: &str = "<MUSTEL_CONFIG>";
/// Placeholder da raiz do repositório.
pub const REPO_PLACEHOLDER: &str = "<REPO>";
/// Placeholder do diretório pessoal do usuário.
pub const HOME_PLACEHOLDER: &str = "<HOME>";

/// Substitui caminhos específicos da máquina por placeholders.
///
/// Sem isso o baseline versionado carrega `C:\Users\liphv\...` (ou `/home/...`) e deixa de ser
/// comparável entre máquinas e runners de CI. O comparador aplica a mesma função na saída do Mustel.
///
/// Os prefixos de casa e de repositório entram na lista porque o console do legado **quebra a
/// mensagem em 80 colunas**, às vezes no meio do caminho — sem eles sobraria o nome do usuário no
/// texto (o meio do caminho fica em outra linha e não casa com a substituição literal).
pub fn redact(text: &str) -> String {
    let mut result = text.to_string();
    let mut replacements = vec![
        (workspace_dir(), WORKDIR_PLACEHOLDER.to_string()),
        (
            LegacyConfigSandbox::config_path(),
            LEGACY_CONFIG_PLACEHOLDER.to_string(),
        ),
        (
            crate::paths::mustel_config_dir(),
            MUSTEL_CONFIG_PLACEHOLDER.to_string(),
        ),
        (crate::paths::repo_root(), REPO_PLACEHOLDER.to_string()),
    ];
    if let Some(home) = dirs::home_dir() {
        replacements.push((home, HOME_PLACEHOLDER.to_string()));
    }

    // Caminhos mais longos primeiro, para não corrigir um pedaço de outro.
    replacements.sort_by_key(|(path, _)| std::cmp::Reverse(path.as_os_str().len()));
    for (path, placeholder) in replacements {
        let literal = path.to_string_lossy().to_string();
        if !literal.is_empty() {
            result = result.replace(&literal, &placeholder);
        }
    }
    result
}

/// Arquivo produzido por um comando, com hash e (quando texto) conteúdo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactCapture {
    /// Rótulo do artefato (caminho relativo ou nome lógico).
    pub path: String,
    /// SHA-256 do conteúdo, ou `None` quando o arquivo **não existe** (ausente ≠ vazio).
    pub sha256: Option<String>,
    /// Conteúdo em texto, quando o arquivo é UTF-8 e pequeno.
    pub text: Option<String>,
}

/// Saída de uma execução.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunOutput {
    /// Código de saída (ausente quando o processo foi morto por sinal).
    pub exit_code: Option<i32>,
    /// stdout capturado.
    pub stdout: String,
    /// stderr capturado.
    pub stderr: String,
}

/// Como a configuração do usuário deve estar antes da execução.
///
/// Sem isto o comparador teria de adivinhar pelo texto das observações; explícito é melhor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigSeed {
    /// O comando não lê nem escreve configuração.
    #[default]
    Unchanged,
    /// O arquivo de configuração **não** existe.
    Absent,
    /// O arquivo contém o fixture do harness.
    Fixture,
}

/// Como o `pgpass.conf` deve estar antes da execução.
///
/// Sem isto o comparador teria de adivinhar pelo texto das observações; e é **obrigatório** para
/// casos de `pgpass`: sem semear, o oráculo leria o arquivo real do usuário (que pode ter
/// credenciais de verdade) e o conteúdo iria para o baseline versionado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PgPassSeed {
    /// O comando não lê nem escreve o `pgpass.conf`.
    #[default]
    Unchanged,
    /// O arquivo **não** existe.
    Absent,
    /// O arquivo tem as duas entradas de exemplo do harness.
    Fixture,
    /// O arquivo tem uma entrada **sem** quebra de linha no fim.
    NoTrailingNewline,
    /// O arquivo tem uma entrada com o host curinga (`*:5432:mydb:postgres`).
    Wildcard,
}

/// Uma captura completa (comando + saída + artefatos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCapture {
    /// Identificador estável usado no nome do arquivo do baseline.
    pub slug: String,
    /// Argumentos passados ao binário.
    pub argv: Vec<String>,
    /// Estado inicial da configuração do usuário.
    #[serde(default)]
    pub config_seed: ConfigSeed,
    /// Estado inicial do `pgpass.conf` do usuário.
    #[serde(default)]
    pub pgpass_seed: PgPassSeed,
    /// Saída do processo.
    pub exit_code: Option<i32>,
    /// stdout decodificado como UTF-8 (lossy, para leitura humana).
    pub stdout: String,
    /// stderr decodificado como UTF-8 (lossy, para leitura humana).
    pub stderr: String,
    /// stdout em base64 — preserva os bytes exatos (a saída de tabela do legado não é UTF-8).
    pub stdout_b64: String,
    /// stderr em base64 — preserva os bytes exatos.
    pub stderr_b64: String,
    /// Artefatos gerados/alterados.
    pub artifacts: Vec<ArtifactCapture>,
    /// Observações (ex.: "ambiente-dependente", "adicional do Mustel").
    pub notes: Vec<String>,
}

impl CommandCapture {
    /// Cria a captura executando o binário em `cwd`.
    pub fn run(slug: &str, binary: &Path, cwd: &Path, argv: &[&str], notes: &[&str]) -> Self {
        let output = Command::new(binary)
            .args(argv)
            .current_dir(cwd)
            .output()
            .unwrap_or_else(|error| panic!("failed to run {}: {error}", binary.display()));

        Self {
            slug: slug.to_string(),
            argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
            config_seed: ConfigSeed::default(),
            pgpass_seed: PgPassSeed::default(),
            exit_code: output.status.code(),
            stdout: redact(&String::from_utf8_lossy(&output.stdout)),
            stderr: redact(&String::from_utf8_lossy(&output.stderr)),
            stdout_b64: STANDARD.encode(redact_bytes(&output.stdout)),
            stderr_b64: STANDARD.encode(redact_bytes(&output.stderr)),
            artifacts: Vec::new(),
            notes: notes.iter().map(|note| (*note).to_string()).collect(),
        }
    }

    /// Define o estado inicial da configuração do usuário.
    pub fn with_seed(mut self, seed: ConfigSeed) -> Self {
        self.config_seed = seed;
        self
    }

    /// Define o estado inicial do `pgpass.conf` do usuário.
    pub fn with_pgpass_seed(mut self, seed: PgPassSeed) -> Self {
        self.pgpass_seed = seed;
        self
    }

    /// Anexa um artefato externo (fora do diretório de trabalho).
    pub fn with_artifact(mut self, label: &str, path: &Path) -> Self {
        self.artifacts.push(artifact(label, path));
        self
    }

    /// Anexa todos os arquivos encontrados em `dir` (relativos ao próprio diretório).
    pub fn with_artifacts_in(mut self, dir: &Path) -> Self {
        let mut found = BTreeMap::new();
        collect_files(dir, dir, &mut found);
        self.artifacts.extend(
            found
                .into_iter()
                .map(|(label, path)| artifact(&label, &path)),
        );
        self
    }
}

/// Aplica a redação nos bytes crus (preserva a codepage do console).
fn redact_bytes(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes).to_string();
    let redacted = redact(&text);
    if redacted == text {
        return bytes.to_vec();
    }
    // Houve substituição: reconstrúi em UTF-8 (a saída passa a ter o placeholder).
    redacted.into_bytes()
}

/// Captura um arquivo (hash + texto quando possível).
///
/// Arquivo ausente é registrado como `sha256: None` — importante para distinguir "não foi criado" de
/// "foi criado vazio", que são resultados diferentes no contrato de paridade.
pub fn artifact(label: &str, path: &Path) -> ArtifactCapture {
    let Ok(bytes) = std::fs::read(path) else {
        return ArtifactCapture {
            path: label.to_string(),
            sha256: None,
            text: None,
        };
    };

    let mut hasher = Sha256::new();
    hasher.update(&bytes);

    ArtifactCapture {
        path: label.to_string(),
        sha256: Some(format!("{:x}", hasher.finalize())),
        text: String::from_utf8(bytes)
            .ok()
            .filter(|text| text.len() <= 64 * 1024),
    }
}

fn collect_files(root: &Path, dir: &Path, found: &mut BTreeMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, found);
        } else if let Ok(relative) = path.strip_prefix(root) {
            found.insert(relative.to_string_lossy().replace('\\', "/"), path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_hash_is_stable_and_content_sensitive() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, "conteudo").expect("write");

        let first = artifact("sample.txt", &file);
        let second = artifact("sample.txt", &file);
        assert_eq!(first.text.as_deref(), Some("conteudo"));
        let hash = first.sha256.as_deref().expect("hash present");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(first.sha256, second.sha256, "same content, same hash");

        std::fs::write(&file, "outro conteudo").expect("rewrite");
        assert_ne!(first.sha256, artifact("sample.txt", &file).sha256);
    }

    #[test]
    fn redaction_replaces_machine_specific_paths() {
        let workspace = workspace_dir().to_string_lossy().to_string();
        let message = format!("Error: SQL input file not found: {workspace}/nao-existe.sql");
        let redacted = redact(&message);
        assert!(redacted.contains(WORKDIR_PLACEHOLDER));
        assert!(!redacted.contains(&workspace));
    }

    #[test]
    fn redaction_also_covers_home_and_repo_prefixes() {
        let repo = crate::paths::repo_root().to_string_lossy().to_string();
        let redacted = redact(&format!("Error: File not found: {repo}\\oracle\\x.dump"));
        assert!(redacted.contains(REPO_PLACEHOLDER));
        assert!(!redacted.contains(&repo));

        if let Some(home) = dirs::home_dir() {
            let home = home.to_string_lossy().to_string();
            let redacted = redact(&format!("caminho: {home}\\algo.txt"));
            assert!(redacted.contains(HOME_PLACEHOLDER));
            assert!(!redacted.contains(&home));
        }
    }

    #[test]
    fn redaction_is_idempotent_for_text_without_paths() {
        assert_eq!(redact("Error: nada aqui"), "Error: nada aqui");
    }

    #[test]
    fn missing_artifact_is_absent_not_empty() {
        let captured = artifact("ausente.txt", Path::new("/definitely/not/here"));
        assert!(captured.sha256.is_none(), "missing file must have no hash");
        assert!(captured.text.is_none());
    }

    #[test]
    fn empty_file_is_present_with_empty_text() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let file = dir.path().join("vazio.txt");
        std::fs::write(&file, "").expect("write");

        let captured = artifact("vazio.txt", &file);
        assert!(captured.sha256.is_some(), "empty file exists");
        assert_eq!(captured.text.as_deref(), Some(""));
    }
}
