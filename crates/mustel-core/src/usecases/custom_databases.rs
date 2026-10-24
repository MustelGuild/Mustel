//! Seleção de bases a partir de uma **query** contra um banco do cadastro.
//!
//! O dono guarda os tenants e suas connection strings num banco (o "configurador"). Esta é a camada
//! que lê esse resultado e o transforma em *quais bases de quais servidores* — sem UI, para ser
//! testável e para a janela só desenhar.
//!
//! Três passos, nesta ordem: [`load`] roda o SQL e lê as linhas, [`validate`] casa cada linha com um
//! servidor **do cadastro** e aplica os `excludePatterns` dele, e [`resolve_credential`] decide
//! usuário/senha. A validação nunca inventa servidor: host fora do cadastro é erro.

use std::collections::BTreeMap;

use crate::config::ServerConfigEntry;
use crate::error::CoreError;
use crate::postgres::connection_string;
use crate::query::discovery;

/// Uma linha lida da query de seleção, já normalizada.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SelectionRow {
    /// Host do servidor (obrigatório para a linha valer).
    pub host: Option<String>,
    /// Porta (ausente = a do servidor cadastrado que casar).
    pub port: Option<u16>,
    /// Banco alvo (obrigatório para a linha valer).
    pub database: Option<String>,
    /// Usuário; ausente = o do cadastro.
    pub user: Option<String>,
    /// Senha; **só em memória**, nunca gravada, logada ou impressa.
    pub password: Option<String>,
    /// Nome do servidor cadastrado, quando a query traz a coluna `server`.
    pub server: Option<String>,
}

/// `Debug` sem a senha: a struct circula por código que pode despejar estado num log.
impl core::fmt::Debug for SelectionRow {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SelectionRow")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("server", &self.server)
            .field("password", &self.password.as_ref().map(|_| "<oculta>"))
            .finish()
    }
}

/// Valida as linhas contra o cadastro e agrupa por servidor.
///
/// **Não inventa servidor:** host fora do cadastro aborta a carga (D9), e host+porta ambíguo também —
/// o dono tem dois servidores no mesmo `127.0.0.1:5432`, então adivinhar seria pior do que pedir a
/// coluna `server`.
pub fn validate(
    rows: &[SelectionRow],
    servers: &[ServerConfigEntry],
) -> Result<Vec<ServerSelection>, SelectionError> {
    let mut unknown_hosts: Vec<String> = Vec::new();
    let mut unknown_rows = 0usize;
    let mut ambiguous: Option<SelectionError> = None;
    let mut unknown_server: Option<String> = None;
    let mut by_server: BTreeMap<String, Vec<DatabaseChoice>> = BTreeMap::new();

    for row in rows {
        let Some(database) = row.database.as_deref() else {
            continue;
        };

        let server = match &row.server {
            // A coluna `server` manda: casa pelo **nome**, sem heurística.
            Some(name) => match servers
                .iter()
                .find(|server| server.name.eq_ignore_ascii_case(name))
            {
                Some(server) => server,
                None => {
                    unknown_server.get_or_insert_with(|| name.clone());
                    continue;
                }
            },
            None => {
                let host = row.host.as_deref().unwrap_or_default();
                let port = row.port;
                let matches: Vec<&ServerConfigEntry> = servers
                    .iter()
                    .filter(|server| server.host.eq_ignore_ascii_case(host))
                    .filter(|server| port.is_none_or(|port| server.port == port))
                    .collect();

                match matches.len() {
                    0 => {
                        let label = match port {
                            Some(port) => format!("{host}:{port}"),
                            None => host.to_string(),
                        };
                        if !unknown_hosts.iter().any(|known| known == &label) {
                            unknown_hosts.push(label);
                        }
                        unknown_rows += 1;
                        continue;
                    }
                    1 => matches[0],
                    _ => {
                        ambiguous.get_or_insert(SelectionError::AmbiguousServer {
                            host: host.to_string(),
                            port: port.unwrap_or(5432),
                            candidates: matches.iter().map(|server| server.name.clone()).collect(),
                        });
                        continue;
                    }
                }
            }
        };

        // Aviso de credencial (D8): usuário diferente do cadastro e sem senha na linha.
        let warning = match (&row.user, &row.password) {
            (Some(user), None) if !user.eq_ignore_ascii_case(&server.username) => Some(format!(
                "user '{user}' has no password in the query; the vault only holds '{0}'",
                server.username
            )),
            _ => None,
        };

        let blocked_by = server
            .exclude_patterns
            .iter()
            .find(|pattern| discovery::matches_pattern(database, pattern))
            .cloned();

        let choices = by_server.entry(server.name.clone()).or_default();
        if !choices.iter().any(|choice| choice.name == database) {
            choices.push(DatabaseChoice {
                name: database.to_string(),
                blocked_by,
                warning,
            });
        }
    }

    if !unknown_hosts.is_empty() {
        return Err(SelectionError::UnknownHosts {
            hosts: unknown_hosts,
            rows: unknown_rows,
        });
    }
    if let Some(name) = unknown_server {
        return Err(SelectionError::UnknownServer { name });
    }
    if let Some(error) = ambiguous {
        return Err(error);
    }
    if by_server.is_empty() {
        return Err(SelectionError::Empty);
    }

    Ok(by_server
        .into_iter()
        .map(|(server, databases)| ServerSelection { server, databases })
        .collect())
}

