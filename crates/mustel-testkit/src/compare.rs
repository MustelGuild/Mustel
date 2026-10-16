//! Comparação entre o baseline do oráculo e o binário do Mustel.
//!
//! Regras (derivadas dos fatos observados no T9 — ver `oracle/README.md`):
//! - **exit code** e **artefatos** são o contrato duro (N1/N2): diferença = falha;
//! - **stdout/stderr** é aviso (N4): a redação difere porque o framework é outro; além disso a saída
//!   de tabela do legado não é UTF-8, então esses casos pulam a comparação de texto;
//! - casos de **ajuda** comparam o conjunto de flags longas (N1 de superfície): flag que existe no
//!   oráculo e falta no Mustel é falha; flag extra no Mustel é informativo (adições aceitas);
//! - comando ainda **não migrado** (stub) é registrado como pulado, não como falha.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::capture::{ArtifactCapture, CommandCapture};

/// Comandos cujo comportamento já foi migrado e portanto é comparado em profundidade.
/// Atualizar a cada tarefa de comando concluída.
pub const MIGRATED_COMMANDS: &[&[&str]] = &[
    &["settings", "db-servers", "ls"],
    &["settings", "db-servers", "add"],
];

/// Nível de um resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Contrato atendido.
    Pass,
    /// Diferença aceita (N4/formatação ou adição deliberada).
    Warn,
    /// Violação do contrato (N1–N3).
    Fail,
    /// Não avaliado (stub, texto não-UTF-8, caso não aplicável).
    Skipped,
}

/// Um item verificado de um caso.
#[derive(Debug, Clone)]
pub struct Check {
    /// Nome curto do item ("exit code", "flag --ssl-mode", ...).
    pub name: String,
    /// Resultado.
    pub level: Level,
    /// Detalhe legível.
    pub detail: String,
}

/// Resultado da comparação de um caso.
#[derive(Debug, Clone)]
pub struct CaseComparison {
    /// Slug do caso.
    pub slug: String,
    /// Itens verificados.
    pub checks: Vec<Check>,
}

impl CaseComparison {
    /// Pior nível entre os itens verificados.
    pub fn worst(&self) -> Level {
        if self.checks.iter().any(|check| check.level == Level::Fail) {
            Level::Fail
        } else if self.checks.iter().any(|check| check.level == Level::Warn) {
            Level::Warn
        } else if self
            .checks
            .iter()
            .all(|check| check.level == Level::Skipped)
        {
            Level::Skipped
        } else {
            Level::Pass
        }
    }
}

/// Compara um caso do baseline com a execução atual do Mustel.
pub fn compare_case(
    baseline: &CommandCapture,
    mustel_output: &crate::capture::RunOutput,
    mustel_config: Option<&Path>,
) -> CaseComparison {
    let mut checks = Vec::new();

    // 1. exit code — contrato duro.
    checks.push(Check {
        name: "exit code".to_string(),
        level: if baseline.exit_code == mustel_output.exit_code {
            Level::Pass
        } else {
            Level::Fail
        },
        detail: format!(
            "oráculo={:?} mustel={:?}",
            baseline.exit_code, mustel_output.exit_code
        ),
    });

    // 2. superfície (casos de ajuda): flags longas do oráculo precisam existir no Mustel.
    if baseline.slug.starts_with("help-") {
        let oracle_help =
            decode_or_text(&baseline.stdout_b64, &baseline.stdout).unwrap_or_default();
        let oracle_flags = long_flags(&oracle_help);
        let mustel_flags = long_flags(&mustel_output.stdout);
        let missing: Vec<&String> = oracle_flags
            .iter()
            .filter(|flag| !mustel_flags.contains(*flag))
            .collect();
        checks.push(Check {
            name: "flags do oráculo".to_string(),
            level: if missing.is_empty() {
                Level::Pass
            } else {
                Level::Fail
            },
            detail: if missing.is_empty() {
                format!("{} flags presentes", oracle_flags.len())
            } else {
                format!("faltando: {missing:?}")
            },
        });

        let extra: Vec<&String> = mustel_flags
            .iter()
            .filter(|flag| !oracle_flags.contains(*flag))
            .collect();
        if !extra.is_empty() {
            checks.push(Check {
                name: "flags extras do Mustel".to_string(),
                level: Level::Warn,
                detail: format!("adições aceitas: {extra:?}"),
            });
        }
    }

    // 3. texto — aviso (N4), e só quando a saída do oráculo é UTF-8 de verdade.
    let oracle_text = match decode_or_text(&baseline.stdout_b64, &baseline.stdout) {
        Some(text) if is_utf8(&baseline.stdout_b64) => Some(text),
        _ => None,
    };
    match oracle_text {
        Some(oracle_stdout) => {
            let expected = normalize(&oracle_stdout);
            let actual = normalize(&mustel_output.stdout);
            checks.push(Check {
                name: "stdout (N4)".to_string(),
                level: if expected == actual {
                    Level::Pass
                } else {
                    Level::Warn
                },
                detail: if expected == actual {
                    "texto idêntico".to_string()
                } else {
                    "redação difere (formatação é N4)".to_string()
                },
            });
        }
        None => checks.push(Check {
            name: "stdout (N4)".to_string(),
            level: Level::Skipped,
            detail: "saída do legado não é UTF-8 (codepage do console)".to_string(),
        }),
    }

    // 3b. conteúdo ASCII — quando o texto não é comparável por codepage, a tabela do legado ainda
    // tem conteúdo que precisa bater: cabeçalhos e valores em ASCII. Comparar como multiconjunto
    // ignora bordas, largura de coluna e codepage, e ainda pega "faltou um servidor".
    if !is_utf8(&baseline.stdout_b64) {
        let oracle_tokens = ascii_token_counts(&decode_raw(&baseline.stdout_b64));
        let mustel_tokens = ascii_token_counts(mustel_output.stdout.as_bytes());
        let equal = oracle_tokens == mustel_tokens;
        checks.push(Check {
            name: "conteúdo (tokens ASCII)".to_string(),
            level: if equal { Level::Pass } else { Level::Fail },
            detail: if equal {
                format!("{} tokens conferem", oracle_tokens.len())
            } else {
                format!(
                    "faltando: {:?}; sobrando: {:?}",
                    missing_tokens(&oracle_tokens, &mustel_tokens),
                    missing_tokens(&mustel_tokens, &oracle_tokens)
                )
            },
        });
    }

    // 4. artefatos — contrato duro, com as normalizações registradas em known-divergences.
    for artifact in &baseline.artifacts {
        checks.push(compare_artifact(artifact, mustel_config));
    }

    CaseComparison {
        slug: baseline.slug.clone(),
        checks,
    }
}

