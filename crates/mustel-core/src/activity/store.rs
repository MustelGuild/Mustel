//! O arquivo do log: `activity-YYYY-MM-DD.jsonl`, um evento por linha.
//!
//! `append` abre em modo append a cada escrita — sem handle preso, o que deixa o hook de panic
//! gravar com segurança a qualquer momento. `load` relê a janela de retenção e ignora linhas
//! malformadas (um arquivo truncado no meio não derruba a aba). `prune` apaga os arquivos fora da
//! janela.
//!
//! Como os nomes são datas ISO, a comparação para a janela é **lexicográfica** — não é preciso
//! converter a data de volta para dias.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::entry::Entry;

/// Prefixo dos arquivos diários.
const FILE_PREFIX: &str = "activity-";
/// Extensão dos arquivos diários.
const FILE_SUFFIX: &str = ".jsonl";

/// Um diretório de arquivos diários do log.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Store sobre `dir` (criado no primeiro `append`).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// O diretório deste store.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// O arquivo do dia de `at_ms`.
    pub fn file_for(&self, at_ms: u64) -> PathBuf {
        self.dir.join(format!(
            "{FILE_PREFIX}{}{FILE_SUFFIX}",
            crate::datetime::day_key(at_ms)
        ))
    }

    /// Grava uma entrada no arquivo do dia dela.
    ///
    /// O diretório nasce `0700` e o arquivo `0600` no Unix (`private_fs`): o log guarda nomes
    /// de servidores, hosts e mensagens de erro, então não pode ser legível por outros usuários.
    pub fn append(&self, entry: &Entry) -> io::Result<()> {
        crate::private_fs::create_dir(&self.dir)?;
        let line = serde_json::to_string(entry)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut file = crate::private_fs::open_append(&self.file_for(entry.at_ms))?;
        writeln!(file, "{line}")
    }

    /// Lê as entradas dos últimos `days` dias (incluindo hoje), em ordem cronológica.
    ///
    /// Linhas malformadas são ignoradas: o log é melhor esforço, não um contrato de integridade.
    /// Arquivos e diretório existentes são restringidos a `0600`/`0700` no Unix de forma melhor
    /// esforço — uma falha de `chmod` não impede ler o log.
    pub fn load(&self, days: u32, now_ms: u64) -> io::Result<Vec<Entry>> {
        let cutoff = cutoff_day(days, now_ms);
        let mut entries = Vec::new();

        if self.dir.is_dir() {
            let _ = crate::private_fs::restrict_dir(&self.dir);
        }
        for (path, day) in self.files()? {
            let _ = crate::private_fs::restrict_file(&path);
            if day < cutoff {
                continue;
            }
            let text = fs::read_to_string(path)?;
            for line in text.lines() {
                if let Ok(entry) = serde_json::from_str::<Entry>(line) {
                    entries.push(entry);
                }
            }
        }

        entries.sort_by_key(|entry| entry.at_ms);
        Ok(entries)
    }

    /// Apaga os arquivos fora da janela e devolve quantos saíram.
    ///
    /// Os arquivos que ficam também são restringidos a `0600` (melhor esforço) no Unix.
    pub fn prune(&self, days: u32, now_ms: u64) -> io::Result<usize> {
        let cutoff = cutoff_day(days, now_ms);
        let mut removed = 0;

        if self.dir.is_dir() {
            let _ = crate::private_fs::restrict_dir(&self.dir);
        }
        for (path, day) in self.files()? {
            if day < cutoff {
                fs::remove_file(path)?;
                removed += 1;
            } else {
                let _ = crate::private_fs::restrict_file(&path);
            }
        }

        Ok(removed)
    }

    /// Os arquivos do diretório, com a data (texto ISO) extraída do nome.
    fn files(&self) -> io::Result<Vec<(PathBuf, String)>> {
        let reader = match fs::read_dir(&self.dir) {
            Ok(reader) => reader,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };

        let mut files = Vec::new();
        for entry in reader {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            if let Some(day) = day_of(&path) {
                files.push((path, day));
            }
        }

        Ok(files)
    }
}

/// O primeiro dia da janela de retenção (`days` dias incluindo hoje), como texto ISO.
fn cutoff_day(days: u32, now_ms: u64) -> String {
    let span = u64::from(days.saturating_sub(1)) * 86_400_000;
    crate::datetime::day_key(now_ms.saturating_sub(span))
}

