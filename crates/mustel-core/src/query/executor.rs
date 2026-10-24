//! Execução paralela do `query run`: produtores em paralelo, um escritor único.
//!
//! Porte do `QueryRunCommand.ExecuteOnSelectedServers` + `CsvExporter` do FurLab. O que veio do
//! legado, e que o diff de arquivos mostra:
//!
//! - **dois níveis de paralelismo**: `defaults.maxParallelism` servidores ao mesmo tempo (o
//!   `Parallel.ForEachAsync` externo) e, dentro de cada servidor, `server.maxParallelism` bancos (o
//!   `Parallel.ForEachAsync` interno);
//! - **um canal com um escritor único**, com contrapressão: quem consulta manda blocos de linhas e
//!   espera quando o canal enche, como o `Channel<CsvRow>` do legado;
//! - arquivo **por servidor**, criado com BOM + o cabeçalho da primeira execução que terminou naquele
//!   servidor e alimentado por `append`. O cabeçalho **não** manda nas linhas seguintes: cada linha sai
//!   com as colunas do **próprio** resultado (medido no oráculo: um segundo banco com outras colunas
//!   escreve campos a mais do que o cabeçalho, uma linha torta), e quem relê o arquivo no fim — o
//!   merge do consolidado — casa campo a campo com o cabeçalho, o que enfia o valor excedente debaixo
//!   do nome da coluna daquela posição. É o "header inconsistente é ACEITÁVEL" do legado, e está
//!   reproduzido aqui de propósito;
//! - `_erros.csv` e `_log.csv` ganham uma linha por execução terminada — sucesso ou falha —, com o
//!   `DurationMs` medido por banco (a tabela ao vivo é T35);
//! - o consolidado é escrito **relendo** os arquivos por servidor, no fim (ver
//!   [`csv_writer::merge_consolidated`]);
//! - consulta que falha é tentada de novo até 3 vezes (Polly), só para erro de Postgres ou timeout.
//!
//! O que muda de propósito (registrado em `fixtures/known-divergences.md`): as linhas vão para o arquivo
//! **conforme chegam**, [`CHUNK_ROWS`] por vez, então a memória não cresce com o tamanho do resultado
//! — o legado materializa o resultado de cada banco antes de escrever. Duas consequências: uma falha
//! **no meio** de um resultado deixa no arquivo as linhas já gravadas (o legado descartaria todas) e
//! esse caso **não** é retentado, para não duplicar dados; e dois bancos do mesmo servidor podem
//! intercalar blocos no arquivo dele (o legado escreve um resultado inteiro por vez), o que não muda
//! o conjunto de linhas nem as colunas.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{Semaphore, mpsc};

use crate::error::CoreError;
use crate::events::{CoreEvent, EventSink, Status};
use crate::postgres::client::QueryStream;
use crate::postgres::settings::ConnectionSettings;
use crate::query::csv_writer::{self, Appender};
use crate::signal::CancellationToken;

/// Linhas por mensagem do canal.
///
/// O legado dimensiona o canal em `defaults.maxParallelism * 2` **resultados**; aqui uma mensagem
/// carrega um bloco de linhas, então o mesmo número de mensagens em voo continua sendo um punhado de
/// KBs — é isso que mantém a memória estável num export grande sem deixar o escritor esperando.
pub const CHUNK_ROWS: usize = 64;

/// Tentativas extras do legado (Polly: 3 retries com espera exponencial de 500ms·2^n).
const MAX_RETRIES: u32 = 3;

/// Uma consulta a executar: o banco e a conexão já resolvidos.
#[derive(Debug, Clone)]
pub struct DatabaseTarget {
    /// Banco alvo.
    pub database: String,
    /// Conexão resolvida (sem senha — ela entra na hora de conectar).
    pub settings: ConnectionSettings,
}