fn compare_artifact(artifact: &ArtifactCapture, mustel_config: Option<&Path>) -> Check {
    let name = format!("artefato {}", artifact.path);

    if artifact.path != "config" {
        return Check {
            name,
            level: Level::Skipped,
            detail: "sem regra de comparação para este artefato".to_string(),
        };
    }

    let Some(mustel_path) = mustel_config else {
        return Check {
            name,
            level: Level::Skipped,
            detail: "caminho da config do Mustel não informado".to_string(),
        };
    };

    // Config ausente dos dois lados é igualdade legítima.
    let oracle_present = artifact.sha256.is_some();
    let mustel_present = mustel_path.is_file();
    if !oracle_present && !mustel_present {
        return Check {
            name,
            level: Level::Pass,
            detail: "ausente nos dois lados".to_string(),
        };
    }
    if oracle_present != mustel_present {
        return Check {
            name,
            level: Level::Fail,
            detail: format!("presença difere: oráculo={oracle_present} mustel={mustel_present}"),
        };
    }

    let oracle_text = artifact
        .text
        .clone()
        .unwrap_or_else(|| std::fs::read_to_string(mustel_path).unwrap_or_default());
    let mustel_text = std::fs::read_to_string(mustel_path).unwrap_or_default();

    match (parse_config(&oracle_text), parse_config(&mustel_text)) {
        (Some(oracle), Some(mustel)) => {
            let equal = oracle == mustel;
            Check {
                name,
                level: if equal { Level::Pass } else { Level::Fail },
                detail: if equal {
                    "conteúdo equivalente (sem `encryptedPassword`)".to_string()
                } else {
                    format!(
                        "conteúdo difere:\n  oráculo: {}\n  mustel:  {}",
                        compact(&oracle),
                        compact(&mustel)
                    )
                },
            }
        }
        _ => Check {
            name,
            level: Level::Fail,
            detail: "config não pôde ser interpretada como JSON".to_string(),
        },
    }
}

/// Lê a config como JSON (aceitando comentários) e remove campos que o Mustel não tem.
fn parse_config(text: &str) -> Option<Value> {
    let stripped = mustel_core::config::jsonc::strip_comments(text);
    let mut value: Value = serde_json::from_str(&stripped).ok()?;
    if let Some(servers) = value.get_mut("servers").and_then(Value::as_array_mut) {
        for server in servers {
            if let Some(object) = server.as_object_mut() {
                object.remove("encryptedPassword");
                object.remove("password");
            }
        }
    }
    Some(value)
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<invalido>".to_string())
}

fn decode_or_text(b64: &str, fallback: &str) -> Option<String> {
    STANDARD
        .decode(b64)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .or_else(|| Some(fallback.to_string()))
}

fn is_utf8(b64: &str) -> bool {
    STANDARD
        .decode(b64)
        .map(|bytes| std::str::from_utf8(&bytes).is_ok())
        .unwrap_or(false)
}

/// Bytes crus do baseline (sem perda de codepage).
fn decode_raw(b64: &str) -> Vec<u8> {
    STANDARD.decode(b64).unwrap_or_default()
}

