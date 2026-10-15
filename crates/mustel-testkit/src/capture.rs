//! Execução e registro de uma captura do oráculo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

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

/// Uma captura completa (comando + saída + artefatos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCapture {
    /// Identificador estável usado no nome do arquivo do baseline.
    pub slug: String,
    /// Argumentos passados ao binário.
    pub argv: Vec<String>,
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
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            stdout_b64: STANDARD.encode(&output.stdout),
            stderr_b64: STANDARD.encode(&output.stderr),
            artifacts: Vec::new(),
            notes: notes.iter().map(|note| (*note).to_string()).collect(),
        }
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