/// Um servidor selecionado e os bancos dele, na ordem em que entram na execução.
#[derive(Debug, Clone)]
pub struct ServerPlan {
    /// Nome do servidor: vai na coluna `Server` e no nome do arquivo.
    pub name: String,
    /// `maxParallelism` do cadastro: quantos bancos **deste** servidor rodam ao mesmo tempo.
    pub max_parallelism: usize,
    /// Bancos alvo, na ordem em que foram listados ou descobertos.
    pub databases: Vec<DatabaseTarget>,
}

/// O que a execução precisa saber; nada aqui depende de prompt ou de terminal.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// Servidores selecionados **na ordem da seleção** (é também a ordem do merge do consolidado).
    pub servers: Vec<ServerPlan>,
    /// Consulta já resolvida (`-c`/`-i`).
    pub sql: Arc<str>,
    /// Diretório da execução (`<saída>/<timestamp>`), já criado.
    pub directory: PathBuf,
    /// Timestamp da execução, que nomeia os arquivos.
    pub timestamp: String,
    /// `maxParallelism` dos defaults: quantos **servidores** rodam ao mesmo tempo.
    pub max_parallelism: usize,
    /// `--command-timeout`, aplicado por leitura de linha.
    pub command_timeout: Duration,
}

/// Números do resumo final e os arquivos gerados.
#[derive(Debug)]
pub struct ExecutionOutcome {
    /// Bancos que responderam com sucesso.
    pub successes: usize,
    /// Bancos que falharam.
    pub failures: usize,
    /// Soma das linhas trazidas.
    pub total_rows: u64,
    /// Consolidado, quando houve **alguma** linha de dados (o legado só o cria nesse caso).
    pub consolidated: Option<PathBuf>,
    /// `_erros.csv`, quando houve falha.
    pub errors: Option<PathBuf>,
    /// `_log.csv` (o legado imprime o caminho mesmo quando o arquivo não existe).
    pub log: PathBuf,
}

/// Executa a consulta nos servidores escolhidos e escreve os CSVs da execução.
///
/// `events` recebe cada banco que termina e cada servidor que fica sem banco, na ordem em que
/// acontecerem — é de onde sai a tabela de progresso. Quem não quer acompanhar passa um coletor
/// vazio (ver os testes).
pub async fn execute(
    request: &ExecutionRequest,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<ExecutionOutcome, CoreError> {
    let (sender, receiver) = mpsc::channel::<Message>(request.max_parallelism.max(1) * 2);

    let writer = write_loop(&request.directory, &request.timestamp, receiver);
    let producers = async move {
        let servers = Semaphore::new(request.max_parallelism.max(1));
        let mut running = FuturesUnordered::new();
        for server in &request.servers {
            running.push(run_server(
                request, server, &servers, &sender, events, token,
            ));
        }

        let mut counts = Counts::default();
        while let Some(partial) = running.next().await {
            counts.add(partial?);
        }

        // Fecha o canal: o escritor termina de gravar quando as últimas mensagens chegarem. O
        // `running` precisa ir embora antes, porque as consultas emprestam o `sender`.
        drop(running);
        drop(sender);

        Ok::<Counts, CoreError>(counts)
    };

    let (written, produced) = tokio::join!(writer, producers);
    written.map_err(CoreError::Io)?;
    let counts = produced?;

    let server_names: Vec<String> = request
        .servers
        .iter()
        .map(|server| server.name.clone())
        .collect();
    let consolidated =
        csv_writer::merge_consolidated(&request.directory, &request.timestamp, &server_names)
            .map_err(CoreError::Io)?;

    let errors = request
        .directory
        .join(csv_writer::errors_file_name(&request.timestamp));
    let errors = errors.is_file().then_some(errors);
    let log = request
        .directory
        .join(csv_writer::log_file_name(&request.timestamp));

    Ok(ExecutionOutcome {
        successes: counts.successes,
        failures: counts.failures,
        total_rows: counts.rows,
        consolidated,
        errors,
        log,
    })
}

/// Números que uma execução (ou um servidor) contribuiu para o resumo final.
#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    successes: usize,
    failures: usize,
    rows: u64,
}