/// Quem autentica numa linha da seleção (D7).
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedCredential {
    /// Usuário que vai conectar.
    pub user: String,
    /// Senha: da linha quando houver; senão a do cofre.
    pub password: Option<String>,
    /// Aviso quando a senha do cofre não serve para o usuário pedido (D8).
    pub warning: Option<String>,
}

/// `Debug` sem a senha, pela mesma razão do [`SelectionRow`].
impl core::fmt::Debug for ResolvedCredential {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ResolvedCredential")
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "<oculta>"))
            .field("warning", &self.warning)
            .finish()
    }
}

/// Decide usuário e senha de uma linha, na ordem que o dono definiu: **o que vier da linha vale**;
/// o que faltar sai do cadastro + cofre.
pub fn resolve_credential(
    row: &SelectionRow,
    server: &ServerConfigEntry,
    stored_password: Option<&str>,
) -> ResolvedCredential {
    let user = row.user.clone().unwrap_or_else(|| server.username.clone());

    if let Some(password) = row.password.clone() {
        return ResolvedCredential {
            user,
            password: Some(password),
            warning: None,
        };
    }

    if user.eq_ignore_ascii_case(&server.username) {
        return ResolvedCredential {
            user,
            password: stored_password.map(str::to_string),
            warning: None,
        };
    }

    ResolvedCredential {
        warning: Some(format!(
            "user '{user}' has no password in the query; the vault only holds '{0}'",
            server.username
        )),
        user,
        password: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, host: &str, port: u16) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: host.to_string(),
            port,
            username: "postgres".to_string(),
            exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
            ..ServerConfigEntry::default()
        }
    }

    /// Cadastro com o caso real do dono: dois servidores no mesmo host e porta.
    fn registry() -> Vec<ServerConfigEntry> {
        vec![
            server("Local", "127.0.0.1", 5432),
            server("Local 2", "127.0.0.1", 5432),
            server("Producao", "10.0.0.5", 5432),
        ]
    }

    fn row(host: &str, database: &str) -> SelectionRow {
        SelectionRow {
            host: Some(host.to_string()),
            database: Some(database.to_string()),
            ..SelectionRow::default()
        }
    }

    /// Host fora do cadastro **aborta** e a mensagem agrega os distintos (D9).
    /// CONTRACT: IF-224
    #[test]
    fn an_unregistered_host_aborts_with_an_aggregated_message() {
        let rows = vec![
            row("10.0.0.9", "a"),
            row("10.0.0.9", "b"),
            row("10.0.0.7", "c"),
        ];

        let error = validate(&rows, &registry()).expect_err("host desconhecido aborta");
        let text = error.to_string();
        assert!(text.contains("10.0.0.9"), "lista o host: {text}");
        assert!(text.contains("10.0.0.7"), "lista o outro host: {text}");
        assert!(text.contains("3 row(s)"), "conta as linhas: {text}");
        assert_eq!(text.matches("10.0.0.9").count(), 1, "não repete o host");
    }

    /// Host+porta ambíguo pede a coluna `server` em vez de adivinhar.
    /// CONTRACT: IF-224
    #[test]
    fn an_ambiguous_host_aborts_naming_the_candidates() {
        let rows = vec![row("127.0.0.1", "dev")];

        let error = validate(&rows, &registry()).expect_err("ambíguo aborta");
        let text = error.to_string();
        assert!(
            text.contains("Local") && text.contains("Local 2"),
            "candidatos: {text}"
        );
        assert!(text.contains("server"), "ensina a saída: {text}");
    }

    /// A porta desempata o caso homônimo, e a coluna `server` tem precedência.
    /// CONTRACT: IF-224
    #[test]
    fn port_and_server_column_disambiguate() {
        let mut other = server("Outro", "127.0.0.1", 5433);
        other.exclude_patterns = Vec::new();
        let mut registry = registry();
        registry.push(other);

        let rows = vec![SelectionRow {
            host: Some("127.0.0.1".to_string()),
            port: Some(5433),
            database: Some("dev".to_string()),
            ..SelectionRow::default()
        }];
        let selection = validate(&rows, &registry).expect("porta desempata");
        assert_eq!(selection[0].server, "Outro");

        let rows = vec![SelectionRow {
            server: Some("producao".to_string()),
            host: Some("127.0.0.1".to_string()),
            database: Some("dev".to_string()),
            ..SelectionRow::default()
        }];
        let selection = validate(&rows, &registry).expect("nome manda");
        assert_eq!(selection[0].server, "Producao");
    }

    /// `excludePatterns` marcam a base como barrada, sem derrubar a carga (D10).
    /// CONTRACT: IF-225
    #[test]
    fn excluded_databases_come_back_marked() {
        let mut registry = registry();
        registry[2].exclude_patterns = vec!["template*".to_string(), "postgres".to_string()];
        let rows = vec![
            row("10.0.0.5", "producao"),
            row("10.0.0.5", "postgres"),
            row("10.0.0.5", "template_zero"),
        ];

        let selection = validate(&rows, &registry).expect("valida");
        let databases = &selection[0].databases;

        assert_eq!(databases[0].blocked_by, None, "produção passa");
        assert_eq!(
            databases[1].blocked_by.as_deref(),
            Some("postgres"),
            "`postgres` é barrada pelo padrão"
        );
        assert_eq!(
            databases[2].blocked_by.as_deref(),
            Some("template*"),
            "`template*` casa por prefixo"
        );
    }

    /// A mesma base repetida na query entra uma vez só.
    #[test]
    fn repeated_databases_are_deduplicated() {
        let rows = vec![row("10.0.0.5", "producao"), row("10.0.0.5", "producao")];
        let selection = validate(&rows, &registry()).expect("valida");
        assert_eq!(selection[0].databases.len(), 1);
    }

    /// Usuário diferente do cadastro **sem** senha na linha vira aviso (D8).
    /// CONTRACT: IF-226
    #[test]
    fn a_different_user_without_a_password_is_a_warning() {
        let mut rows = vec![row("10.0.0.5", "producao")];
        rows[0].user = Some("ana".to_string());

        let selection = validate(&rows, &registry()).expect("valida");
        let warning = selection[0].databases[0]
            .warning
            .as_deref()
            .expect("tem aviso");
        assert!(warning.contains("ana"), "nomeia o usuário: {warning}");

        // Com senha na linha, sem aviso.
        let mut rows = vec![row("10.0.0.5", "producao")];
        rows[0].user = Some("ana".to_string());
        rows[0].password = Some("segredo".to_string());
        let selection = validate(&rows, &registry()).expect("valida");
        assert_eq!(selection[0].databases[0].warning, None);
    }

    /// Matriz de credencial (D7): o que vier da linha vale; o que faltar vem do cadastro + cofre.
    /// CONTRACT: IF-226
    #[test]
    fn the_credential_matrix_follows_the_decision() {
        let server = server("Producao", "10.0.0.5", 5432);

        // Usuário e senha na linha.
        let row = SelectionRow {
            user: Some("ana".to_string()),
            password: Some("da-linha".to_string()),
            ..SelectionRow::default()
        };
        let credential = resolve_credential(&row, &server, Some("do-cofre"));
        assert_eq!(credential.user, "ana");
        assert_eq!(credential.password.as_deref(), Some("da-linha"));

        // Mesmo usuário do cadastro, sem senha: cofre.
        let row = SelectionRow {
            user: Some("postgres".to_string()),
            ..SelectionRow::default()
        };
        let credential = resolve_credential(&row, &server, Some("do-cofre"));
        assert_eq!(credential.password.as_deref(), Some("do-cofre"));

        // Nada na linha: cadastro + cofre.
        let credential = resolve_credential(&SelectionRow::default(), &server, Some("do-cofre"));
        assert_eq!(credential.user, "postgres");
        assert_eq!(credential.password.as_deref(), Some("do-cofre"));
        assert_eq!(credential.warning, None);

        // Usuário diferente sem senha: não usa a senha de outro usuário.
        let row = SelectionRow {
            user: Some("ana".to_string()),
            ..SelectionRow::default()
        };
        let credential = resolve_credential(&row, &server, Some("do-cofre"));
        assert_eq!(
            credential.password, None,
            "senha de outro usuário não serve"
        );
        assert!(credential.warning.is_some());
    }

    /// A senha não aparece em `Debug` de nenhum dos tipos do fluxo.
    /// CONTRACT: IF-226
    #[test]
    fn debug_never_shows_the_password() {
        let row = SelectionRow {
            host: Some("10.0.0.5".to_string()),
            database: Some("producao".to_string()),
            password: Some("segredo".to_string()),
            ..SelectionRow::default()
        };
        let credential = resolve_credential(&row, &server("P", "10.0.0.5", 5432), None);

        for text in [format!("{row:?}"), format!("{credential:?}")] {
            assert!(!text.contains("segredo"), "senha vazou: {text}");
        }
    }
}

