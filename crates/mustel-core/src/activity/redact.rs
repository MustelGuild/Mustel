//! Anonimização do relatório.
//!
//! Duas camadas, nesta ordem:
//!
//! 1. **literais conhecidos** — nome/host/usuário de cada servidor, bases citadas, perfis, home e
//!    diretório de config viram tokens estáveis (`server-1`, `host-1`, …);
//! 2. **padrões** — o que sobrar em texto livre é varrido por regex: identificadores entre aspas,
//!    URLs/connection strings, e-mails, IPs, caminhos Windows e POSIX.
//!
//! A legenda ([`Redactor::legend`]) guarda os pares reais → token **só para a tela**; ela nunca é
//! aplicada ao texto do relatório. O SQL é descartado antes de qualquer coisa.
//!
//! O redator é determinístico: a mesma configuração e as mesmas entradas geram os mesmos tokens.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::entry::Entry;
use crate::config::model::{AiProfile, ServerConfigEntry};

/// Um valor que virou token no relatório (para a legenda da tela).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// Categoria do valor (`"Server"`, `"Host"`, `"Database"`, `"Profile"`, `"Path"`).
    pub what: &'static str,
    /// O valor real (nunca vai ao relatório).
    pub from: String,
    /// O token que o substitui.
    pub to: String,
}

/// O ambiente conhecido, usado para montar o mapa de substituições.
pub struct Context<'a> {
    /// O cadastro de servidores.
    pub servers: &'a [ServerConfigEntry],
    /// Os perfis de IA.
    pub profiles: &'a [AiProfile],
    /// O diretório do usuário, quando conhecido.
    pub home: Option<&'a Path>,
    /// O diretório de configuração do Mustel, quando conhecido.
    pub config: Option<&'a Path>,
}

/// O mapa de anonimização de um relatório.
#[derive(Debug, Default)]
pub struct Redactor {
    /// `(de, para)`, ordenado do literal mais longo para o mais curto.
    literals: Vec<(String, String)>,
    legend: Vec<Replacement>,
}

impl Redactor {
    /// Monta o mapa a partir do ambiente e das entradas que serão exportadas.
    pub fn new(context: &Context<'_>, entries: &[Entry]) -> Self {
        let mut redactor = Redactor::default();

        let mut servers: Vec<&ServerConfigEntry> = context.servers.iter().collect();
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        for (index, server) in servers.iter().enumerate() {
            redactor.add("Server", &server.name, format!("server-{}", index + 1));
        }

        let hosts: BTreeSet<&str> = context
            .servers
            .iter()
            .map(|server| server.host.as_str())
            .filter(|host| !host.is_empty())
            .collect();
        for (index, host) in hosts.iter().enumerate() {
            let token = format!("host-{}", index + 1);
            redactor.add("Host", host, token.clone());
            for server in context
                .servers
                .iter()
                .filter(|server| server.host == **host)
            {
                redactor.add(
                    "Host",
                    &format!("{}:{}", server.host, server.port),
                    token.clone(),
                );
            }
        }

        let users: BTreeSet<&str> = context
            .servers
            .iter()
            .map(|server| server.username.as_str())
            .filter(|user| !user.is_empty())
            .collect();
        for (index, user) in users.iter().enumerate() {
            redactor.add("User", user, format!("user-{}", index + 1));
        }

        let mut databases: BTreeSet<String> = BTreeSet::new();
        for server in context.servers {
            for database in &server.databases {
                databases.insert(database.clone());
            }
        }
        for entry in entries {
            if let Some((_, database)) = entry
                .target
                .as_deref()
                .and_then(|target| target.split_once('/'))
            {
                databases.insert(database.to_string());
            }
        }
        for (index, database) in databases.iter().enumerate() {
            redactor.add("Database", database, format!("db-{}", index + 1));
        }

        let mut profiles: Vec<&AiProfile> = context.profiles.iter().collect();
        profiles.sort_by(|left, right| left.id.cmp(&right.id));
        for (index, profile) in profiles.iter().enumerate() {
            let token = format!("profile-{}", index + 1);
            redactor.add("Profile", &profile.id, token.clone());
            redactor.add("Profile", &profile.name, token);
        }

        if let Some(config) = context.config {
            redactor.add(
                "Path",
                &config.display().to_string(),
                "<config>".to_string(),
            );
        }
        if let Some(home) = context.home {
            redactor.add("Path", &home.display().to_string(), "<home>".to_string());
        }

        // O mais longo primeiro: "Local2" precisa ser trocado antes de "Local".
        redactor
            .literals
            .sort_by_key(|left| std::cmp::Reverse(left.0.len()));

        redactor
    }

