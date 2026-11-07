//! Onde o log de atividade e os relatórios moram.
//!
//! Log não é configuração: o lugar canônico é a pasta de **estado** do SO (no Linux,
//! `~/.local/state`; em Windows/macOS, a pasta de dados local), sob `Mustel`. `MUSTEL_LOG_DIR`
//! sobrescreve tudo — é o que isola os testes e o harness de paridade do log real do usuário.

use std::path::{Path, PathBuf};

/// Variável de ambiente que sobrescreve o diretório do log.
pub const LOG_DIR_ENV: &str = "MUSTEL_LOG_DIR";

/// Subpasta dos arquivos `activity-*.jsonl`.
const LOG_SUBDIR: &str = "logs";
/// Subpasta dos `.log` gerados pelo relatório.
const REPORTS_SUBDIR: &str = "reports";

/// O diretório do usuário, quando o SO expõe (usado pelo redator do relatório).
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Diretório do log, honrando `MUSTEL_LOG_DIR`.
pub fn log_dir() -> Option<PathBuf> {
    log_dir_with(override_dir().as_deref())
}

/// Versão pura de [`log_dir`]: sem tocar o ambiente, para ser testável e reutilizável.
pub fn log_dir_with(override_dir: Option<&Path>) -> Option<PathBuf> {
    match override_dir {
        Some(dir) => Some(dir.to_path_buf()),
        None => state_root().map(|root| root.join(LOG_SUBDIR)),
    }
}

/// Diretório dos relatórios `.log` (dentro do override, quando houver).
pub fn reports_dir() -> Option<PathBuf> {
    reports_dir_with(override_dir().as_deref())
}

/// Versão pura de [`reports_dir`].
pub fn reports_dir_with(override_dir: Option<&Path>) -> Option<PathBuf> {
    match override_dir {
        Some(dir) => Some(dir.join(REPORTS_SUBDIR)),
        None => state_root().map(|root| root.join(REPORTS_SUBDIR)),
    }
}

/// Raiz de estado do Mustel: `state_dir` (Linux) com fallback para `data_local_dir` (Windows/macOS).
fn state_root() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .map(|dir| dir.join(crate::config::paths::APP_DIR_NAME))
}

/// O valor de `MUSTEL_LOG_DIR`, quando definido e não vazio.
fn override_dir() -> Option<PathBuf> {
    std::env::var_os(LOG_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_becomes_the_log_directory() {
        let dir = Path::new("/tmp/mustel-logs");

        assert_eq!(log_dir_with(Some(dir)), Some(dir.to_path_buf()));
    }

    #[test]
    fn the_reports_live_inside_the_override() {
        let dir = Path::new("/tmp/mustel-logs");

        assert_eq!(reports_dir_with(Some(dir)), Some(dir.join(REPORTS_SUBDIR)));
    }

    #[test]
    fn without_an_override_the_default_ends_with_the_known_subdirectories() {
        let log = log_dir_with(None);
        let reports = reports_dir_with(None);

        // `dirs` pode não expor pasta de estado/dados (ambiente mínimo); quando expõe, o sufixo é
        // previsível.
        if let Some(log) = log {
            assert!(log.ends_with(LOG_SUBDIR), "{log:?}");
        }
        if let Some(reports) = reports {
            assert!(reports.ends_with(REPORTS_SUBDIR), "{reports:?}");
        }
    }
}
