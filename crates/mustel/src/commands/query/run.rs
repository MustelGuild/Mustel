//! `mustel query run` — executa a consulta nos bancos escolhidos e exporta o CSV.
//!
//! Escopo desta entrega (T32 + T33): descoberta incluída; ficam para as próximas sub-entregas, e
//! está registrado aqui para não parecer esquecimento:
//!
//! - **T34**: execução em paralelo (`maxParallelism`), `--separate-files` e escrita incremental;
//! - **T35**: detecção de consulta destrutiva com confirmação e a tabela de progresso ao vivo.
//!
//! Paridade medida e conferida no código do legado:
//!
//! - as validações de uso saem com exit **1** (não 2, como no `pgpass`): `-c` com `-i` juntos, `-i`
//!   inexistente, consulta vazia, nenhum servidor cadastrado, nada selecionado;
//! - o texto da consulta vem de `-c` (com o "desescape" de aspas externas do legado) ou do arquivo
//!   `-i`, lido como UTF-8;
//! - **descoberta de bancos** (T33): `--all` liga `fetchAllDatabases` em **todos** os servidores
//!   escolhidos; quem descobre consulta `pg_database` no banco `postgres` e filtra pelos
//!   `excludePatterns` do cadastro (curinga `*`); falha de descoberta **avisa e cai para os bancos do
//!   cadastro** em vez de derrubar o comando; e o `--exclude` (nomes exatos, sem diferenciar
//!   maiúsculas) filtra os dois caminhos, no fim;
//! - a saída vai para `<diretório>/<timestamp>/`, com um CSV por servidor, o consolidado, o de
//!   erros (só quando falha) e o de log — nomes e conteúdo iguais aos do oráculo (ver
//!   `mustel_core::query::csv_writer`);
//! - a tabela de resultados do legado é ao vivo; aqui só o resumo final é impresso (a tabela é
//!   T35) — o texto do resumo é o mesmo;
//! - se **nenhum** servidor respondeu e houve falha, sai `Error: No server responded successfully. \
//!   Please check the connections.` com exit 1; se não houve banco nenhum, imprime `No accessible \
//!   databases found across selected servers.` e sai **0**;
//! - o legado tenta de novo (3 vezes, com espera exponencial) só para erro de Postgres ou timeout.

use std::path::PathBuf;

use mustel_core::config::ServerConfigEntry;
use mustel_core::error::CoreError;
use mustel_core::postgres::client::query_rows;
use mustel_core::query::{csv_writer, discovery};
use mustel_core::signal::CancellationToken;
use mustel_core::validation;

use crate::cli::QueryRunArgs;
use crate::ui::prompts;

use super::settings::{self, Target};

/// Tentativas extras do legado (Polly: 3 retries com espera exponencial de 500ms·2^n).
const MAX_RETRIES: u32 = 3;

/// Resultado de uma consulta: as colunas e as linhas **já formatadas** para o CSV.
type QueryResult = (Vec<String>, Vec<Vec<String>>);

/// Um resultado bem-sucedido: alvo, colunas do resultado e linhas.
type Success = (Target, Vec<String>, Vec<Vec<String>>);

/// Executa o comando.
pub async fn run(args: QueryRunArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    let sql = match resolve_sql(&args) {
        Ok(sql) => sql,
        Err(message) => {
            println!("{message}");
            return Ok(1);
        }
    };

    let servers = match settings::servers_from_settings(&args)? {
        Some(servers) => servers,
        None => {
            println!(
                "Error: No servers configured. Run 'settings db-servers add' to add a server first."
            );
            return Ok(1);
        }
    };

    let selected = select_servers(&servers, &args)?;
    if selected.is_empty() {
        println!("No servers selected. Execution cancelled.");
        return Ok(1);
    }

    execute(selected, &sql, &args, token).await
}
/// Texto da consulta: `-c` (com desescape) ou o arquivo de `-i`.
///
/// As mensagens e a ordem das checagens são as do legado; todas saem com exit 1.
fn resolve_sql(args: &QueryRunArgs) -> Result<String, String> {
    let command = args
        .command
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let input = args
        .input
        .as_deref()
        .filter(|value| !value.trim().is_empty());

    if command.is_some() && input.is_some() {
        return Err(
            "Error: Options -c/--command and -i/--input are mutually exclusive. Use only one."
                .to_string(),
        );
    }

    let sql = match (command, input) {
        (Some(command), _) => unescape_inline_query(command),
        (None, Some(path)) => {
            if !validation::is_valid_path(path) {
                return Err(format!(
                    "Error: Invalid input path: '{path}'. Path traversal not allowed."
                ));
            }

            let full_path = absolute_path(path);
            let Ok(contents) = std::fs::read(&full_path) else {
                return Err(format!("Error: SQL input file not found: {full_path}"));
            };

            String::from_utf8_lossy(&contents).into_owned()
        }
        (None, None) => {
            return Err("Error: Use -c for an inline query or -i for a SQL file.".to_string());
        }
    };

    if sql.trim().is_empty() {
        return Err("Error: SQL query is empty.".to_string());
    }

    Ok(sql)
}

