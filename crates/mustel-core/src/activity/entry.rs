//! O modelo de um evento do log de atividade.
//!
//! Uma entrada é deliberadamente **estruturada**: a aba Activity a exibe em tabela, o relatório a
//! reconstrói em texto e o redator a anonimiza campo a campo. O `op` é um identificador estável
//! (`"query.run"`), não um texto de tela — assim o formato do arquivo sobrevive a mudanças de
//! redação, e [`op_label`] decide o rótulo mostrado.

use serde::{Deserialize, Serialize};

/// A área do produto a que o evento pertence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// Ciclo da aplicação (abriu, fechou, panic, configuração).
    App,
    /// Execução de consulta.
    Query,
    /// Backup e restore.
    Database,
    /// Cadastro de servidores e teste de conexão.
    Connections,
    /// Aba Settings.
    Settings,
    /// Painel de IA.
    Ai,
    /// Queries de seleção.
    Selector,
}

impl Area {
    /// Rótulo curto para a tabela e o relatório.
    pub fn label(self) -> &'static str {
        match self {
            Area::App => "App",
            Area::Query => "Query",
            Area::Database => "Database",
            Area::Connections => "Connections",
            Area::Settings => "Settings",
            Area::Ai => "AI",
            Area::Selector => "Selector",
        }
    }
}

/// O desfecho do evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Terminou bem.
    Ok,
    /// Falhou.
    Error,
    /// Foi cancelado pelo usuário.
    Cancelled,
    /// Não é sucesso nem falha (abertura/fechamento do app).
    Info,
}

impl Status {
    /// Rótulo curto para a tabela e o relatório.
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Error => "Error",
            Status::Cancelled => "Cancelled",
            Status::Info => "Info",
        }
    }
}

/// Um evento do log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Instante em milissegundos desde 1970-01-01T00:00:00Z.
    pub at_ms: u64,
    /// Área do produto.
    pub area: Area,
    /// Identificador estável da operação (`"query.run"`, `"connection.test"`, …).
    pub op: String,
    /// Desfecho.
    pub status: Status,
    /// Alvo (servidor, perfil, `"servidor/base"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Resumo curto (`"42 rows"`, `"3 databases"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Mensagem de erro, quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Prévia do SQL — **só local**; o relatório descarta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    /// Quanto durou, quando o chamador mede.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl Entry {
    /// Cria a entrada com o instante atual.
    pub fn new(area: Area, op: impl Into<String>, status: Status) -> Self {
        Self {
            at_ms: crate::datetime::now_ms(),
            area,
            op: op.into(),
            status,
            target: None,
            detail: None,
            error: None,
            sql: None,
            duration_ms: None,
        }
    }

    /// Usa um instante explícito (testes e importação).
    pub fn at(mut self, at_ms: u64) -> Self {
        self.at_ms = at_ms;
        self
    }

    /// Define o alvo.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Define o resumo.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Define a mensagem de erro.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Guarda a prévia do SQL (só local).
    pub fn with_sql(mut self, sql: &str) -> Self {
        self.sql = Some(sql_preview(sql));
        self
    }

    /// Define a duração.
    pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Rótulo de exibição de uma operação; desconhecidas caem no próprio identificador.
pub fn op_label(op: &str) -> &str {
    match op {
        "app.start" => "App started",
        "app.stop" => "App closed",
        "app.panic" => "Crash",
        "app.config" => "Configuration",
        "app.report" => "Report",
        "query.run" => "Query",
        "database.backup" => "Backup",
        "database.restore" => "Restore",
        "database.pgpass.add" => "pgpass add",
        "database.pgpass.list" => "pgpass list",
        "database.pgpass.remove" => "pgpass remove",
        "settings.db_servers.ls" => "Server list",
        "settings.db_servers.add" | "server.save" => "Save server",
        "settings.db_servers.rm" | "server.remove" => "Remove server",
        "settings.db_servers.test" | "connection.test" => "Connection test",
        "settings.db_servers.set_password" | "password.save" => "Save password",
        "schema.load" => "Schema",
        "databases.list" => "Database list",
        "ai.generate" => "AI: SQL",
        "ai.ddl" => "AI: schema",
        "ai.test" => "AI test",
        "ai.profiles.save" => "Save AI profiles",
        "selector.test" => "Selector test",
        "selector.apply" => "Apply selector",
        "app.update.check" => "Update check",
        "app.update.download" => "Update download",
        "app.update.apply" => "Update install",
        "job.failed" => "Job",
        "cli.parse" => "Command line",
        other => other,
    }
}

/// Prévia de SQL: uma linha só e no máximo 200 caracteres (o suficiente para reconhecer a consulta).
pub fn sql_preview(sql: &str) -> String {
    let collapsed = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview: String = collapsed.chars().take(200).collect();
    if collapsed.chars().count() > 200 {
        preview.push('…');
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: u64 = 20_763 * 86_400_000 + 3_600_000;

    #[test]
    fn an_entry_round_trips_through_json() {
        let entry = Entry::new(Area::Query, "query.run", Status::Ok)
            .at(AT)
            .with_target("Local")
            .with_detail("42 rows")
            .with_duration_ms(812);

        let line = serde_json::to_string(&entry).expect("serializa");
        let back: Entry = serde_json::from_str(&line).expect("lê de volta");

        assert_eq!(back, entry);
        assert!(line.contains("\"area\":\"query\""));
        assert!(line.contains("\"status\":\"ok\""));
    }

    #[test]
    fn empty_optional_fields_stay_out_of_the_file() {
        let line = serde_json::to_string(&Entry::new(Area::App, "app.start", Status::Info))
            .expect("serializa");

        assert!(!line.contains("target"));
        assert!(!line.contains("error"));
        assert!(!line.contains("sql"));
        assert!(!line.contains("duration_ms"));
    }

    #[test]
    fn unknown_operations_fall_back_to_their_identifier() {
        assert_eq!(op_label("query.run"), "Query");
        assert_eq!(op_label("cli.parse"), "Command line");
        assert_eq!(op_label("database.backup"), "Backup");
        assert_eq!(op_label("future.op"), "future.op");
    }

    #[test]
    fn a_long_sql_becomes_a_single_short_line() {
        let sql = format!("select\n  {}\nfrom t", "coluna ".repeat(80));

        let preview = sql_preview(&sql);

        assert!(!preview.contains('\n'), "uma linha só");
        assert!(preview.ends_with('…'), "marca o corte");
        assert_eq!(preview.chars().count(), 201);
    }

    #[test]
    fn a_short_sql_is_kept_as_is() {
        assert_eq!(sql_preview("select 1"), "select 1");
    }
}
