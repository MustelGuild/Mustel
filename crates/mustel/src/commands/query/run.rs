//! `mustel query run` — o adaptador de terminal do caso de uso.
//!
//! A orquestração (plano, descoberta, execução, regra do "nenhum servidor respondeu") mora em
//! [`mustel_core::usecases::query_run`]; aqui fica o que é de terminal: os prompts (arquivo de
//! entrada, seleção de servidores, confirmação da consulta destrutiva), a leitura do cadastro e do
//! cofre para resolver as conexões, e o desenho dos eventos em [`progress::TerminalSink`].
//!
//! Paridade medida e conferida no código do legado:
//!
//! - as validações de uso saem com exit **1** (não 2, como no `pgpass`): `-c` com `-i` juntos, `-i`
//!   inexistente, consulta vazia, nenhum servidor cadastrado, nada selecionado;
//! - o texto da consulta vem de `-c` (com o "desescape" de aspas externas do legado) ou do arquivo
//!   `-i`, lido como UTF-8;
//! - a confirmação da consulta destrutiva vem **depois** da seleção e do `--all`, e só quando o
//!   cadastro pede (`defaults.requireConfirmation`); responder não sai com exit 1;
//! - se **nenhum** servidor respondeu e houve falha, o caso de uso devolve
//!   `No server responded successfully. Please check the connections.` (exit 1); sem banco nenhum,
//!   sai `No accessible databases found across selected servers.` e exit **0**;
//! - a senha segue `-W` → cofre (pelo **nome do servidor**) → prompt, com o texto
//!   `Password for '<servidor>': `, resolvida **uma vez por servidor** (a entrada vai pronta para o
//!   núcleo; diferença 17 em `docs/explanation/diferencas-deliberadas.md`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mustel_core::config::ServerConfigEntry;
use mustel_core::error::CoreError;
use mustel_core::query::analyzer::{self, QueryType};
use mustel_core::query::discovery;
use mustel_core::signal::CancellationToken;
use mustel_core::usecases::query_run::{self, QueryRunRequest};
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

    let selected = select_servers(&servers)?;
    if selected.is_empty() {
        println!("No servers selected. Execution cancelled.");
        return Ok(1);
    }

    let defaults = settings::defaults()?;

    // A confirmação vem **depois** da seleção e do `--all`, como no legado — é quando já se sabe
    // quantos servidores e bancos a consulta vai atingir.
    if needs_confirmation(&sql, args.no_confirm, defaults.require_confirmation) {
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

    let request = QueryRunRequest {
        servers: selected
            .iter()
            .map(|server| settings::server_run(server, &args))
            .collect::<Result<Vec<_>, CoreError>>()?,
        sql: Arc::from(sql.as_str()),
        output_directory: PathBuf::from(settings::output_directory(&args, &defaults)),
        max_parallelism: defaults.max_parallelism as usize,
        command_timeout: Duration::from_secs(args.command_timeout.unwrap_or(300).max(1) as u64),
        all: args.all,
        exclude: discovery::exclude_names(args.exclude.as_deref()),
        escape_formulas: args.csv_escape_formulas,
    };

    // Os eventos vão para a tabela ao vivo; o que sai depois dela (caminhos e resumo) fica na fila
    // do sink e só aparece em `finish()`. O `finish` roda mesmo quando o caso de uso devolve erro: no
    // legado o resumo sai antes da mensagem de erro.
    let sink = progress::TerminalSink::new();
    let outcome = query_run::run(&request, &sink, token).await;
    sink.finish();
    outcome?;

    Ok(0)
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
fn select_servers(servers: &[ServerConfigEntry]) -> Result<Vec<ServerConfigEntry>, CoreError> {
    if servers.len() == 1 {
        return Ok(servers.to_vec());
    }

    let names: Vec<String> = servers.iter().map(|server| server.name.clone()).collect();
    let preselected: Vec<usize> = (0..names.len()).collect();
    let selected =
        prompts::select_many("Select servers to execute query on:", &names, &preselected)?;

    Ok(servers
        .iter()
        .filter(|server| selected.contains(&server.name))
        .cloned()
        .collect())
}

/// Caminho absoluto, como o `Path.GetFullPath` do legado.
/// A consulta pede confirmação antes de rodar?
///
/// Três condições, na ordem do legado: a consulta é destrutiva, o usuário **não** passou
/// `--no-confirm`, e o cadastro **não** desligou `requireConfirmation`. Extraída para poder ser
/// testada sem terminal — o prompt em si é N3 e vive nos testes de PTY.
fn needs_confirmation(sql: &str, no_confirm: bool, require_confirmation: bool) -> bool {
    analyzer::analyze(sql) == QueryType::Destructive && !no_confirm && require_confirmation
}

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
            csv_escape_formulas: false,
        }
    }

    #[test]
    /// CONTRACT: IF-015
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
    /// CONTRACT: IF-011
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
    /// CONTRACT: IF-012
    fn neither_command_nor_input_is_refused() {
        assert_eq!(
            resolve_sql(&args()).expect_err("recusa"),
            "Error: Use -c for an inline query or -i for a SQL file."
        );
    }

    #[test]
    /// CONTRACT: IF-013
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
    /// CONTRACT: IF-014
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

    /// IF-181: `--no-confirm` dispensa a confirmação; `requireConfirmation=false` também; e uma
    /// consulta de leitura nunca pede confirmação.
    ///
    /// A tabela fixa as três condições de uma vez — em especial o bypass do `--no-confirm`, que é o
    /// que torna o comando utilizável em automação.
    /// CONTRACT: IF-181
    #[test]
    fn only_a_destructive_query_without_no_confirm_asks_for_confirmation() {
        let destructive = "DROP TABLE clientes";
        let reading = "SELECT * FROM clientes";

        assert!(needs_confirmation(destructive, false, true));
        assert!(
            !needs_confirmation(destructive, true, true),
            "--no-confirm dispensa"
        );
        assert!(
            !needs_confirmation(destructive, false, false),
            "requireConfirmation=false dispensa"
        );
        assert!(
            !needs_confirmation(reading, false, true),
            "consulta de leitura não pede confirmação"
        );
    }
}
