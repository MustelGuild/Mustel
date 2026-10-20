//! `mustel query run` — executa a consulta nos bancos escolhidos e exporta o CSV.
//!
//! Escopo desta entrega (T32 + T33 + T34): descoberta de bancos, execução em paralelo com um escritor
//! único e escrita incremental. Fica para a próxima sub-entrega, e está registrado aqui para não
//! parecer esquecimento:
//!
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
//! - **execução** (T34): `defaults.maxParallelism` servidores ao mesmo tempo e, em cada servidor,
//!   `server.maxParallelism` bancos — o paralelismo do legado, com um canal para um escritor único
//!   (ver `mustel_core::query::executor`);
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
use std::sync::Arc;

use mustel_core::config::ServerConfigEntry;
use mustel_core::error::CoreError;
use mustel_core::query::analyzer::{self, QueryType};
use mustel_core::query::executor::{self, ExecutionRequest, ServerPlan};
use mustel_core::query::{csv_writer, discovery};
use mustel_core::signal::CancellationToken;
use mustel_core::validation;

use crate::cli::QueryRunArgs;
use crate::ui::{progress, prompts, style, table};

use super::settings;

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

    // A confirmação vem **depois** da seleção e do `--all`, como no legado — é quando já se sabe
    // quantos servidores e bancos a consulta vai atingir.
    if analyzer::analyze(&sql) == QueryType::Destructive
        && !args.no_confirm
        && settings::defaults()?.require_confirmation
    {
        let databases = selected
            .iter()
            .map(|server| {
                if args.all || server.fetch_all_databases {
                    // O legado conta 1 por servidor quando vai descobrir os bancos: não sabe quantos são.
                    1
                } else {
                    server.databases.len().max(1)
                }
            })
            .sum();

        if !confirm_destructive(
            &analyzer::description(&sql),
            selected.len(),
            databases,
            &sql,
        )? {
            println!("Query execution cancelled by user.");
            return Ok(1);
        }
    }

    execute(selected, &sql, &args, token).await
}

/// Aviso + tabela + confirmação do `ConfirmDestructiveQuery` do legado.
///
/// A tabela é N4 (o legado a desenha com bordas do Spectre); o que compõe contrato aqui é o texto da
/// pergunta e a decisão de seguir ou cancelar com exit 1.
fn confirm_destructive(
    description: &str,
    servers: usize,
    databases: usize,
    sql: &str,
) -> Result<bool, CoreError> {
    let preview: String = sql.chars().take(200).collect();
    let preview = if sql.chars().count() > 200 {
        format!("{preview}...")
    } else {
        preview
    };

    let rows = vec![
        vec!["Query Type".to_string(), description.to_string()],
        vec!["Servers Affected".to_string(), servers.to_string()],
        vec!["Databases Affected".to_string(), format!("~{databases}")],
        vec!["Preview".to_string(), preview],
    ];

    println!();
    println!("{}", style::danger("WARNING: DESTRUCTIVE QUERY DETECTED"));
    println!("{}", table::render(&["Property", "Value"], &rows));
    println!();

    prompts::confirm("Proceed with execution?", true)
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
    let defaults = settings::defaults()?;
    let base_directory = settings::output_directory(args, &defaults);
    let timestamp = csv_writer::timestamp_for(std::time::SystemTime::now());
    let directory = PathBuf::from(&base_directory).join(&timestamp);
    std::fs::create_dir_all(&directory).map_err(CoreError::Io)?;

    // Alvos: para cada servidor, os bancos do cadastro ou os descobertos (`--all` /
    // `fetchAllDatabases`), já filtrados pelos dois critérios do legado. Todo servidor selecionado
    // entra no plano, mesmo sem banco: o `Servers:` do resumo e a ordem do merge do consolidado
    // contam com eles (o legado monta a tabela e o merge da mesma lista).
    let exclude_names = discovery::exclude_names(args.exclude.as_deref());

    // A descoberta tem indicador próprio no legado (`AnsiConsole.Status`, só quando algum servidor
    // vai descobrir os bancos).
    let discovering = servers
        .iter()
        .any(|server| args.all || server.fetch_all_databases)
        .then(|| progress::Progress::spinner("Discovering databases..."));

    let mut plans: Vec<ServerPlan> = Vec::new();
    for server in &servers {
        if let Some(status) = &discovering {
            status.set_message(format!("Discovering databases on {}...", server.name));
        }

        let databases = databases_for_server(server, args, &exclude_names).await?;
        let databases = databases
            .iter()
            .map(|database| settings::target_for(server, database, args))
            .collect::<Result<Vec<_>, CoreError>>()?;

        plans.push(ServerPlan {
            name: server.name.clone(),
            max_parallelism: server.max_parallelism as usize,
            databases,
        });
    }

    if let Some(status) = discovering {
        status.finish();
    }

    if plans.iter().all(|plan| plan.databases.is_empty()) {
        // Aqui o legado sai antes de criar o canal, o log e o consolidado — e sem o resumo.
        println!("No accessible databases found across selected servers.");
        return Ok(0);
    }

    let request = ExecutionRequest {
        servers: plans,
        sql: Arc::from(sql),
        directory,
        timestamp,
        max_parallelism: defaults.max_parallelism.max(1) as usize,
        command_timeout: std::time::Duration::from_secs(
            args.command_timeout.unwrap_or(300).max(1) as u64
        ),
    };

    // A tabela ao vivo do legado: uma linha por banco que termina, desenhada no lugar e deixada na
    // tela no fim (fora de terminal, nada é escrito).
    let results = progress::LiveTable::new();
    let sink = progress::TableSink::new(&results);
    let outcome = executor::execute(&request, &sink, token).await?;
    results.finish();

    println!();
    if let Some(consolidated) = &outcome.consolidated {
        println!("✅ Consolidated → {}", consolidated.display());
    }
    if let Some(errors) = &outcome.errors {
        println!("❌ Errors       → {}", errors.display());
    }
    println!("Log           → {}", outcome.log.display());
    println!();
    println!(
        "Servers: {} | Success: {} | Failed: {} | Total rows: {}",
        request.servers.len(),
        outcome.successes,
        outcome.failures,
        outcome.total_rows
    );

    if outcome.successes == 0 && outcome.failures > 0 {
        return Err(CoreError::Other(
            "No server responded successfully. Please check the connections.".to_string(),
        ));
    }

    Ok(0)
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