impl Counts {
    fn add(&mut self, other: Self) {
        self.successes += other.successes;
        self.failures += other.failures;
        self.rows += other.rows;
    }
}

/// Roda os bancos de **um** servidor, com o `maxParallelism` do próprio servidor.
async fn run_server(
    request: &ExecutionRequest,
    server: &ServerPlan,
    servers: &Semaphore,
    sender: &mpsc::Sender<Message>,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<Counts, CoreError> {
    if token.is_cancelled() {
        return Ok(Counts::default());
    }

    // O `Parallel.ForEachAsync` de fora segura um lugar por servidor enquanto os bancos dele rodam.
    let Ok(_slot) = servers.acquire().await else {
        return Ok(Counts::default());
    };

    if server.databases.is_empty() {
        // O legado marca o servidor sem banco com uma linha própria e não roda nada nele.
        events.emit(CoreEvent::ServerWithoutDatabases {
            server: &server.name,
        });

        return Ok(Counts::default());
    }

    let databases = Semaphore::new(server.max_parallelism.max(1));
    let mut running = FuturesUnordered::new();
    for target in &server.databases {
        running.push(run_database(
            request, server, target, &databases, sender, events, token,
        ));
    }

    let mut counts = Counts::default();
    while let Some(partial) = running.next().await {
        counts.add(partial?);
    }

    Ok(counts)
}

/// Roda a consulta em **um** banco e manda o resultado para o escritor.
async fn run_database(
    request: &ExecutionRequest,
    server: &ServerPlan,
    target: &DatabaseTarget,
    databases: &Semaphore,
    sender: &mpsc::Sender<Message>,
    events: &dyn EventSink,
    token: &CancellationToken,
) -> Result<Counts, CoreError> {
    if token.is_cancelled() {
        return Ok(Counts::default());
    }

    let Ok(_slot) = databases.acquire().await else {
        return Ok(Counts::default());
    };

    let executed_at = SystemTime::now();
    let started = Instant::now();
    let mut attempt = 0u32;
    // Tentar de novo depois de mandar linhas duplicaria dados no arquivo: o legado descarta o
    // resultado parcial e recomeça (ele só materializa no fim), o que aqui já não é possível.
    let mut sent_rows = false;

    loop {
        match stream_once(request, server, target, sender, token, &mut sent_rows).await {
            Ok(rows) => {
                let duration = started.elapsed();
                send(
                    sender,
                    Message::Finished(LogEntry {
                        server: Arc::from(server.name.as_str()),
                        database: Arc::from(target.database.as_str()),
                        executed_at,
                        status: Status::Success,
                        rows,
                        duration_ms: milliseconds(duration),
                        error: String::new(),
                    }),
                )
                .await?;

                events.emit(CoreEvent::DatabaseFinished {
                    server: &server.name,
                    database: &target.database,
                    status: Status::Success,
                    rows,
                    duration,
                    error: "",
                });

                return Ok(Counts {
                    successes: 1,
                    failures: 0,
                    rows,
                });
            }
            Err(error) => {
                if token.is_cancelled() {
                    // O legado não registra nada do que foi cancelado no meio.
                    return Ok(Counts::default());
                }

                // Canal fechado quer dizer que o escritor morreu (disco, permissão...): isso não é
                // falha de banco e não pode virar uma linha de erro no CSV.
                if sender.is_closed() {
                    return Err(error);
                }

                let retryable = matches!(error, CoreError::Postgres { .. } | CoreError::Timeout(_));
                if retryable && !sent_rows && attempt < MAX_RETRIES {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(2u64.pow(attempt) * 500)).await;
                    continue;
                }

                // No erro o legado marca `DurationMs` como 0 e tira o `ExecutedAt` na hora da falha.
                let message = error.to_string();
                send(
                    sender,
                    Message::Finished(LogEntry {
                        server: Arc::from(server.name.as_str()),
                        database: Arc::from(target.database.as_str()),
                        executed_at: SystemTime::now(),
                        status: Status::Error,
                        rows: 0,
                        duration_ms: "0".to_string(),
                        error: message.clone(),
                    }),
                )
                .await?;

                events.emit(CoreEvent::DatabaseFinished {
                    server: &server.name,
                    database: &target.database,
                    status: Status::Error,
                    rows: 0,
                    duration: Duration::ZERO,
                    error: &message,
                });

                return Ok(Counts {
                    successes: 0,
                    failures: 1,
                    rows: 0,
                });
            }
        }
    }
}

