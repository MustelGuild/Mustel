//! Forma de uma captura do oráculo (desserializada do baseline congelado).
//!
//! Desde a remoção do `legacy/` não existe mais runner de captura: as 47 capturas de
//! `fixtures/baseline/` são contrato congelado e o `oracle-compare` só as **lê**. O que sobra aqui
//! são os tipos que descrevem uma captura e a redação de caminhos da máquina ([`redact`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

/// Caminho que o **legado** usava para a configuração, para redigir as capturas antigas.
///
/// O `legacy/` saiu do repositório, mas o baseline congelado ainda contém esse caminho no texto; sem
/// esta substituição o arquivo versionado carregaria o nome de usuário da máquina que capturou.
fn legacy_config_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\legacy"))
        .join("FurLab")
        .join("furlab.jsonc")
}

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
        (legacy_config_path(), LEGACY_CONFIG_PLACEHOLDER.to_string()),
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
///
/// Só é **lida** do baseline congelado: o runner que executava o binário do oráculo saiu junto com o
/// `legacy/`, e não há como re-gravar as 47 capturas.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