/// Multiconjunto de tokens ASCII imprimíveis de uma saída.
///
/// A tabela do legado sai na codepage do console: as bordas viram bytes inválidos em UTF-8, mas
/// nomes, hosts, portas e cabeçalhos sobrevivem em ASCII. É esse conteúdo que precisa bater.
fn ascii_token_counts(bytes: &[u8]) -> std::collections::BTreeMap<String, usize> {
    let mut counts = std::collections::BTreeMap::new();
    let mut current = String::new();

    for byte in bytes {
        let character = *byte as char;
        let is_token_char = character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | '-' | '*' | ':' | '/');
        if is_token_char {
            current.push(character);
        } else if !current.is_empty() {
            *counts.entry(std::mem::take(&mut current)).or_insert(0) += 1;
        }
    }
    if !current.is_empty() {
        *counts.entry(current).or_insert(0) += 1;
    }

    counts
}

/// Tokens presentes em `left` que faltam (ou aparecem menos) em `right`.
fn missing_tokens(
    left: &std::collections::BTreeMap<String, usize>,
    right: &std::collections::BTreeMap<String, usize>,
) -> Vec<String> {
    let mut missing = Vec::new();
    for (token, count) in left {
        let other = right.get(token).copied().unwrap_or(0);
        if other < *count {
            missing.push(format!("{token} x{}", count - other));
        }
    }
    missing
}

/// Normaliza para comparar texto entre frameworks: caminhos da máquina, fim de linha e nome do
/// programa. Os caminhos precisam da mesma redação usada na captura, senão a comparação só passa na
/// máquina que gerou o baseline.
fn normalize(text: &str) -> String {
    crate::capture::redact(text)
        .replace("\r\n", "\n")
        .replace("FurLab.CLI", "mustel")
        .replace("fur.exe", "mustel")
        .trim_end()
        .to_string()
}

/// Extrai as flags longas (`--exemplo`) de um texto de ajuda.
fn long_flags(text: &str) -> Vec<String> {
    let mut flags: Vec<String> = Vec::new();
    for token in text.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')') {
        let candidate = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
        if candidate.starts_with("--") && candidate.len() > 2 {
            let flag = candidate.to_string();
            if !flags.contains(&flag) {
                flags.push(flag);
            }
        }
    }
    flags.sort();
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_long_flags_from_help_text() {
        let help = "  -i, --input <file>   Path\n      --no-confirm      Skip\n  -h, --help\n";
        let flags = long_flags(help);
        assert_eq!(flags, vec!["--help", "--input", "--no-confirm"]);
    }

    #[test]
    fn normalize_ignores_line_endings_and_program_name() {
        assert_eq!(normalize("a\r\nb"), normalize("a\nb"));
        assert_eq!(normalize("usage: FurLab.CLI"), normalize("usage: mustel"));
    }

    #[test]
    fn normalize_redacts_the_mustel_workspace_path() {
        let workspace = crate::paths::workspace_dir().to_string_lossy().to_string();
        let normalized = normalize(&format!("Error: File not found: {workspace}/x.dump"));
        assert!(normalized.contains(crate::capture::WORKDIR_PLACEHOLDER));
        assert!(!normalized.contains(&workspace));
    }

    #[test]
    fn parse_config_drops_password_fields_and_ignores_comments() {
        let legacy = r#"{
          // comentário
          "servers": [ { "name": "dev", "encryptedPassword": "blob", "port": 5432 } ],
          "defaults": { "outputDirectory": "./results" }
        }"#;
        let mustel = r#"{ "servers": [ { "name": "dev", "port": 5432 } ], "defaults": { "outputDirectory": "./results" } }"#;

        assert_eq!(parse_config(legacy), parse_config(mustel));
    }

    #[test]
    fn ascii_tokens_ignore_box_drawing_and_count_repeats() {
        // Bytes de borda em codepage (não-UTF-8) mais conteúdo ASCII.
        let mut bytes = vec![0xDA, 0xC4, 0xBF];
        bytes.extend_from_slice(b" Name  No  No ");
        bytes.extend_from_slice(&[0xC0, 0xC4, 0xD9]);

        let counts = ascii_token_counts(&bytes);
        assert_eq!(counts.get("Name"), Some(&1));
        assert_eq!(counts.get("No"), Some(&2));
        assert_eq!(counts.len(), 2);
    }

    #[test]
    fn missing_tokens_reports_the_difference() {
        let oracle = ascii_token_counts(b" dev prod 5432");
        let mustel = ascii_token_counts(b" dev 5432");

        assert_eq!(
            missing_tokens(&oracle, &mustel),
            vec!["prod x1".to_string()]
        );
        assert!(missing_tokens(&mustel, &oracle).is_empty());
    }

    #[test]
    fn exit_code_difference_is_a_failure() {
        let baseline = CommandCapture {
            slug: "x".to_string(),
            argv: vec![],
            config_seed: crate::capture::ConfigSeed::Absent,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            stdout_b64: STANDARD.encode([]),
            stderr_b64: STANDARD.encode([]),
            artifacts: vec![],
            notes: vec![],
        };
        let output = crate::capture::RunOutput {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
        };

        let comparison = compare_case(&baseline, &output, None);
        assert_eq!(comparison.worst(), Level::Fail);
        assert_eq!(comparison.checks[0].name, "exit code");
    }
}