/// Uma base e por que (não) entra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseChoice {
    /// Nome do banco.
    pub name: String,
    /// `Some(padrão)` quando um `excludePatterns` do servidor barrou a base.
    pub blocked_by: Option<String>,
    /// Aviso não bloqueante (ex.: usuário da string sem senha).
    pub warning: Option<String>,
}

/// O que a seleção diz para um servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSelection {
    /// Nome do servidor **do cadastro**.
    pub server: String,
    /// Bases que a query trouxe para ele, na ordem em que apareceram.
    pub databases: Vec<DatabaseChoice>,
}

/// Por que a validação falhou.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// Hosts que não existem no cadastro (a carga inteira é abortada).
    #[error("{} host(s) not registered ({} row(s)): {} — register them in `mustel settings db-servers add`",
        hosts.len(), rows, hosts.join(", "))]
    UnknownHosts {
        /// Hosts distintos encontrados, na ordem.
        hosts: Vec<String>,
        /// Quantas linhas dependiam deles.
        rows: usize,
    },
    /// Um mesmo host+porta casa com mais de um servidor cadastrado.
    #[error("host {}:{} matches more than one registered server ({}): add a `server` column to the query",
        host, port, candidates.join(", "))]
    AmbiguousServer {
        /// Host da linha.
        host: String,
        /// Porta da linha.
        port: u16,
        /// Servidores que casaram.
        candidates: Vec<String>,
    },
    /// A coluna `server` trouxe um nome que não está no cadastro.
    #[error("server '{}' from the `server` column is not registered", name)]
    UnknownServer {
        /// Nome que veio na linha.
        name: String,
    },
    /// Nenhuma linha aproveitável.
    #[error("the selection query returned no usable row")]
    Empty,
}