    /// A legenda para exibir na tela (nunca aplicada ao texto do relatório).
    pub fn legend(&self) -> &[Replacement] {
        &self.legend
    }

    /// Aplica literais e padrões a um texto livre.
    pub fn apply(&self, text: &str) -> String {
        let mut redacted = text.to_string();
        for (from, to) in &self.literals {
            if !from.is_empty() {
                redacted = redacted.replace(from, to);
            }
        }

        redacted = QUOTED_DOUBLE.replace_all(&redacted, "\"…\"").into_owned();
        redacted = QUOTED_SINGLE.replace_all(&redacted, "'…'").into_owned();
        redacted = URL.replace_all(&redacted, "<url>").into_owned();
        redacted = EMAIL.replace_all(&redacted, "<email>").into_owned();
        redacted = IPV4.replace_all(&redacted, "<ip>").into_owned();
        redacted = IPV6.replace_all(&redacted, "<ip>").into_owned();
        redacted = WINDOWS_PATH.replace_all(&redacted, "<path>").into_owned();
        redacted = POSIX_PATH.replace_all(&redacted, "$1<path>").into_owned();

        redacted
    }

    /// Uma cópia da entrada pronta para o relatório: campos redigidos e SQL fora.
    pub fn report_entry(&self, entry: &Entry) -> Entry {
        let mut redacted = entry.clone();
        redacted.target = entry.target.as_deref().map(|target| self.apply(target));
        redacted.detail = entry.detail.as_deref().map(|detail| self.apply(detail));
        redacted.error = entry.error.as_deref().map(|error| self.apply(error));
        redacted.sql = None;
        redacted
    }

    /// Registra um literal (ignora vazio e duplicado; o primeiro token vence).
    fn add(&mut self, what: &'static str, from: &str, to: String) {
        if from.trim().is_empty() {
            return;
        }
        if self.literals.iter().any(|(existing, _)| existing == from) {
            return;
        }
        self.literals.push((from.to_string(), to.clone()));
        self.legend.push(Replacement {
            what,
            from: from.to_string(),
            to,
        });
    }
}