/// O texto ISO da data em `activity-YYYY-MM-DD.jsonl`, se o nome casar.
fn day_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let date = name.strip_prefix(FILE_PREFIX)?.strip_suffix(FILE_SUFFIX)?;
    let mut parts = date.split('-');

    let year = parts.next()?;
    let month = parts.next()?;
    let day = parts.next()?;
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }

    Some(date.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::entry::{Area, Status};

    /// 2026-11-06T12:00:00Z.
    const NOVEMBER_6: u64 = 20_763 * 86_400_000 + 12 * 3_600_000;
    /// 2026-11-05T12:00:00Z (dentro da janela de 30 dias).
    const NOVEMBER_5: u64 = 20_762 * 86_400_000 + 12 * 3_600_000;
    /// 2026-10-01T12:00:00Z.
    const OCTOBER_1: u64 = 20_727 * 86_400_000 + 12 * 3_600_000;

    fn entry(at_ms: u64, detail: &str) -> Entry {
        Entry::new(Area::App, "app.start", Status::Info)
            .at(at_ms)
            .with_detail(detail)
    }

    #[test]
    /// CONTRACT: IF-265
    fn an_entry_round_trips_through_the_store() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());
        let written = entry(NOVEMBER_6, "primeiro");

        store.append(&written).expect("grava");
        let loaded = store.load(30, NOVEMBER_6).expect("lê");

        assert_eq!(loaded, vec![written]);
    }

    /// O log não é legível por outros usuários: `0700` no diretório e `0600` nos arquivos.
    ///
    /// Arquivos existentes são restringidos no `load`; os novos já nascem privados no `append`.
    /// CONTRACT: M1 da auditoria de 2026-11-08.
    #[cfg(unix)]
    #[test]
    fn the_log_directory_and_files_are_private() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("temporário");
        let dir = root.path().join("logs");
        fs::create_dir_all(&dir).expect("cria");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("chmod dir");
        let old = dir.join("activity-2026-11-05.jsonl");
        fs::write(&old, "").expect("grava");
        fs::set_permissions(&old, fs::Permissions::from_mode(0o644)).expect("chmod arquivo");

        let store = Store::new(&dir);
        store.append(&entry(NOVEMBER_6, "novo")).expect("grava");
        let _ = store.load(30, NOVEMBER_6).expect("lê");

        let mode = |path: &std::path::Path| {
            std::fs::metadata(path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(&dir), 0o700, "diretório do log");
        assert_eq!(mode(&old), 0o600, "arquivo existente");
        assert_eq!(mode(&store.file_for(NOVEMBER_6)), 0o600, "arquivo novo");
    }

    #[test]
    fn entries_come_back_in_chronological_order() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());

        store.append(&entry(NOVEMBER_6, "depois")).expect("grava");
        store.append(&entry(NOVEMBER_5, "antes")).expect("grava");

        let loaded = store.load(30, NOVEMBER_6).expect("lê");

        assert_eq!(
            loaded
                .iter()
                .map(|entry| entry.detail.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("antes"), Some("depois")]
        );
    }

    #[test]
    fn a_malformed_line_does_not_break_the_load() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());
        let good = entry(NOVEMBER_6, "boa");
        let line = serde_json::to_string(&good).expect("serializa");
        fs::write(
            store.file_for(NOVEMBER_6),
            format!("not json\n{line}\n{{\"quebrado\":\n"),
        )
        .expect("escreve");

        let loaded = store.load(30, NOVEMBER_6).expect("lê");

        assert_eq!(loaded, vec![good]);
    }

    #[test]
    fn only_the_retention_window_is_loaded() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());
        store.append(&entry(OCTOBER_1, "fora")).expect("grava");
        store.append(&entry(NOVEMBER_6, "dentro")).expect("grava");

        let loaded = store.load(30, NOVEMBER_6).expect("lê");

        assert_eq!(
            loaded
                .iter()
                .map(|entry| entry.detail.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("dentro")],
            "o de 1º de outubro está fora dos 30 dias até 6 de novembro"
        );
    }

    #[test]
    fn files_outside_the_window_are_pruned() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());
        store.append(&entry(OCTOBER_1, "fora")).expect("grava");
        store.append(&entry(NOVEMBER_6, "dentro")).expect("grava");

        let removed = store.prune(30, NOVEMBER_6).expect("poda");

        assert_eq!(removed, 1);
        assert!(!store.file_for(OCTOBER_1).exists());
        assert!(store.file_for(NOVEMBER_6).exists());
    }

    #[test]
    fn the_file_name_is_the_utc_day() {
        let store = Store::new("/tmp/mustel");
        let path = store.file_for(NOVEMBER_6);

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("activity-2026-11-06.jsonl")
        );
    }

    #[test]
    fn a_missing_directory_loads_empty() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path().join("ainda-nao-existe"));

        assert!(store.load(30, NOVEMBER_6).expect("lê").is_empty());
        assert_eq!(store.prune(30, NOVEMBER_6).expect("poda"), 0);
    }
}
