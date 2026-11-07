//! O relatório em texto: o conteúdo do `.log` que o usuário revisa e envia.
//!
//! Recebe **todas** as entradas retidas e não trunca nada — o requisito é que a prévia mostre
//! exatamente o que será enviado. O SQL já saiu no [`Redactor`]; a legenda da anonimização fica
//! fora do texto ([`Report::legend`]) e só aparece na tela.

use super::entry::{Entry, Status, op_label};
use super::redact::{Redactor, Replacement};
use crate::datetime::format_utc;

/// Versão e plataforma que encabeçam o relatório.
#[derive(Debug, Clone, Copy)]
pub struct AppInfo<'a> {
    /// Versão do Mustel.
    pub version: &'a str,
    /// Sistema operacional (`linux`, `windows`, `macos`).
    pub os: &'a str,
    /// Arquitetura (`x86_64`, `aarch64`).
    pub arch: &'a str,
}

impl AppInfo<'static> {
    /// A versão e a plataforma deste binário.
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
}

/// O relatório pronto.
#[derive(Debug, Clone)]
pub struct Report {
    /// O texto do `.log` (o anexo).
    pub text: String,
    /// Os pares reais → token, para a legenda da tela (nunca vão ao texto).
    pub legend: Vec<Replacement>,
    /// Quantas entradas entraram.
    pub entries: usize,
    /// Quantas delas falharam.
    pub errors: usize,
}

/// Monta o relatório de **todas** as entradas, em ordem cronológica.
pub fn render(entries: &[Entry], redactor: &Redactor, info: &AppInfo<'_>, now_ms: u64) -> Report {
    let mut redacted: Vec<Entry> = entries
        .iter()
        .map(|entry| redactor.report_entry(entry))
        .collect();
    redacted.sort_by_key(|entry| entry.at_ms);

    let errors = redacted
        .iter()
        .filter(|entry| entry.status == Status::Error)
        .count();

    let mut text = String::new();
    text.push_str("Mustel report\n");
    text.push_str(&format!("Version: {}\n", info.version));
    text.push_str(&format!("System: {} ({})\n", info.os, info.arch));
    text.push_str(&format!("Generated: {} UTC\n", format_utc(now_ms)));
    text.push_str(&format!("Period: {}\n", period(&redacted)));
    text.push_str(&format!(
        "Entries: {} (errors: {})\n",
        redacted.len(),
        errors
    ));
    text.push_str(
        "\nAnonymization: server, host, user, database and profile names, filesystem paths\n\
         and SQL are replaced or omitted in this report.\n",
    );

    if redacted.is_empty() {
        text.push_str("\nNo activity recorded yet.\n");
        return Report {
            text,
            legend: redactor.legend().to_vec(),
            entries: 0,
            errors: 0,
        };
    }

    text.push_str(&format!(
        "\n{:<19}  {:<11}  {:<20}  {:<9}  {:<20}  {}\n",
        "TIME (UTC)", "AREA", "OPERATION", "RESULT", "TARGET", "DETAIL"
    ));
    for entry in &redacted {
        text.push_str(&format!(
            "{:<19}  {:<11}  {:<20}  {:<9}  {:<20}  {}\n",
            format_utc(entry.at_ms),
            entry.area.label(),
            op_label(&entry.op),
            entry.status.label(),
            entry.target.as_deref().unwrap_or(""),
            detail(entry)
        ));
    }

    if errors > 0 {
        text.push_str("\nERRORS\n");
        for entry in redacted
            .iter()
            .filter(|entry| entry.status == Status::Error)
        {
            text.push_str(&format!(
                "\n[{} UTC] {}{}\n",
                format_utc(entry.at_ms),
                op_label(&entry.op),
                entry
                    .target
                    .as_deref()
                    .map(|target| format!(" — {target}"))
                    .unwrap_or_default()
            ));
            text.push_str(entry.error.as_deref().unwrap_or("(no message)"));
            text.push('\n');
        }
    }

    Report {
        text,
        legend: redactor.legend().to_vec(),
        entries: redacted.len(),
        errors,
    }
}

/// O período coberto pelas entradas.
fn period(entries: &[Entry]) -> String {
    match (entries.first(), entries.last()) {
        (Some(first), Some(last)) => format!(
            "{} → {} UTC",
            format_utc(first.at_ms),
            format_utc(last.at_ms)
        ),
        _ => "—".to_string(),
    }
}

