//! Retenção dos relatórios `.log` gerados pela aba **Activity**.
//!
//! O log tem 30 dias (`RETENTION_DAYS`); os relatórios seguem a mesma janela. O nome carrega a data
//! UTC (`mustel-report-YYYYMMDD-HHMMSS.log`), então a comparação é lexicográfica, como nos arquivos
//! do log. Arquivos que ficam são restringidos a `0600` no Unix — relatórios antigos de versões
//! anteriores podem ter sido gravados com o modo padrão.

use std::fs;
use std::io;
use std::path::Path;

/// Prefixo dos arquivos gerados por `Report…`.
const REPORT_PREFIX: &str = "mustel-report-";
/// Extensão dos relatórios.
const REPORT_SUFFIX: &str = ".log";

/// Apaga os relatórios fora da janela e devolve quantos saíram.
///
/// O diretório e os relatórios que ficam são restringidos a `0700`/`0600` no Unix de forma melhor
/// esforço, como o [`Store`](super::Store) faz com o log.
pub fn prune(dir: &Path, days: u32, now_ms: u64) -> io::Result<usize> {
    if dir.is_dir() {
        let _ = crate::private_fs::restrict_dir(dir);
    }

    let reader = match fs::read_dir(dir) {
        Ok(reader) => reader,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };

    let span = u64::from(days.saturating_sub(1)) * 86_400_000;
    let cutoff = crate::datetime::day_key(now_ms.saturating_sub(span));
    let mut removed = 0;

    for entry in reader {
        let path = entry?.path();
        let Some(day) = report_day(&path) else {
            continue;
        };
        if day < cutoff {
            fs::remove_file(path)?;
            removed += 1;
        } else {
            let _ = crate::private_fs::restrict_file(&path);
        }
    }

    Ok(removed)
}

/// A data (`YYYY-MM-DD`) do nome de um relatório, se o nome tiver o formato esperado.
fn report_day(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name
        .strip_prefix(REPORT_PREFIX)?
        .strip_suffix(REPORT_SUFFIX)?;
    let date = rest.get(0..8)?;
    if !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_name(at_ms: u64) -> String {
        format!(
            "mustel-report-{}.log",
            crate::datetime::format_utc_compact(at_ms)
        )
    }

    #[test]
    fn it_prunes_only_old_reports_and_restricts_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let now = 1_700_000_000_000u64;
        let old = dir.path().join(report_name(now - 40 * 86_400_000));
        let current = dir.path().join(report_name(now));
        let other = dir.path().join("notes.txt");
        fs::write(&old, "velho").expect("velho");
        fs::write(&current, "atual").expect("atual");
        fs::write(&other, "fica").expect("outro");

        let removed = prune(dir.path(), super::super::RETENTION_DAYS, now).expect("poda");

        assert_eq!(removed, 1);
        assert!(!old.exists(), "o relatório de 40 dias saiu");
        assert!(current.exists(), "o relatório do dia fica");
        assert!(other.exists(), "arquivo que não é relatório fica");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            assert_eq!(
                fs::metadata(&current)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600,
                "relatório que fica é restringido"
            );
            assert_eq!(
                fs::metadata(dir.path())
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700,
                "o diretório é restringido"
            );
        }
    }

    #[test]
    fn a_missing_directory_is_zero() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nao-existe");

        assert_eq!(prune(&missing, 30, 1_700_000_000_000).expect("poda"), 0);
    }

    #[test]
    fn malformed_names_are_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let malformed = dir.path().join("mustel-report-nope.log");
        fs::write(&malformed, "fica").expect("malformado");

        assert_eq!(prune(dir.path(), 30, 1_700_000_000_000).expect("poda"), 0);
        assert!(malformed.exists(), "nome fora do formato não é removido");
    }
}
