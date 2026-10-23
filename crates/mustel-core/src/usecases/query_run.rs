//! `mustel query run` como caso de uso do núcleo (T40).
//!
//! Recebe a entrada **resolvida** — servidores escolhidos com a conexão pronta, o texto da consulta,
//! o diretório de saída e os números do paralelismo — e faz o que é regra:
//!
//! 1. cria `<saída>/<timestamp>/`, com o timestamp da execução;
//! 2. monta o plano ([`plan`]): descobre bancos onde o cadastro pede, filtra pelos dois critérios e
//!    troca o banco de cada conexão;
//! 3. para cedo, com exit **0**, quando não sobrou banco nenhum — sem criar canal, log nem
//!    consolidado, e sem o resumo (é o `No accessible databases...` do legado);
//! 4. executa ([`executor`]) e, no fim, emite os caminhos dos arquivos e os números do resumo;
//! 5. devolve erro quando **nenhum** servidor respondeu e houve falha.
//!
//! O que o legado decide depois da execução e fica no núcleo por isso: a regra do 5 é sobre o
//! resultado, não sobre a tela. Quem formata é o frontend, ouvindo os eventos.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::config::{ConfigStore, UserDefaults};
use crate::error::CoreError;
use crate::events::{CoreEvent, EventSink};
use crate::query::csv_writer;
use crate::query::executor::{self, ExecutionOutcome, ExecutionRequest};
use crate::query::plan::{self, ServerRun};
use crate::signal::CancellationToken;

/// O que o comando precisa saber; nada aqui depende de prompt ou de terminal.
pub struct QueryRunRequest {
    /// Servidores escolhidos, **na ordem da seleção** (é também a ordem do merge do consolidado).
    pub servers: Vec<ServerRun>,
    /// Consulta já resolvida (`-c`/`-i`) e desescapada.
    pub sql: Arc<str>,
    /// Diretório base da saída: o timestamp da execução é criado dentro dele.
    pub output_directory: PathBuf,
    /// `maxParallelism` dos defaults: quantos **servidores** rodam ao mesmo tempo.
    pub max_parallelism: usize,
    /// `--command-timeout`, aplicado por leitura de linha.
    pub command_timeout: Duration,
    /// `--all`: descobre os bancos em **todos** os servidores escolhidos.
    pub all: bool,
    /// Nomes exatos do `--exclude`, já normalizados (sem diferenciar maiúsculas).
    pub exclude: HashSet<String>,
}

/// Como a execução terminou.
#[derive(Debug)]
pub enum QueryRunOutcome {
    /// Nenhum banco acessível: o comando termina com exit **0** e nada foi criado.
    NoAccessibleDatabases,
    /// A consulta rodou (com sucesso, com falha, ou com as duas coisas).
    Executed(ExecutionOutcome),
}

/// Preferências do arquivo (`defaults`): diretório de saída, paralelismo e confirmação.
///
/// Fica no núcleo porque os dois frontends precisam do **mesmo** default: o CLI para montar a entrada
/// do comando, a GUI para preencher a tela.
pub fn defaults(store: &ConfigStore) -> Result<UserDefaults, CoreError> {
    Ok(store.load()?.defaults.unwrap_or_default())
}