/// `detalhe · duração`, quando houver.
fn detail(entry: &Entry) -> String {
    let mut detail = entry.detail.clone().unwrap_or_default();
    if let Some(duration_ms) = entry.duration_ms {
        if !detail.is_empty() {
            detail.push_str(" · ");
        }
        detail.push_str(&format!("{duration_ms} ms"));
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::entry::Area;
    use crate::activity::redact::Context;
    use crate::config::model::{AiProfile, ServerConfigEntry};
    use std::path::{Path, PathBuf};

    const NOW: u64 = 20_763 * 86_400_000 + 23 * 3_600_000;

    fn server() -> ServerConfigEntry {
        ServerConfigEntry {
            name: "Local".to_string(),
            host: "127.0.0.1".to_string(),
            username: "postgres".to_string(),
            databases: vec!["mustel_test".to_string()],
            ..ServerConfigEntry::default()
        }
    }

    fn profile() -> AiProfile {
        AiProfile {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            ..AiProfile::default()
        }
    }

    fn report_for(entries: &[Entry], home: &Path, config: &Path) -> Report {
        let servers = [server()];
        let profiles = [profile()];
        let context = Context {
            servers: &servers,
            profiles: &profiles,
            home: Some(home),
            config: Some(config),
        };
        let redactor = Redactor::new(&context, entries);
        render(entries, &redactor, &AppInfo::current(), NOW)
    }

    fn entries() -> Vec<Entry> {
        vec![
            Entry::new(Area::Query, "query.run", Status::Ok)
                .at(NOW - 3_600_000)
                .with_target("Local")
                .with_detail("42 rows")
                .with_duration_ms(812)
                .with_sql("select * from clientes"),
            Entry::new(Area::Connections, "connection.test", Status::Error)
                .at(NOW - 60_000)
                .with_target("Local")
                .with_error("Connection failed for user postgres at 127.0.0.1"),
        ]
    }

    #[test]
    fn the_report_has_a_header_and_every_entry() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let report = report_for(&entries(), &home, &config);

        assert!(report.text.starts_with("Mustel report\n"));
        assert!(report.text.contains("Version:"));
        assert!(report.text.contains("System:"));
        assert!(report.text.contains("Entries: 2 (errors: 1)"));
        assert_eq!(report.entries, 2);
        assert_eq!(report.errors, 1);
        assert!(report.text.contains("42 rows · 812 ms"));
        assert!(report.text.contains("ERRORS"));
    }

    #[test]
    fn the_report_never_contains_the_real_names() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let report = report_for(&entries(), &home, &config);

        for original in [
            "Local",
            "127.0.0.1",
            "postgres",
            "mustel_test",
            "Ollama",
            "/home/ana",
        ] {
            assert!(
                !report.text.contains(original),
                "sobrou {original} no relatório:\n{}",
                report.text
            );
        }
        assert!(report.text.contains("server-1"));
        assert!(report.text.contains("host-1"));
        assert!(report.text.contains("user-1"));
    }

    #[test]
    fn sql_is_not_in_the_report() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let report = report_for(&entries(), &home, &config);

        assert!(!report.text.contains("select *"), "{}", report.text);
        assert!(!report.text.contains("clientes"), "{}", report.text);
    }

    #[test]
    fn the_legend_is_separate_from_the_text() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let report = report_for(&entries(), &home, &config);

        assert!(
            report
                .legend
                .iter()
                .any(|entry| entry.from == "Local" && entry.to == "server-1"),
            "a legenda guarda o valor real para a tela"
        );
        assert!(
            !report.text.contains("server-1 → Local"),
            "a legenda não entra no texto"
        );
    }

    #[test]
    fn an_empty_log_says_so() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let report = report_for(&[], &home, &config);

        assert!(report.text.contains("No activity recorded yet."));
        assert_eq!(report.entries, 0);
        assert_eq!(report.errors, 0);
        assert!(report.text.contains("Period: —"));
    }

    #[test]
    fn every_entry_appears_without_truncation() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let many: Vec<Entry> = (0..500)
            .map(|index| {
                Entry::new(Area::Query, "query.run", Status::Ok)
                    .at(NOW - 600_000 + index * 1_000)
                    .with_detail(format!("run {index}"))
            })
            .collect();

        let report = report_for(&many, &home, &config);

        for index in 0..500 {
            assert!(
                report.text.contains(&format!("run {index}\n")),
                "a entrada {index} sumiu do relatório"
            );
        }
        assert_eq!(report.entries, 500);
    }
}
