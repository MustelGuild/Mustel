//! O log de atividade da janela.
//!
//! O [`Log`] embrulha o [`Store`] do núcleo, mantém as entradas carregadas (a aba Activity lê
//! daqui) e registra o que acontece. [`entry_for`] é a tradução de um [`JobResult`] para uma
//! entrada — pura e testável, como o `toast_for`.
//!
//! Escrever é **melhor esforço**: uma falha de disco vira um aviso na aba, não um erro que
//! interrompe a operação.

use std::path::PathBuf;

use mustel_core::activity::{Area, Entry, Status, Store, log_dir};
use mustel_core::config::model::AiProfile;

use crate::request::JobResult;

/// Quantos dias a janela carrega e retém.
pub(crate) const DAYS: u32 = 30;

/// O log da janela: store em disco + entradas em memória.
#[derive(Debug, Default)]
pub(crate) struct Log {
    /// `None` quando o SO não expõe pasta de estado (ou em testes).
    store: Option<Store>,
    /// As entradas dos últimos 30 dias, em ordem cronológica.
    entries: Vec<Entry>,
    /// O último problema de leitura/escrita, mostrado na aba.
    error: Option<String>,
}

impl Log {
    /// Abre o log padrão, carrega a janela e poda o que passou dos 30 dias.
    pub(crate) fn open() -> Self {
        let mut log = Self {
            store: log_dir().map(Store::new),
            entries: Vec::new(),
            error: None,
        };
        log.reload();
        log
    }

    /// Log sem disco, para testes de UI.
    #[cfg(test)]
    pub(crate) fn in_memory(entries: Vec<Entry>) -> Self {
        Self {
            store: None,
            entries,
            error: None,
        }
    }