/// Tira um par de aspas externas e desescapa aspas escapadas, como o `UnescapeInlineQuery`.
///
/// Existe porque o shell e o parser do legado nem sempre removem as aspas externas: `"SELECT 1"`
/// chega com aspas, e `SELECT \"x\"` chega com barras.
fn unescape_inline_query(query: &str) -> String {
    let trimmed = query.trim();
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    unquoted.replace("\\\"", "\"").replace("\\'", "'")
}

/// Seleção de servidores: um só passa direto; vários abrem a multi-seleção com todos marcados.
fn select_servers(
    servers: &[ServerConfigEntry],
    args: &QueryRunArgs,
) -> Result<Vec<ServerConfigEntry>, CoreError> {
    if servers.len() == 1 {
        return Ok(servers.to_vec());
    }

    let names: Vec<String> = servers.iter().map(|server| server.name.clone()).collect();
    let preselected: Vec<usize> = (0..names.len()).collect();
    let selected =
        prompts::select_many("Select servers to execute query on:", &names, &preselected)?;
    let _ = args;

    Ok(servers
        .iter()
        .filter(|server| selected.contains(&server.name))
        .cloned()
        .collect())
}

/// Roda a consulta nos bancos de cada servidor e escreve os CSVs.
async fn execute(
    servers: Vec<ServerConfigEntry>,
    sql: &str,
    args: &QueryRunArgs,
    token: &CancellationToken,
) -> Result<i32, CoreError> {
    let base_directory = settings::output_directory(args)?;
    let timestamp = csv_writer::timestamp_for(std::time::SystemTime::now());
    let directory = PathBuf::from(&base_directory).join(&timestamp);
    std::fs::create_dir_all(&directory).map_err(CoreError::Io)?;

    // Alvos: para cada servidor, os bancos do cadastro ou os descobertos (`--all` /
    // `fetchAllDatabases`), já filtrados pelos dois critérios do legado.
    let exclude_names = discovery::exclude_names(args.exclude.as_deref());
    let mut targets: Vec<Target> = Vec::new();
    for server in &servers {
        for database in databases_for_server(server, args, &exclude_names).await? {
            targets.push(settings::target_for(server, &database, args)?);
        }
    }

    if targets.is_empty() {
        println!("No accessible databases found across selected servers.");
        return Ok(0);
    }

    let command_timeout =
        std::time::Duration::from_secs(args.command_timeout.unwrap_or(300).max(1) as u64);

    let mut successes: Vec<Success> = Vec::new();
    let mut failures = 0usize;
    let mut error_rows: Vec<Vec<String>> = Vec::new();
    let mut log_rows: Vec<Vec<String>> = Vec::new();

    for target in targets {
        if token.is_cancelled() {
            break;
        }

        let executed_at = std::time::SystemTime::now();
        match execute_with_retry(&target, sql, command_timeout, token).await {
            Ok((columns, data)) => {
                log_rows.push(log_row(&target, executed_at, "Success", data.len(), ""));
                successes.push((target, columns, data));
            }
            Err(error) => {
                failures += 1;
                let message = error.to_string();
                error_rows.push(vec![
                    target.server.clone(),
                    target.database.clone(),
                    csv_writer::iso8601_for(executed_at),
                    message.clone(),
                ]);
                log_rows.push(log_row(&target, executed_at, "Error", 0, &message));
            }
        }
    }

    write_outputs(&directory, &timestamp, &successes, &error_rows, &log_rows)?;

    let total_rows: usize = successes.iter().map(|(_, _, data)| data.len()).sum();
    let consolidated = directory.join(csv_writer::consolidated_file_name(&timestamp));
    let errors_path = directory.join(csv_writer::errors_file_name(&timestamp));
    let log_path = directory.join(csv_writer::log_file_name(&timestamp));

    println!();
    if consolidated.is_file() {
        println!("✅ Consolidated → {}", consolidated.display());
    }
    if errors_path.is_file() && !error_rows.is_empty() {
        println!("❌ Errors       → {}", errors_path.display());
    }
    println!("Log           → {}", log_path.display());
    println!();
    println!(
        "Servers: {} | Success: {} | Failed: {} | Total rows: {}",
        servers.len(),
        successes.len(),
        failures,
        total_rows
    );

    if successes.is_empty() && failures > 0 {
        return Err(CoreError::Other(
            "No server responded successfully. Please check the connections.".to_string(),
        ));
    }

    Ok(0)
}