/// Roda o SQL e devolve as linhas normalizadas.
///
/// Linha sem `host` ou sem `database` é descartada e **contada** em `ignored_rows` — o status da tela
/// precisa desse número para não esconder lixo.
pub async fn load(
    client: &tokio_postgres::Client,
    sql: &str,
) -> Result<LoadedSelection, CoreError> {
    let rows = client
        .query(sql, &[])
        .await
        .map_err(|error| CoreError::InvalidArgument(format!("selection query failed: {error}")))?;

    let mut selection = LoadedSelection::default();
    for row in &rows {
        let parsed = row_to_selection(row);
        // Sem base não há o que marcar. Sem host **nem** nome de servidor não há como identificar
        // quem é — a coluna `server` sozinha basta (D9), porque ela aponta para o cadastro.
        if parsed.database.is_some() && (parsed.host.is_some() || parsed.server.is_some()) {
            selection.rows.push(parsed);
        } else {
            selection.ignored_rows += 1;
        }
    }

    Ok(selection)
}

/// O que [`load`] devolveu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedSelection {
    /// Linhas aproveitáveis.
    pub rows: Vec<SelectionRow>,
    /// Quantas linhas foram descartadas por falta de host ou de banco.
    pub ignored_rows: usize,
}

/// Nomes de coluna que carregam a connection string inteira.
const CONNECTION_STRING_COLUMNS: [&str; 4] = [
    "connection_string",
    "connectionstring",
    "conn_str",
    "connstr",
];

