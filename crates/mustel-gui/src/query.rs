//! `query run` a partir da janela (T41).
//!
//! Monta a **entrada resolvida** que o caso de uso pede — servidores com a conexão pronta, sql,
//! diretório de saída, paralelismo, timeout — a partir do cadastro e da senha do cofre. Nenhuma regra
//! é duplicada aqui: resolver a conexão é traduzir o cadastro para `ConnectionSettings`, e quem
//! executa é [`mustel_core::usecases::query_run`].
//!
//! A senha vem do **cofre**, como no CLI quando não há `-W`; se não houver senha guardada, a janela
//! avisa e não pergunta — a GUI não tem prompt de senha (isso é o T42).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::query::plan::ServerRun;
use mustel_core::usecases::query_run::{self, QueryRunRequest};

/// Timeout de comando padrão (o mesmo `--command-timeout` do legado).
const DEFAULT_COMMAND_TIMEOUT_SECONDS: u64 = 300;

/// Por que não deu para montar a entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// O servidor não tem senha guardada no cofre.
    NoPassword(String),
    /// O cadastro não pôde ser lido.
    Config(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPassword(server) => write!(
                formatter,
                "No password stored for '{server}'. Use `mustel settings db-servers set-password`."
            ),
            Self::Config(message) => write!(formatter, "{message}"),
        }
    }
}

/// Monta a requisição de `query run` para um servidor e uma consulta.
pub fn request_for(
    server: &ServerConfigEntry,
    sql: &str,
    password: &str,
) -> Result<QueryRunRequest, InputError> {
    let store =
        ConfigStore::with_default_path().map_err(|error| InputError::Config(error.to_string()))?;
    let defaults =
        query_run::defaults(&store).map_err(|error| InputError::Config(error.to_string()))?;

    Ok(QueryRunRequest {
        servers: vec![server_run(server, password)],
        sql: Arc::from(sql),
        output_directory: PathBuf::from(defaults.output_directory),
        max_parallelism: defaults.max_parallelism as usize,
        command_timeout: Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECONDS),
        all: server.fetch_all_databases,
        exclude: HashSet::new(),
    })
}

/// A conexão resolvida de um servidor do cadastro, sem banco (o núcleo troca por alvo).
fn server_run(server: &ServerConfigEntry, password: &str) -> ServerRun {
    ServerRun {
        name: server.name.clone(),
        max_parallelism: server.max_parallelism as usize,
        databases: server.databases.clone(),
        exclude_patterns: server.exclude_patterns.clone(),
        fetch_all_databases: server.fetch_all_databases,
        settings: ConnectionSettings {
            host: server.host.clone(),
            port: server.port,
            user: server.username.clone(),
            password: Some(password.to_string()),
            database: None,
            ssl_mode: server.ssl_mode.parse().unwrap_or_default(),
            connect_timeout: Duration::from_secs(u64::from(server.timeout).max(1)),
            command_timeout: Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECONDS),
            ..ConnectionSettings::default()
        },
    }
}

/// Senha guardada do servidor no cofre do sistema.
pub fn stored_password(server: &str) -> Result<String, InputError> {
    match SystemKeyring::new().get(server) {
        Ok(Some(password)) => Ok(password),
        _ => Err(InputError::NoPassword(server.to_string())),
    }
}

/// Executa a consulta e devolve o desfecho, emitindo os eventos no sink.
pub async fn run(
    request: &QueryRunRequest,
    events: &dyn mustel_core::events::EventSink,
    token: &mustel_core::signal::CancellationToken,
) -> Result<query_run::QueryRunOutcome, CoreError> {
    query_run::run(request, events, token).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> ServerConfigEntry {
        ServerConfigEntry {
            name: "dev".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            databases: vec!["mydb".to_string()],
            ssl_mode: "Prefer".to_string(),
            timeout: 30,
            ..ServerConfigEntry::default()
        }
    }

    #[test]
    fn the_request_carries_the_resolved_connection_and_no_database() {
        let request = request_for(&server(), "SELECT 1", "segredo").expect("monta");

        assert_eq!(request.servers.len(), 1);
        let run = &request.servers[0];
        assert_eq!(run.name, "dev");
        assert_eq!(run.settings.host, "localhost");
        assert_eq!(run.settings.user, "postgres");
        assert_eq!(
            run.settings.password.as_deref(),
            Some("segredo"),
            "a senha do cofre entra na conexão"
        );
        assert_eq!(
            run.settings.database, None,
            "o núcleo escolhe o banco de cada alvo"
        );
        assert_eq!(request.sql.as_ref(), "SELECT 1");
    }

    #[test]
    fn a_server_without_password_says_what_to_do() {
        let error = stored_password("servidor-que-nao-existe-no-cofre").expect_err("sem senha");

        assert_eq!(
            error,
            InputError::NoPassword("servidor-que-nao-existe-no-cofre".to_string())
        );
        assert!(error.to_string().contains("set-password"));
    }
}