/// Executa com as tentativas do legado (`NpgsqlException`/`TimeoutException` → 3 retries).
async fn execute_with_retry(
    target: &Target,
    sql: &str,
    command_timeout: std::time::Duration,
    token: &CancellationToken,
) -> Result<QueryResult, CoreError> {
    let mut attempt = 0u32;

    loop {
        if token.is_cancelled() {
            return Err(CoreError::Cancelled);
        }

        match query_rows(&target.settings, sql, command_timeout).await {
            Ok(result) => return Ok(result),
            Err(error) => {
                let retryable = matches!(error, CoreError::Postgres { .. } | CoreError::Timeout(_));
                if !retryable || attempt >= MAX_RETRIES {
                    return Err(error);
                }

                attempt += 1;
                let delay = std::time::Duration::from_millis(2u64.pow(attempt) * 500);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Escreve os arquivos da execução (por servidor, consolidado, erros e log).
fn write_outputs(
    directory: &std::path::Path,
    timestamp: &str,
    successes: &[Success],
    error_rows: &[Vec<String>],
    log_rows: &[Vec<String>],
) -> Result<(), CoreError> {
    // CSV por servidor: cabeçalho com as colunas **daquele** resultado.
    for (target, columns, data) in successes {
        let header = header_for(columns);
        let rows: Vec<Vec<String>> = data
            .iter()
            .map(|row| {
                let mut fields = vec![target.server.clone(), target.database.clone()];
                fields.extend(row.iter().cloned());
                fields
            })
            .collect();

        let path = directory.join(csv_writer::server_file_name(&target.server, timestamp));
        csv_writer::write_file(&path, &csv_writer::render_csv(&header, &rows))?;
    }

    // Consolidado: união das colunas, na ordem de primeira aparição, e as linhas de todos.
    let mut columns: Vec<String> = Vec::new();
    for (_, result_columns, _) in successes {
        for column in result_columns {
            if !columns.contains(column) {
                columns.push(column.clone());
            }
        }
    }

    if !columns.is_empty() {
        let header = header_for(&columns);
        let mut rows: Vec<Vec<String>> = Vec::new();

        for (target, result_columns, data) in successes {
            for row in data {
                let mut fields = vec![target.server.clone(), target.database.clone()];
                for column in &columns {
                    let index = result_columns
                        .iter()
                        .position(|candidate| candidate == column);
                    fields.push(
                        index
                            .and_then(|index| row.get(index).cloned())
                            .unwrap_or_default(),
                    );
                }
                rows.push(fields);
            }
        }

        let path = directory.join(csv_writer::consolidated_file_name(timestamp));
        csv_writer::write_file(&path, &csv_writer::render_csv(&header, &rows))?;
    }

    if !error_rows.is_empty() {
        let header = ["Server", "Database", "ExecutedAt", "Error"]
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        let path = directory.join(csv_writer::errors_file_name(timestamp));
        csv_writer::write_file(&path, &csv_writer::render_csv(&header, error_rows))?;
    }

    let log_header = [
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
    .collect::<Vec<_>>();
    let path = directory.join(csv_writer::log_file_name(timestamp));
    csv_writer::write_file(&path, &csv_writer::render_csv(&log_header, log_rows))?;

    Ok(())
}

/// Cabeçalho do CSV de resultados: `Server`, `Database` e as colunas da consulta.
fn header_for(columns: &[String]) -> Vec<String> {
    let mut header = vec!["Server".to_string(), "Database".to_string()];
    header.extend(columns.iter().cloned());
    header
}

/// Bancos de um servidor, como o `GetDatabasesForServerAsync` do legado.
///
/// Descobre quando o servidor pede (`--all` liga em todos); se a descoberta falhar, **avisa e usa os
/// bancos do cadastro** (ou nenhum, quando não há). Os dois avisos aparecem porque são `LogWarning`
/// no legado, que imprime desse nível para cima.
async fn databases_for_server(
    server: &ServerConfigEntry,
    args: &QueryRunArgs,
    exclude_names: &std::collections::HashSet<String>,
) -> Result<Vec<String>, CoreError> {
    let discover = args.all || server.fetch_all_databases;

    let databases = if discover {
        // A conexão da descoberta vai no banco de manutenção, como no legado.
        let settings = settings::discovery_settings(server, args)?;
        match discovery::discover_databases(&settings).await {
            Ok(databases) => databases,
            Err(error) => {
                println!(
                    "Warning: Auto-discovery failed for '{}': {error}",
                    server.name
                );
                if server.databases.is_empty() {
                    return Ok(Vec::new());
                }

                println!("Falling back to configured databases.");
                server.databases.clone()
            }
        }
    } else {
        server.databases.clone()
    };

    // Os padrões do cadastro entram na descoberta; os nomes do `--exclude`, no fim (como no legado).
    Ok(discovery::filter_databases(
        databases,
        &server.exclude_patterns,
        exclude_names,
    ))
}

/// Linha do CSV de log (o `DurationMs` fica vazio: o legado mede o tempo por banco, que só o
/// progresso ao vivo usa — a medição entra no T35 junto da tabela).
fn log_row(
    target: &Target,
    executed_at: std::time::SystemTime,
    status: &str,
    rows: usize,
    error: &str,
) -> Vec<String> {
    vec![
        target.server.clone(),
        target.database.clone(),
        csv_writer::iso8601_for(executed_at),
        status.to_string(),
        rows.to_string(),
        String::new(),
        error.to_string(),
    ]
}

/// Caminho absoluto, como o `Path.GetFullPath` do legado.
fn absolute_path(path: &str) -> String {
    std::path::absolute(path)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> QueryRunArgs {
        QueryRunArgs {
            input: None,
            command: None,
            output: None,
            npgsql_connection_string: None,
            host: None,
            port: None,
            database: None,
            username: None,
            password: Some("senha".to_string()),
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            pooling: None,
            min_pool_size: None,
            max_pool_size: None,
            keepalive: None,
            connection_lifetime: None,
            all: false,
            exclude: None,
            no_confirm: false,
        }
    }

    #[test]
    fn the_inline_query_is_unescaped_like_the_legacy() {
        assert_eq!(unescape_inline_query("\"SELECT 1\""), "SELECT 1");
        assert_eq!(unescape_inline_query("'SELECT 1'"), "SELECT 1");
        assert_eq!(
            unescape_inline_query("SELECT \\\"nome\\\" FROM t"),
            "SELECT \"nome\" FROM t"
        );
        assert_eq!(unescape_inline_query("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn command_and_input_are_mutually_exclusive() {
        let mut command = args();
        command.command = Some("SELECT 1".to_string());
        command.input = Some("q.sql".to_string());

        assert_eq!(
            resolve_sql(&command).expect_err("recusa"),
            "Error: Options -c/--command and -i/--input are mutually exclusive. Use only one."
        );
    }

    #[test]
    fn neither_command_nor_input_is_refused() {
        assert_eq!(
            resolve_sql(&args()).expect_err("recusa"),
            "Error: Use -c for an inline query or -i for a SQL file."
        );
    }

    #[test]
    fn a_missing_input_file_is_refused_with_the_absolute_path() {
        let mut command = args();
        command.input = Some("nao-existe.sql".to_string());

        let message = resolve_sql(&command).expect_err("recusa");
        assert!(
            message.starts_with("Error: SQL input file not found: ")
                && message.ends_with("nao-existe.sql"),
            "{message}"
        );
    }

    #[test]
    fn a_traversal_input_path_is_refused() {
        let mut command = args();
        command.input = Some("%2e%2e%2fsegredo.sql".to_string());

        assert_eq!(
            resolve_sql(&command).expect_err("recusa"),
            "Error: Invalid input path: '%2e%2e%2fsegredo.sql'. Path traversal not allowed."
        );
    }

    #[test]
    fn an_empty_query_is_refused() {
        let mut command = args();
        command.command = Some("   ".to_string());

        assert_eq!(
            resolve_sql(&command).expect_err("recusa"),
            "Error: Use -c for an inline query or -i for a SQL file.",
            "espaço em branco conta como ausente, antes de chegar em 'query is empty'"
        );

        let mut comment_only = args();
        comment_only.command = Some("-- só um comentário".to_string());
        assert_eq!(
            resolve_sql(&comment_only).expect("aceita"),
            "-- só um comentário"
        );
    }
}