/// Uma tentativa: abre a consulta e manda os blocos de linhas para o escritor.
///
/// Devolve quantas linhas passaram e marca `sent_rows` quando algum bloco **chegou a ir** para o
/// arquivo — é o que impede a retentativa de duplicar dados.
async fn stream_once(
    request: &ExecutionRequest,
    server: &ServerPlan,
    target: &DatabaseTarget,
    sender: &mpsc::Sender<Message>,
    token: &CancellationToken,
    sent_rows: &mut bool,
) -> Result<u64, CoreError> {
    let mut stream =
        QueryStream::open(&target.settings, &request.sql, request.command_timeout).await?;

    let server_name: Arc<str> = Arc::from(server.name.as_str());
    let database: Arc<str> = Arc::from(target.database.as_str());
    let columns: Arc<[String]> = Arc::from(stream.columns());

    // O arquivo do servidor nasce aqui, com o cabeçalho deste resultado: é o `AppendToServerCsv` do
    // legado, que cria o arquivo na primeira escrita — mesmo quando o resultado não tem linha.
    send(
        sender,
        Message::Started {
            server: Arc::clone(&server_name),
            columns: Arc::clone(&columns),
        },
    )
    .await?;

    let mut rows = 0u64;
    let mut chunk: Vec<Vec<String>> = Vec::with_capacity(CHUNK_ROWS);

    loop {
        if token.is_cancelled() {
            return Err(CoreError::Cancelled);
        }

        let Some(values) = stream.next_row().await? else {
            break;
        };

        chunk.push(values);
        rows += 1;

        if chunk.len() == CHUNK_ROWS {
            *sent_rows = true;
            send(
                sender,
                Message::Rows {
                    server: Arc::clone(&server_name),
                    database: Arc::clone(&database),
                    values: std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK_ROWS)),
                },
            )
            .await?;
        }
    }

    if !chunk.is_empty() {
        *sent_rows = true;
        send(
            sender,
            Message::Rows {
                server: server_name,
                database,
                values: chunk,
            },
        )
        .await?;
    }

    Ok(rows)
}

/// `DurationMs` do log, no formato do `double` do legado (`sw.Elapsed.TotalMilliseconds`).
fn milliseconds(duration: Duration) -> String {
    format!("{}", duration.as_secs_f64() * 1000.0)
}

/// Manda uma mensagem para o escritor; erro aqui significa que o escritor parou (canal fechado).
async fn send(sender: &mpsc::Sender<Message>, message: Message) -> Result<(), CoreError> {
    sender.send(message).await.map_err(|_| {
        CoreError::Other("the CSV writer stopped; the execution was interrupted".to_string())
    })
}

/// O que o produtor manda para o escritor único — o `CsvRow` do legado, em pedaços.
enum Message {
    /// Resultado começou: é o que cria o arquivo do servidor com o cabeçalho dele.
    Started {
        server: Arc<str>,
        columns: Arc<[String]>,
    },
    /// Bloco de linhas já formatadas, na ordem das colunas do resultado que as produziu.
    Rows {
        server: Arc<str>,
        database: Arc<str>,
        values: Vec<Vec<String>>,
    },
    /// Execução terminou (sucesso ou erro): vira linha no `_log.csv` e, no erro, no `_erros.csv`.
    Finished(LogEntry),
}