/// Identificadores entre aspas duplas (`relation "clientes"`) perdem o conteúdo.
static QUOTED_DOUBLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""(?:[^"\\\n]|\\.)*""#).expect("padrão constante"));
/// Valores entre aspas simples (`password 'segredo'`) perdem o conteúdo.
static QUOTED_SINGLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"'(?:[^'\\\n]|\\.)*'").expect("padrão constante"));
/// URLs e connection strings (`postgres://...`).
static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[A-Za-z][A-Za-z0-9+.-]*://[^\s"'<>]+"#).expect("padrão constante")
});
/// Endereços de e-mail.
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("padrão constante")
});
/// Endereços IPv4.
static IPV4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("padrão constante"));
/// Endereços IPv6 (três ou mais grupos, para não confundir com horários).
static IPV6: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[0-9A-Fa-f]{1,4}:){3,7}[0-9A-Fa-f]{0,4}\b").expect("padrão constante")
});
/// Caminhos absolutos do Windows (`C:\Users\...`).
static WINDOWS_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b[A-Za-z]:\\[^\s"'<>]+"#).expect("padrão constante"));
/// Caminhos absolutos POSIX (`/home/...`), preservando o separador que vem antes.
static POSIX_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(^|[\s(])(/(?:[^\s"'<>/]+/)*[^\s"'<>/ ]+)"#).expect("padrão constante")
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::entry::{Area, Status};
    use std::path::PathBuf;

    fn server(name: &str, host: &str, user: &str, databases: &[&str]) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: host.to_string(),
            username: user.to_string(),
            databases: databases
                .iter()
                .map(|database| (*database).to_string())
                .collect(),
            ..ServerConfigEntry::default()
        }
    }

    fn context<'a>(
        servers: &'a [ServerConfigEntry],
        profiles: &'a [AiProfile],
        home: &'a Path,
        config: &'a Path,
    ) -> Context<'a> {
        Context {
            servers,
            profiles,
            home: Some(home),
            config: Some(config),
        }
    }

    fn entries() -> Vec<Entry> {
        vec![
            Entry::new(Area::Query, "query.run", Status::Ok)
                .with_target("Local/mustel_test")
                .with_detail("42 rows")
                .with_sql("select * from clientes"),
        ]
    }

    fn sample<'a>(
        servers: &'a [ServerConfigEntry],
        profiles: &'a [AiProfile],
        home: &'a Path,
        config: &'a Path,
    ) -> (Redactor, Vec<Entry>) {
        let entries = entries();
        let redactor = Redactor::new(&context(servers, profiles, home, config), &entries);
        (redactor, entries)
    }

    /// CONTRACT: IF-267
    #[test]
    fn known_names_become_stable_tokens() {
        let servers = [server("Local", "127.0.0.1", "postgres", &["mustel_test"])];
        let profiles = [AiProfile {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            ..AiProfile::default()
        }];
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, _) = sample(&servers, &profiles, &home, &config);

        let text = "Local 127.0.0.1 127.0.0.1:5432 postgres mustel_test Ollama ollama";
        let redacted = redactor.apply(text);

        for original in [
            "Local",
            "127.0.0.1",
            "postgres",
            "mustel_test",
            "Ollama",
            "ollama",
        ] {
            assert!(
                !redacted.contains(original),
                "sobrou {original}: {redacted}"
            );
        }
        assert!(redacted.contains("server-1"));
        assert!(redacted.contains("host-1"));
        assert!(redacted.contains("user-1"));
        assert!(redacted.contains("db-1"));
        assert!(redacted.contains("profile-1"));
    }

    #[test]
    fn the_longest_literal_wins() {
        let servers = [
            server("Local", "127.0.0.1", "postgres", &[]),
            server("Local2", "127.0.0.2", "postgres", &[]),
        ];
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, _) = sample(&servers, &[], &home, &config);

        let redacted = redactor.apply("Local2 e Local");

        assert_eq!(redacted, "server-2 e server-1");
    }

    #[test]
    fn free_text_patterns_are_scrubbed() {
        let servers = [server("Local", "127.0.0.1", "postgres", &[])];
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, _) = sample(&servers, &[], &home, &config);

        let text = concat!(
            r#"connection to server at "10.1.2.3", port 5432 failed: "#,
            r#"relation "clientes" does not exist; "#,
            "postgres://ana:segredo@db.interno:5432/vendas; ",
            r"C:\Users\ana\app.log; ",
            "https://api.example.com/v1; ana@example.com; ",
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
        );
        let redacted = redactor.apply(text);

        for original in [
            "10.1.2.3",
            "clientes",
            "segredo",
            "db.interno",
            "C:\\Users\\ana",
            "https://api.example.com",
            "ana@example.com",
            "2001:0db8",
        ] {
            assert!(
                !redacted.contains(original),
                "sobrou {original}: {redacted}"
            );
        }
        assert!(redacted.contains("<ip>"));
        assert!(redacted.contains("<url>"));
        assert!(redacted.contains("<email>"));
        assert!(redacted.contains("<path>"));
    }

    #[test]
    fn quoted_identifiers_lose_their_content() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, _) = sample(&[], &[], &home, &config);

        assert_eq!(
            redactor.apply(r#"relation "clientes" does not exist"#),
            r#"relation "…" does not exist"#
        );
    }

    #[test]
    fn the_home_and_config_directories_are_tokens() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, _) = sample(&[], &[], &home, &config);

        let redacted = redactor.apply("/home/ana/.config/Mustel/mustel.jsonc e /home/ana/docs");

        assert!(redacted.contains("<config>"), "{redacted}");
        assert!(redacted.contains("<home>"), "{redacted}");
        assert!(!redacted.contains("/home/ana"), "{redacted}");
    }

    #[test]
    fn sql_is_dropped_from_the_report_entry() {
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (redactor, entries) = sample(&[], &[], &home, &config);

        let report = redactor.report_entry(&entries[0]);

        assert!(report.sql.is_none(), "SQL nunca vai ao relatório");
        assert_eq!(report.detail.as_deref(), Some("42 rows"));
    }

    #[test]
    fn the_legend_is_local_and_deterministic() {
        let servers = [server("Local", "127.0.0.1", "postgres", &["mustel_test"])];
        let home = PathBuf::from("/home/ana");
        let config = PathBuf::from("/home/ana/.config/Mustel");
        let (first, _) = sample(&servers, &[], &home, &config);
        let (second, _) = sample(&servers, &[], &home, &config);

        assert_eq!(first.legend(), second.legend());
        assert!(
            first
                .legend()
                .iter()
                .any(|entry| entry.what == "Server" && entry.from == "Local"),
            "a legenda mostra o valor real só para a tela"
        );
    }
}
