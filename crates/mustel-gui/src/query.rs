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

use mustel_core::config::model::SelectionQuery;
use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::query::plan::ServerRun;
use mustel_core::usecases::custom_databases::{self, LoadedSelection, ServerSelection};
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
    /// Falha do núcleo (conexão, leitura do catálogo) com a mensagem pronta.
    Core(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPassword(server) => write!(
                formatter,
                "No password stored for '{server}'. Use `mustel settings db-servers set-password`."
            ),
            Self::Config(message) | Self::Core(message) => write!(formatter, "{message}"),
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
        servers: vec![server_run(server, password)?],
        sql: Arc::from(sql),
        output_directory: absolute(&defaults.output_directory),
        max_parallelism: defaults.max_parallelism as usize,
        command_timeout: Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECONDS),
        all: server.fetch_all_databases,
        exclude: HashSet::new(),
    })
}

/// Monta a requisição a partir dos servidores **já escolhidos** na tela (T42).
///
/// É o caminho da janela: cada servidor vem com a conexão resolvida ([`server_run`]), e o paralelismo
/// e o timeout saem do cadastro, como no CLI. O diretório de saída é **absolutizado** aqui: a tela
/// mostra os caminhos que o núcleo devolve, e um caminho relativo (`./results/...`) não diz a ninguém
/// onde o arquivo foi parar — o que já confundiu uma sessão de teste.
pub fn request_for_runs(
    runs: Vec<ServerRun>,
    sql: &str,
    output_directory: String,
) -> Result<QueryRunRequest, InputError> {
    let store =
        ConfigStore::with_default_path().map_err(|error| InputError::Config(error.to_string()))?;
    let defaults =
        query_run::defaults(&store).map_err(|error| InputError::Config(error.to_string()))?;

    Ok(QueryRunRequest {
        servers: runs,
        sql: Arc::from(sql),
        output_directory: absolute(&output_directory),
        max_parallelism: defaults.max_parallelism.max(1) as usize,
        command_timeout: Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECONDS),
        // Com vários servidores escolhidos, cada um decide se descobre (`fetchAllDatabases`), como no
        // CLI sem `--all`.
        all: false,
        exclude: HashSet::new(),
    })
}