/// Linha do `_log.csv` (e do `_erros.csv`, quando é erro).
struct LogEntry {
    server: Arc<str>,
    database: Arc<str>,
    executed_at: SystemTime,
    status: Status,
    rows: u64,
    /// Já formatado, como o `double` do legado (`0` no caso de erro).
    duration_ms: String,
    error: String,
}

/// O **escritor único**: consome o canal e grava os arquivos na ordem em que as mensagens chegam.
async fn write_loop(
    directory: &Path,
    timestamp: &str,
    mut receiver: mpsc::Receiver<Message>,
) -> std::io::Result<()> {
    let mut outputs = OutputFiles::new(directory, timestamp);

    while let Some(message) = receiver.recv().await {
        match message {
            Message::Started { server, columns } => outputs.start(&server, &columns)?,
            Message::Rows {
                server,
                database,
                values,
            } => outputs.rows(&server, &database, values)?,
            Message::Finished(entry) => outputs.finish(&entry)?,
        }
    }

    Ok(())
}

/// Os arquivos da execução, abertos sob demanda como no legado.
struct OutputFiles {
    directory: PathBuf,
    timestamp: String,
    /// Um arquivo por servidor, na ordem em que o primeiro resultado de cada um chegou.
    servers: HashMap<String, Appender>,
    errors: Option<Appender>,
    log: Option<Appender>,
}

impl OutputFiles {
    fn new(directory: &Path, timestamp: &str) -> Self {
        Self {
            directory: directory.to_path_buf(),
            timestamp: timestamp.to_string(),
            servers: HashMap::new(),
            errors: None,
            log: None,
        }
    }

    /// Primeiro resultado de um servidor: cria o arquivo dele (BOM + cabeçalho) e nada mais.
    ///
    /// Chamadas seguintes são ignoradas: o cabeçalho do arquivo é o do **primeiro** resultado que
    /// chegou, como o `if (!fileExists)` do legado.
    fn start(&mut self, server: &Arc<str>, columns: &Arc<[String]>) -> std::io::Result<()> {
        if self.servers.contains_key(server.as_ref()) {
            return Ok(());
        }

        let appender = Appender::new(
            self.directory
                .join(csv_writer::server_file_name(server, &self.timestamp)),
            csv_writer::header_for(columns),
        );
        appender.append(&[])?;

        self.servers.insert(server.to_string(), appender);

        Ok(())
    }

    /// Bloco de linhas de um resultado: `Server`, `Database` e os valores **na ordem do resultado**.
    ///
    /// Escrever aqui as colunas do resultado (e não as do cabeçalho do arquivo) é o que o oráculo faz:
    /// se um segundo banco traz colunas diferentes, o arquivo ganha uma linha com o número de campos
    /// dele, e o cabeçalho fica inconsistente — o próprio legado declara isso aceitável.
    fn rows(
        &mut self,
        server: &Arc<str>,
        database: &Arc<str>,
        values: Vec<Vec<String>>,
    ) -> std::io::Result<()> {
        let Some(appender) = self.servers.get_mut(server.as_ref()) else {
            return Err(std::io::Error::other(format!(
                "resultado de '{server}' chegou antes do cabeçalho do arquivo"
            )));
        };

        let rows: Vec<Vec<String>> = values
            .into_iter()
            .map(|values| {
                let mut fields = Vec::with_capacity(values.len() + 2);
                fields.push(server.to_string());
                fields.push(database.to_string());
                fields.extend(values);
                fields
            })
            .collect();

        appender.append(&rows)
    }