/// Converte uma linha do driver em [`SelectionRow`].
///
/// Ordem: coluna de connection string (nominal) → colunas soltas → texto com `=` na primeira coluna
/// como último recurso (a query pode devolver só a string, sem nome de coluna combinado).
fn row_to_selection(row: &tokio_postgres::Row) -> SelectionRow {
    let columns = row.columns();

    if let Some(index) = find_column(columns, &CONNECTION_STRING_COLUMNS)
        && let Some(text) = text_at(row, Some(index))
    {
        let mut parsed = from_connection_string(&text);
        // A coluna `server` continua valendo ao lado da string: é ela que desempata dois servidores
        // no mesmo host e porta (D9), e a string — por definição — não sabe o nome do cadastro.
        parsed.server = text_at(
            row,
            find_column(columns, &["server", "server_name", "servername"]),
        );
        return parsed;
    }

    let mut parsed = SelectionRow {
        // `server` **não** é alias de host: é o nome do servidor cadastrado (D9), a saída para o
        // caso de dois servidores no mesmo host e porta. O alias `Server=` só vale na connection
        // string, onde o parser o resolve.
        host: text_at(row, find_column(columns, &["host", "data source"])),
        port: text_at(row, find_column(columns, &["port"])).and_then(|text| text.parse().ok()),
        database: text_at(row, find_column(columns, &["database", "initial catalog"])),
        user: text_at(
            row,
            find_column(columns, &["username", "user id", "userid", "user"]),
        ),
        password: text_at(row, find_column(columns, &["password", "pwd"])),
        server: text_at(
            row,
            find_column(columns, &["server", "server_name", "servername"]),
        ),
    };

    // Último recurso: a primeira coluna traz uma connection string sem nome combinado.
    if (parsed.host.is_none() || parsed.database.is_none())
        && let Some(text) = text_at(row, Some(0))
        && text.contains('=')
    {
        let from_string = from_connection_string(&text);
        parsed.host = parsed.host.or(from_string.host);
        parsed.port = parsed.port.or(from_string.port);
        parsed.database = parsed.database.or(from_string.database);
        parsed.user = parsed.user.or(from_string.user);
        parsed.password = parsed.password.or(from_string.password);
    }
    parsed
}

/// A partir de uma connection string Npgsql.
fn from_connection_string(text: &str) -> SelectionRow {
    let parsed = connection_string::parse(text);
    SelectionRow {
        host: parsed.host,
        port: parsed.port,
        database: parsed.database,
        user: parsed.user,
        password: parsed.password,
        server: None,
    }
}

/// Índice da primeira coluna que casa com um dos nomes (sem diferenciar maiúsculas).
fn find_column(columns: &[tokio_postgres::Column], names: &[&str]) -> Option<usize> {
    columns.iter().position(|column| {
        names
            .iter()
            .any(|name| column.name().eq_ignore_ascii_case(name))
    })
}

/// Lê uma coluna como texto, aceitando também inteiros (a porta pode vir de `port int`).
fn text_at(row: &tokio_postgres::Row, index: Option<usize>) -> Option<String> {
    let index = index?;

    if let Ok(value) = row.try_get::<usize, Option<String>>(index) {
        return value
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
    }
    if let Ok(value) = row.try_get::<usize, Option<i64>>(index) {
        return value.map(|number| number.to_string());
    }
    if let Ok(value) = row.try_get::<usize, Option<i32>>(index) {
        return value.map(|number| number.to_string());
    }
    if let Ok(value) = row.try_get::<usize, Option<i16>>(index) {
        return value.map(|number| number.to_string());
    }

    None
}