/// Caminho absoluto, como o `Path.GetFullPath` do legado (e o `-o` do CLI).
pub fn absolute(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// A conexão resolvida de um servidor do cadastro, sem banco (o núcleo troca por alvo).
pub fn server_run(server: &ServerConfigEntry, password: &str) -> Result<ServerRun, InputError> {
    Ok(ServerRun {
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
    })
}

/// A conexão de um servidor do cadastro rodando **exatamente** as bases marcadas (D15).
///
/// Não descobre (`fetch_all_databases: false`, e o pedido manda `all: false`): a seleção da tela é
/// a lista final. Os `excludePatterns` vão junto porque o plano os aplica **sempre** — uma base
/// barrada que escapasse da tela ainda assim não chegara ao disco.
pub fn server_run_marked(
    server: &ServerConfigEntry,
    password: &str,
    databases: &[String],
) -> Result<ServerRun, InputError> {
    Ok(ServerRun {
        databases: databases.to_vec(),
        fetch_all_databases: false,
        ..server_run(server, password)?
    })
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

/// Bancos que o servidor tem, para a linha "Referência do autocomplete".
///
/// Usa o mesmo caso de uso do `settings db-servers test` (`pg_database`, ordenado por nome), então a
/// lista é a que o servidor realmente tem — inclusive quando o cadastro usa `fetchAllDatabases` e não
/// guarda nome nenhum.
pub async fn databases(
    server: &ServerConfigEntry,
    password: &str,
) -> Result<Vec<String>, InputError> {
    mustel_core::usecases::db_servers::test_connection(server, password)
        .await
        .map(|report| report.databases)
        .map_err(|error| InputError::Core(error.to_string()))
}

/// O que uma query de seleção produziu, em números — nunca o conteúdo da connection string (D12).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionOutcome {
    /// Servidores que receberam base.
    pub servers: usize,
    /// Bases marcáveis (não barradas por padrão).
    pub databases: usize,
    /// Bases barradas por `excludePatterns` do cadastro (D10).
    pub blocked: usize,
    /// Bases com aviso de credencial (D8).
    pub warnings: usize,
    /// Linhas descartadas por falta de host/banco.
    pub ignored_rows: usize,
}

impl SelectionOutcome {
    /// Resume no texto do status.
    ///
    /// Só contagens: a senha e a connection string nunca entram aqui (D12).
    pub fn summary(&self) -> String {
        let mut text = format!(
            "{} server(s), {} database(s) selected",
            self.servers, self.databases
        );
        if self.blocked > 0 {
            text.push_str(&format!(", {} blocked by excludePatterns", self.blocked));
        }
        if self.warnings > 0 {
            text.push_str(&format!(", {} without a usable password", self.warnings));
        }
        if self.ignored_rows > 0 {
            text.push_str(&format!(", {} row(s) ignored", self.ignored_rows));
        }
        text.push('.');
        text
    }
}

/// Conta o que veio, sem expor nada do conteúdo das linhas.
pub fn summarise(
    loaded: &mustel_core::usecases::custom_databases::LoadedSelection,
    selection: &[mustel_core::usecases::custom_databases::ServerSelection],
) -> SelectionOutcome {
    let mut outcome = SelectionOutcome {
        servers: selection.len(),
        ignored_rows: loaded.ignored_rows,
        ..SelectionOutcome::default()
    };
    for server in selection {
        for database in &server.databases {
            if database.blocked_by.is_some() {
                outcome.blocked += 1;
            } else {
                outcome.databases += 1;
            }
            if database.warning.is_some() {
                outcome.warnings += 1;
            }
        }
    }
    outcome
}

/// Roda a query de seleção numa `SelectionQuery` salva e valida contra o cadastro.
///
/// **Caminho único:** é o que o `Test` da aba Settings e o `⟳ Apply` da aba Query chamam. Os dois
/// precisam ver a mesma coisa — inclusive o erro agregado de host fora do cadastro (D9).
pub async fn selection(
    query: &SelectionQuery,
    servers: &[ServerConfigEntry],
) -> Result<(LoadedSelection, Vec<ServerSelection>), InputError> {
    let entry = servers
        .iter()
        .find(|server| server.name.eq_ignore_ascii_case(&query.server))
        .ok_or_else(|| {
            InputError::Config(format!(
                "server '{}' is not registered; the selection query needs a server from `mustel settings db-servers`",
                query.server
            ))
        })?;

    let password = stored_password(&entry.name)?;
    let mut settings = server_run(entry, &password)?.settings;
    settings.database = Some(query.database.clone());

    let client = mustel_core::postgres::client::connect(&settings)
        .await
        .map_err(|error| InputError::Core(error.to_string()))?;
    let loaded = custom_databases::load(&client, &query.sql)
        .await
        .map_err(|error| InputError::Core(error.to_string()))?;
    let selection = custom_databases::validate(&loaded.rows, servers)
        .map_err(|error| InputError::Core(error.to_string()))?;

    Ok((loaded, selection))
}

/// Carrega o esquema de um banco do servidor, com a senha já resolvida.
///
/// A conexão é a mesma que o `query run` usaria, só que apontada para o banco de **referência** do
/// autocomplete. Nenhuma regra nova: resolve o cadastro e chama o caso de uso do núcleo.
pub async fn schema(
    server: &ServerConfigEntry,
    password: &str,
    database: &str,
) -> Result<mustel_core::usecases::schema::Schema, InputError> {
    let run = server_run(server, password)?;
    let mut settings = run.settings;
    settings.database = Some(database.to_string());

    let client = mustel_core::postgres::client::connect(&settings)
        .await
        .map_err(|error| InputError::Core(error.to_string()))?;
    mustel_core::usecases::schema::load(&client)
        .await
        .map_err(|error| InputError::Core(error.to_string()))
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

    /// A base marcada na tela chega ao plano **explícita**, e a barrada por `excludePatterns` não
    /// chega de jeito nenhum (D15 + R9).
    ///
    /// O teste roda o `plan` de verdade, sem banco: com `fetch_all_databases: false` e `all: false`
    /// ele usa a lista explícita e aplica os padrões do cadastro.
    /// CONTRACT: IF-228
    #[tokio::test]
    async fn marked_databases_reach_the_plan_and_blocked_ones_do_not() {
        let registry = vec![ServerConfigEntry {
            name: "Local".to_string(),
            host: "localhost".to_string(),
            username: "postgres".to_string(),
            exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
            fetch_all_databases: true, // o cadastro descobre; a seleção manda bases explícitas
            ..ServerConfigEntry::default()
        }];

        let mut state = crate::selection::SelectionState::default();
        state.sync_registry(&registry);
        state.discover(
            &registry[0],
            &[
                "dev".to_string(),
                "prod_a".to_string(),
                "postgres".to_string(),
                "template_zero".to_string(),
            ],
        );
        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "prod_a", true);
        // A tela não deixa marcar; se alguém forçar, o estado ignora (R9).
        state.set_marked("Local", "postgres", true);

        let targets = state.targets(&registry);
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].1,
            vec!["dev".to_string(), "prod_a".to_string()],
            "só as marcáveis vão para o pedido"
        );

        let run = server_run_marked(&registry[0], "segredo", &targets[0].1).expect("monta");
        assert!(!run.fetch_all_databases, "a seleção é explícita (D15)");

        // Uma base barrada que escapasse da tela ainda assim é filtrada pelo plano.
        let leaked = server_run_marked(
            &registry[0],
            "segredo",
            &["dev".to_string(), "postgres".to_string()],
        )
        .expect("monta");

        let sink = mustel_core::events::NoopSink;
        let plans = mustel_core::query::plan::plan(&[leaked], false, &HashSet::new(), &sink).await;
        let databases: Vec<&str> = plans[0]
            .databases
            .iter()
            .map(|target| target.database.as_str())
            .collect();
        assert_eq!(
            databases,
            vec!["dev"],
            "`postgres` casa com o padrão e não roda"
        );

        let plans = mustel_core::query::plan::plan(&[run], false, &HashSet::new(), &sink).await;
        let databases: Vec<&str> = plans[0]
            .databases
            .iter()
            .map(|target| target.database.as_str())
            .collect();
        assert_eq!(
            databases,
            vec!["dev", "prod_a"],
            "o resto chega intacto, na ordem"
        );
    }
}