    /// Uma execução terminada: linha no `_log.csv` e, no erro, também no `_erros.csv`.
    fn finish(&mut self, entry: &LogEntry) -> std::io::Result<()> {
        let executed_at = csv_writer::iso8601_for(entry.executed_at);

        if entry.status == Status::Error {
            if self.errors.is_none() {
                self.errors = Some(Appender::new(
                    self.directory
                        .join(csv_writer::errors_file_name(&self.timestamp)),
                    ["Server", "Database", "ExecutedAt", "Error"]
                        .iter()
                        .map(|field| (*field).to_string())
                        .collect(),
                ));
            }

            self.errors
                .as_ref()
                .expect("acabou de ser criado")
                .append(&[vec![
                    entry.server.to_string(),
                    entry.database.to_string(),
                    executed_at.clone(),
                    entry.error.clone(),
                ]])?;
        }

        if self.log.is_none() {
            self.log = Some(Appender::new(
                self.directory
                    .join(csv_writer::log_file_name(&self.timestamp)),
                [
                    "Server",
                    "Database",
                    "ExecutedAt",
                    "Status",
                    "RowCount",
                    "DurationMs",
                    "Error",
                ]
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
            ));
        }

        self.log
            .as_ref()
            .expect("acabou de ser criado")
            .append(&[vec![
                entry.server.to_string(),
                entry.database.to_string(),
                executed_at,
                entry.status.text().to_string(),
                entry.rows.to_string(),
                entry.duration_ms.clone(),
                entry.error.clone(),
            ]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CoreEvent;

    /// Escuta que guarda os eventos em texto, para os testes conferirem a sequência.
    struct Collector<'a>(&'a std::sync::Mutex<Vec<String>>);

    impl EventSink for Collector<'_> {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.lock().expect("collector").push(format!("{event:?}"));
        }
    }

    fn columns(names: &[&str]) -> Arc<[String]> {
        Arc::from(
            names
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<String>>(),
        )
    }

    fn log_entry(server: &str, database: &str, status: Status, rows: u64) -> LogEntry {
        LogEntry {
            server: Arc::from(server),
            database: Arc::from(database),
            executed_at: std::time::UNIX_EPOCH + Duration::from_micros(1_792_333_948_228_411),
            status,
            rows,
            duration_ms: match status {
                Status::Success => "12.5".to_string(),
                Status::Error => "0".to_string(),
            },
            error: match status {
                Status::Success => String::new(),
                Status::Error => "connection refused".to_string(),
            },
        }
    }

    /// Passa as mensagens pelo canal para o escritor e devolve o que ele gravou.
    async fn write(messages: Vec<Message>) -> (tempfile::TempDir, String, std::io::Result<()>) {
        let directory = tempfile::tempdir().expect("tempdir");
        let timestamp = "2026-10-18_143228".to_string();
        let (sender, receiver) = mpsc::channel::<Message>(4);
        let written = {
            let path = directory.path().to_path_buf();
            let stamp = timestamp.clone();
            tokio::spawn(async move { write_loop(&path, &stamp, receiver).await })
        };
        for message in messages {
            let _ = sender.send(message).await;
        }
        drop(sender);

        let result = written
            .await
            .expect("a task do escritor não pode entrar em pânico");
        (directory, timestamp, result)
    }

    fn read(directory: &tempfile::TempDir, name: &str) -> String {
        std::fs::read_to_string(directory.path().join(name)).expect("lê o arquivo")
    }

    #[tokio::test]
    async fn the_writer_creates_the_server_file_once_and_keeps_each_result_columns() {
        let (directory, timestamp, result) = write(vec![
            Message::Started {
                server: Arc::from("dev"),
                columns: columns(&["id", "nome"]),
            },
            Message::Rows {
                server: Arc::from("dev"),
                database: Arc::from("a"),
                values: vec![vec!["1".into(), "x".into()]],
            },
            // Um segundo banco do mesmo servidor, com outras colunas **em número diferente**: o
            // cabeçalho não é reescrito e a linha sai com os campos do próprio resultado, torta em
            // relação ao cabeçalho — é o que o oráculo escreve (medido no cenário do T34).
            Message::Started {
                server: Arc::from("dev"),
                columns: columns(&["nome", "outro", "terceiro"]),
            },
            Message::Rows {
                server: Arc::from("dev"),
                database: Arc::from("b"),
                values: vec![vec!["y".into(), "z".into(), "w".into()]],
            },
            Message::Finished(log_entry("dev", "a", Status::Success, 1)),
        ])
        .await;
        result.expect("grava");

        assert_eq!(
            read(&directory, &csv_writer::server_file_name("dev", &timestamp)),
            "\u{feff}Server,Database,id,nome\r\ndev,a,1,x\r\ndev,b,y,z,w\r\n"
        );
        assert!(
            !directory
                .path()
                .join(csv_writer::errors_file_name(&timestamp))
                .exists(),
            "sem erro, o `_erros.csv` não existe"
        );
        assert_eq!(
            read(&directory, &csv_writer::log_file_name(&timestamp)),
            "\u{feff}Server,Database,ExecutedAt,Status,RowCount,DurationMs,Error\r\n\
             dev,a,2026-10-18T14:32:28.2284110Z,Success,1,12.5,\r\n"
        );
    }