/// Executa a consulta nos servidores escolhidos.
///
/// `events` recebe a fase de descoberta, cada banco que termina, os avisos de fallback e, no fim, os
/// caminhos e os números.
pub async fn run(
    request: &QueryRunRequest,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<QueryRunOutcome, CoreError> {
    let timestamp = csv_writer::timestamp_for(SystemTime::now());
    let directory = request.output_directory.join(&timestamp);
    std::fs::create_dir_all(&directory).map_err(CoreError::Io)?;

    let plans = plan::plan(&request.servers, request.all, &request.exclude, events).await;

    if plans.iter().all(|plan| plan.databases.is_empty()) {
        // Aqui o legado sai antes de criar o canal, o log e o consolidado — e sem o resumo.
        events.emit(CoreEvent::NoAccessibleDatabases);

        return Ok(QueryRunOutcome::NoAccessibleDatabases);
    }

    let servers = plans.len();
    let execution = ExecutionRequest {
        servers: plans,
        sql: Arc::clone(&request.sql),
        directory,
        timestamp,
        max_parallelism: request.max_parallelism.max(1),
        command_timeout: request.command_timeout,
    };

    let outcome = executor::execute(&execution, events, token).await?;

    events.emit(CoreEvent::FilesWritten {
        consolidated: outcome.consolidated.as_deref(),
        errors: outcome.errors.as_deref(),
        log: &outcome.log,
    });
    events.emit(CoreEvent::Summary {
        servers,
        successes: outcome.successes,
        failures: outcome.failures,
        total_rows: outcome.total_rows,
    });

    if outcome.successes == 0 && outcome.failures > 0 {
        return Err(CoreError::Other(
            "No server responded successfully. Please check the connections.".to_string(),
        ));
    }

    Ok(QueryRunOutcome::Executed(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NoopSink;
    use crate::postgres::settings::ConnectionSettings;
    use crate::postgres::tls::SslMode;
    use std::sync::Mutex;

    /// Escuta que guarda os eventos em texto, para os testes conferirem a sequência.
    struct Collector<'a>(&'a Mutex<Vec<String>>);

    impl EventSink for Collector<'_> {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.lock().expect("collector").push(format!("{event:?}"));
        }
    }

    fn request(servers: Vec<ServerRun>, output: &std::path::Path) -> QueryRunRequest {
        QueryRunRequest {
            servers,
            sql: Arc::from("SELECT 1"),
            output_directory: output.to_path_buf(),
            max_parallelism: 4,
            command_timeout: Duration::from_secs(300),
            all: false,
            exclude: HashSet::new(),
        }
    }

    fn server(name: &str, databases: &[&str]) -> ServerRun {
        ServerRun {
            name: name.to_string(),
            max_parallelism: 4,
            databases: databases.iter().map(|value| (*value).to_string()).collect(),
            exclude_patterns: Vec::new(),
            fetch_all_databases: false,
            settings: ConnectionSettings {
                host: "127.0.0.1".to_string(),
                port: 1,
                user: "postgres".to_string(),
                password: Some("segredo".to_string()),
                // `Require` falha na hora, sem TLS ligada: é o único erro de conexão que **não** é
                // retentado, então os testes não pagam as três esperas do Polly.
                ssl_mode: SslMode::Require,
                connect_timeout: Duration::from_millis(1),
                ..ConnectionSettings::default()
            },
        }
    }

    #[tokio::test]
    /// CONTRACT: IF-040
    async fn without_any_accessible_database_the_command_stops_before_writing_files() {
        let output = tempfile::tempdir().expect("tempdir");
        // Servidor sem banco e sem descoberta: o plano sai vazio.
        let request = request(vec![server("vazio", &[])], output.path());
        let events = Mutex::new(Vec::new());

        let outcome = run(&request, &Collector(&events), &CancellationToken::new())
            .await
            .expect("não é erro: o legado sai com 0");

        assert!(matches!(outcome, QueryRunOutcome::NoAccessibleDatabases));

        let seen = events.lock().expect("collector");
        assert_eq!(seen.len(), 1, "só o aviso de que não há banco: {seen:?}");
        assert!(seen[0].contains("NoAccessibleDatabases"), "{seen:?}");

        let directory = std::fs::read_dir(output.path())
            .expect("lê")
            .collect::<Result<Vec<_>, _>>()
            .expect("lê");
        assert_eq!(
            directory.len(),
            1,
            "o diretório do timestamp é criado antes, como no legado"
        );
        assert!(
            directory[0]
                .path()
                .read_dir()
                .expect("lê o diretório da execução")
                .next()
                .is_none(),
            "e fica vazio: sem log, sem consolidado e sem CSV"
        );
    }

    #[tokio::test]
    async fn a_run_that_cannot_reach_any_server_fails_with_the_legacy_message() {
        let output = tempfile::tempdir().expect("tempdir");
        let request = request(vec![server("fora-do-ar", &["app"])], output.path());

        let error = run(&request, &NoopSink, &CancellationToken::new())
            .await
            .expect_err("nenhum servidor respondeu");

        assert_eq!(
            error.to_string(),
            "No server responded successfully. Please check the connections.",
            "o texto é o do legado; quem imprime `Error: ...` é o CLI"
        );
    }

    #[tokio::test]
    /// CONTRACT: IF-041
    async fn the_summary_and_the_file_paths_are_emitted_after_the_run() {
        let output = tempfile::tempdir().expect("tempdir");
        let request = request(vec![server("fora-do-ar", &["app"])], output.path());
        let events = Mutex::new(Vec::new());

        // O erro vem junto: os números finais saem **antes** dele, como o legado imprime.
        let _ = run(&request, &Collector(&events), &CancellationToken::new()).await;

        let seen = events.lock().expect("collector");
        assert!(
            seen.iter().any(|event| event.contains("Summary")),
            "o resumo chega por evento: {seen:?}"
        );
        assert!(
            seen.iter().any(|event| event.contains("FilesWritten")),
            "e os caminhos também: {seen:?}"
        );
        assert!(
            seen.iter()
                .position(|event| event.contains("Summary"))
                .expect("tem resumo")
                > seen
                    .iter()
                    .position(|event| event.contains("DatabaseFinished"))
                    .expect("tem banco terminado"),
            "o resumo é do fim da execução: {seen:?}"
        );
    }
}