    /// Relê a janela do disco (pega execuções feitas pelo CLI com a janela aberta).
    pub(crate) fn reload(&mut self) {
        let Some(store) = &self.store else {
            self.error = Some("The activity log is not available on this system.".to_string());
            return;
        };

        let now = mustel_core::datetime::now_ms();
        let _ = store.prune(DAYS, now);
        match store.load(DAYS, now) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("Could not read the activity log: {error}"));
            }
        }
    }

    /// Registra uma entrada: primeiro no disco, depois na memória.
    pub(crate) fn record(&mut self, entry: Entry) {
        if let Some(store) = &self.store
            && let Err(error) = store.append(&entry)
        {
            self.error = Some(format!("Could not write the activity log: {error}"));
        }
        self.entries.push(entry);
    }

    /// As entradas carregadas, em ordem cronológica.
    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// O último problema de disco, se houver.
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Traduz o resultado de um job em uma entrada do log.
///
/// `profiles` resolve o `id` do perfil para o nome mostrado; `duration_ms` vem do job que estava
/// em andamento (a descoberta de bases roda fora da fila única e não tem duração).
pub(crate) fn entry_for(
    result: &JobResult,
    profiles: &[AiProfile],
    duration_ms: Option<u64>,
) -> Option<Entry> {
    let entry = match result {
        JobResult::Servers(servers) => Entry::new(Area::App, "app.servers", Status::Info)
            .with_detail(format!("{} server(s)", servers.len())),
        JobResult::Saved(name) => {
            Entry::new(Area::Connections, "server.save", Status::Ok).with_target(name)
        }
        JobResult::Removed(name) => {
            Entry::new(Area::Connections, "server.remove", Status::Ok).with_target(name)
        }
        JobResult::PasswordSaved(name) => {
            Entry::new(Area::Connections, "password.save", Status::Ok).with_target(name)
        }
        JobResult::Tested { server, outcome } => {
            let mut entry = Entry::new(
                Area::Connections,
                "connection.test",
                if outcome.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_target(server);
            if let Err(error) = outcome {
                entry = entry.with_error(error);
            }
            entry
        }
        JobResult::QueryDone {
            total_rows,
            successes,
            failures,
            ..
        } => {
            let failed = *failures > 0;
            let mut entry = Entry::new(
                Area::Query,
                "query.run",
                if failed { Status::Error } else { Status::Ok },
            )
            .with_detail(format!(
                "{total_rows} row(s) · {successes} ok, {failures} failed"
            ));
            if failed {
                entry = entry.with_error(format!("{failures} database(s) failed"));
            }
            entry
        }
        JobResult::QueryWithoutDatabases => Entry::new(Area::Query, "query.run", Status::Error)
            .with_error("No accessible databases found across selected servers."),
        JobResult::Schema {
            server,
            database,
            schema,
        } => {
            let mut entry = Entry::new(
                Area::Query,
                "schema.load",
                if schema.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_target(format!("{server}/{database}"));
            match schema {
                Ok(schema) => {
                    entry = entry.with_detail(format!(
                        "{} relation(s), {} function(s)",
                        schema.tables.len(),
                        schema.functions.len()
                    ));
                }
                Err(error) => entry = entry.with_error(error),
            }
            entry
        }
        JobResult::Databases {
            server, databases, ..
        } => {
            let mut entry = Entry::new(
                Area::Query,
                "databases.list",
                if databases.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_target(server);
            match databases {
                Ok(catalog) => {
                    entry = entry.with_detail(format!("{} database(s)", catalog.len()));
                }
                Err(error) => entry = entry.with_error(error),
            }
            entry
        }
        JobResult::SelectionTested { name, outcome } => {
            let mut entry = Entry::new(
                Area::Selector,
                "selector.test",
                if outcome.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_target(name);
            match outcome {
                Ok(outcome) => entry = entry.with_detail(outcome.summary()),
                Err(error) => entry = entry.with_error(error),
            }
            entry
        }
        JobResult::Selection {
            query, selection, ..
        } => {
            let databases: usize = selection.iter().map(|server| server.databases.len()).sum();
            Entry::new(Area::Selector, "selector.apply", Status::Ok)
                .with_target(query)
                .with_detail(format!("{databases} database(s) selected"))
        }
        JobResult::SelectionFailed { query, error } => {
            Entry::new(Area::Selector, "selector.apply", Status::Error)
                .with_target(query)
                .with_error(error)
        }
        JobResult::Ddl { result } => {
            let mut entry = Entry::new(
                Area::Ai,
                "ai.ddl",
                if result.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_detail("schema generated");
            if let Err(error) = result {
                entry = entry.with_error(error);
            }
            entry
        }
        JobResult::SqlGenerated { result } => {
            let mut entry = Entry::new(
                Area::Ai,
                "ai.generate",
                if result.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_detail("SQL generated");
            if let Err(error) = result {
                entry = entry.with_error(error);
            }
            entry
        }
        JobResult::AiProfilesSaved => Entry::new(Area::Ai, "ai.profiles.save", Status::Ok),
        JobResult::AiTested { profile, result } => {
            let name = profiles
                .iter()
                .find(|candidate| candidate.id == *profile)
                .map(|candidate| candidate.name.clone())
                .unwrap_or_else(|| profile.clone());
            let mut entry = Entry::new(
                Area::Ai,
                "ai.test",
                if result.is_ok() {
                    Status::Ok
                } else {
                    Status::Error
                },
            )
            .with_target(name);
            if let Err(error) = result {
                entry = entry.with_error(error);
            }
            entry
        }
        JobResult::Failed(error) => {
            Entry::new(Area::App, "job.failed", Status::Error).with_error(error)
        }
        // O updater registra as próprias entradas (`app.update.*`), com o detalhe que o toast
        // mostra: passar por aqui duplicaria a linha no log.
        JobResult::UpdateChecked { .. }
        | JobResult::UpdateProgress { .. }
        | JobResult::UpdateDownloaded { .. }
        | JobResult::UpdateManual { .. }
        | JobResult::UpdateCancelled
        | JobResult::UpdateFailed(_) => return None,
    };

    Some(match duration_ms {
        Some(duration_ms) => entry.with_duration_ms(duration_ms),
        None => entry,
    })
}

/// O diretório onde os relatórios `.log` são salvos.
///
/// Fora do estado canônico, devolve um fallback **privado** (home ou temporário `0700`); `None` só
/// quando nem isso existe — a tela então avisa que não há pasta segura para salvar.
pub(crate) fn reports_dir_or_fallback() -> Option<PathBuf> {
    mustel_core::activity::reports_dir_or_private_fallback()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::activity::{Area, Status};
    use std::path::PathBuf;

    fn profile(id: &str, name: &str) -> AiProfile {
        AiProfile {
            id: id.to_string(),
            name: name.to_string(),
            ..AiProfile::default()
        }
    }

    fn assert_entry(result: &JobResult, expected: (Area, &str, Status)) {
        let entry =
            entry_for(result, &[], None).unwrap_or_else(|| panic!("sem entrada para {result:?}"));
        assert_eq!(
            (entry.area, entry.op.as_str(), entry.status),
            expected,
            "{result:?}"
        );
    }

    #[test]
    fn servers_and_saves_are_recorded() {
        assert_entry(
            &JobResult::Servers(vec![]),
            (Area::App, "app.servers", Status::Info),
        );
        assert_entry(
            &JobResult::Saved("Local".to_string()),
            (Area::Connections, "server.save", Status::Ok),
        );
        assert_entry(
            &JobResult::Removed("Local".to_string()),
            (Area::Connections, "server.remove", Status::Ok),
        );
        assert_entry(
            &JobResult::PasswordSaved("Local".to_string()),
            (Area::Connections, "password.save", Status::Ok),
        );
    }

    #[test]
    /// CONTRACT: IF-265
    fn a_connection_test_records_the_reason() {
        let entry = entry_for(
            &JobResult::Tested {
                server: "Local".to_string(),
                outcome: Err("Connection failed: refused".to_string()),
            },
            &[],
            Some(1_200),
        )
        .expect("entrada");

        assert_eq!(entry.status, Status::Error);
        assert_eq!(entry.target.as_deref(), Some("Local"));
        assert_eq!(entry.error.as_deref(), Some("Connection failed: refused"));
        assert_eq!(entry.duration_ms, Some(1_200));
    }

    #[test]
    fn a_query_records_rows_and_failures() {
        let entry = entry_for(
            &JobResult::QueryDone {
                consolidated: None,
                log: PathBuf::from("run.log"),
                servers: 2,
                successes: 1,
                failures: 1,
                total_rows: 42,
            },
            &[],
            None,
        )
        .expect("entrada");

        assert_eq!(entry.area, Area::Query);
        assert_eq!(entry.op, "query.run");
        assert_eq!(entry.status, Status::Error);
        assert!(entry.detail.as_deref().unwrap_or("").contains("42 row(s)"));
        assert!(entry.error.as_deref().unwrap_or("").contains("1 database"));

        assert_eq!(
            entry_for(&JobResult::QueryWithoutDatabases, &[], None)
                .expect("entrada")
                .status,
            Status::Error
        );
    }

    #[test]
    fn an_ai_test_uses_the_profile_name() {
        let profiles = [profile("ollama", "Ollama")];

        let entry = entry_for(
            &JobResult::AiTested {
                profile: "ollama".to_string(),
                result: Err("No key stored for this profile.".to_string()),
            },
            &profiles,
            None,
        )
        .expect("entrada");

        assert_eq!(entry.area, Area::Ai);
        assert_eq!(entry.op, "ai.test");
        assert_eq!(entry.status, Status::Error);
        assert_eq!(entry.target.as_deref(), Some("Ollama"));
        assert_eq!(
            entry.error.as_deref(),
            Some("No key stored for this profile.")
        );
    }

    #[test]
    fn selectors_and_ai_are_recorded() {
        assert_entry(
            &JobResult::SelectionFailed {
                query: "todos".to_string(),
                error: "boom".to_string(),
            },
            (Area::Selector, "selector.apply", Status::Error),
        );
        assert_entry(
            &JobResult::AiProfilesSaved,
            (Area::Ai, "ai.profiles.save", Status::Ok),
        );
        assert_entry(
            &JobResult::SqlGenerated {
                result: Ok("select 1".to_string()),
            },
            (Area::Ai, "ai.generate", Status::Ok),
        );
        assert_entry(
            &JobResult::Ddl {
                result: Err("no schema".to_string()),
            },
            (Area::Ai, "ai.ddl", Status::Error),
        );
        assert_entry(
            &JobResult::Failed("boom".to_string()),
            (Area::App, "job.failed", Status::Error),
        );
    }

    #[test]
    fn a_log_without_disk_keeps_entries_in_memory() {
        let mut log = Log::in_memory(Vec::new());
        let entry = Entry::new(Area::App, "app.start", Status::Info);

        log.record(entry.clone());

        assert_eq!(log.entries(), std::slice::from_ref(&entry));
        assert!(log.error().is_none());
    }
}