    #[tokio::test]
    /// CONTRACT: IF-037
    async fn the_writer_logs_the_error_and_writes_it_to_the_errors_file() {
        let (directory, timestamp, result) = write(vec![Message::Finished(log_entry(
            "dev",
            "a",
            Status::Error,
            0,
        ))])
        .await;
        result.expect("grava");

        assert_eq!(
            read(&directory, &csv_writer::errors_file_name(&timestamp)),
            "\u{feff}Server,Database,ExecutedAt,Error\r\n\
             dev,a,2026-10-18T14:32:28.2284110Z,connection refused\r\n"
        );
        assert!(
            read(&directory, &csv_writer::log_file_name(&timestamp))
                .ends_with("dev,a,2026-10-18T14:32:28.2284110Z,Error,0,0,connection refused\r\n"),
            "o erro também entra no log, com duração 0"
        );
    }

    #[tokio::test]
    async fn an_empty_result_creates_the_server_file_with_just_the_header() {
        let (directory, timestamp, result) = write(vec![
            Message::Started {
                server: Arc::from("dev"),
                columns: columns(&["id"]),
            },
            Message::Finished(log_entry("dev", "a", Status::Success, 0)),
        ])
        .await;
        result.expect("grava");

        assert_eq!(
            read(&directory, &csv_writer::server_file_name("dev", &timestamp)),
            "\u{feff}Server,Database,id\r\n"
        );
    }

    #[tokio::test]
    async fn a_run_without_databases_writes_no_consolidated_and_no_errors() {
        let directory = tempfile::tempdir().expect("tempdir");
        let request = ExecutionRequest {
            servers: vec![ServerPlan {
                name: "dev".to_string(),
                max_parallelism: 4,
                databases: Vec::new(),
            }],
            sql: Arc::from("SELECT 1"),
            directory: directory.path().to_path_buf(),
            timestamp: "ts".to_string(),
            max_parallelism: 4,
            command_timeout: Duration::from_secs(300),
        };

        let events = std::sync::Mutex::new(Vec::new());
        let outcome = execute(&request, &Collector(&events), &CancellationToken::new())
            .await
            .expect("executa");

        assert_eq!(outcome.successes, 0);
        assert_eq!(outcome.failures, 0);
        assert_eq!(outcome.total_rows, 0);
        assert!(outcome.consolidated.is_none(), "nada para consolidar");
        assert!(outcome.errors.is_none(), "nenhum erro");
        assert!(
            !outcome.log.exists(),
            "sem execução nenhuma, o log não é criado (o caminho ainda é impresso)"
        );
        assert_eq!(
            events.lock().expect("collector").len(),
            1,
            "o servidor sem banco vira uma linha da tabela: {:?}",
            events.lock().expect("collector")
        );
        assert!(
            events.lock().expect("collector")[0].contains("ServerWithoutDatabases"),
            "{:?}",
            events.lock().expect("collector")
        );
    }

    #[test]
    /// CONTRACT: IF-036
    fn the_duration_of_the_log_reads_like_the_dotnet_double() {
        assert_eq!(milliseconds(Duration::from_millis(12)), "12");
        assert_eq!(milliseconds(Duration::from_micros(12_500)), "12.5");
    }
}
