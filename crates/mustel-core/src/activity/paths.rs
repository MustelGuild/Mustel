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

/// Diretório dos relatórios com um fallback **privado** quando o SO não expõe pasta de estado.
///
/// A ordem é `MUSTEL_LOG_DIR`/state → `~/.mustel/reports` → diretório temporário exclusivo criado
/// com `0700`. O fallback nunca devolve o `/tmp` em si; o salvamento só grava em diretório privado.
pub fn reports_dir_or_private_fallback() -> Option<PathBuf> {
    if let Some(dir) = reports_dir() {
        return Some(dir);
    }
    if let Some(home) = home_dir() {
        return Some(home.join(".mustel").join(REPORTS_SUBDIR));
    }
    private_temp_reports_dir()
}

/// Cria um diretório temporário exclusivo e privado (`0700` no Unix).
fn private_temp_reports_dir() -> Option<PathBuf> {
    let base = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();

    for attempt in 0..16 {
        let path = base.join(format!(
            "mustel-reports-{}-{nanos}-{attempt}",
            std::process::id()
        ));
        match private_dir(&path) {
            Ok(()) => return Some(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// Cria o diretório sem seguir um symlink plantado (falha se o caminho já existir).
fn private_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(not(unix))]
    let builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)?;
    crate::private_fs::restrict_dir(path)
}

/// Raiz de estado do Mustel (`<state>/Mustel`), sem o subdiretório do log.
///
/// O log fica em `logs/` e o estado do updater em `update.json`, ambos sob esta raiz.
pub fn state_root() -> Option<PathBuf> {
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

    #[cfg(unix)]
    #[test]
    fn the_private_temp_fallback_is_0700() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = private_temp_reports_dir().expect("fallback privado");
        assert!(dir.starts_with(std::env::temp_dir()), "{dir:?}");
        assert_eq!(
            std::fs::metadata(&dir)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        std::fs::remove_dir(&dir).expect("limpa");
    }
}
