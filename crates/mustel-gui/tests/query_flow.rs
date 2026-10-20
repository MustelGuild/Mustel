//! O caminho da tela de consulta contra um Postgres **de verdade** (T42).
//!
//! **Gate**: só roda com `MUSTEL_PG_TEST_URL` definida, como o teste de integração do núcleo. O que
//! ele prova é o que a janela faz de ponta a ponta: montar a entrada com
//! [`mustel_gui::query::request_for_runs`], rodar o caso de uso e **ler de volta o consolidado** — é
//! exatamente o que a tabela virtualizada mostra.
//!
//! Como rodar:
//! ```text
//! MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test \
//!   cargo test -p mustel-gui --test query_flow
//! ```

use std::str::FromStr;

use mustel_core::config::ServerConfigEntry;
use mustel_core::query::csv_writer::parse_records;
use mustel_core::query::plan::ServerRun;
use mustel_core::usecases::query_run;

use mustel_gui::query;

/// O servidor do teste, derivado de `MUSTEL_PG_TEST_URL`.
fn server_from_url() -> Option<(ServerConfigEntry, ServerRun, String)> {
    let url = std::env::var("MUSTEL_PG_TEST_URL").ok()?;
    let parsed = tokio_postgres::Config::from_str(&url).ok()?;

    let tokio_postgres::config::Host::Tcp(host) = parsed.get_hosts().first()?;
    let host = host.clone();
    let port = parsed.get_ports().first().copied().unwrap_or(5432);
    let user = parsed.get_user().unwrap_or("postgres").to_string();
    let password = parsed
        .get_password()
        .map(|password| String::from_utf8_lossy(password).to_string())?;
    let database = parsed.get_dbname().unwrap_or("postgres").to_string();

    let server = ServerConfigEntry {
        name: "integracao".to_string(),
        host,
        port,
        username: user,
        databases: vec![database],
        ssl_mode: "Disable".to_string(),
        timeout: 30,
        ..ServerConfigEntry::default()
    };

    let run = ServerRun {
        name: server.name.clone(),
        max_parallelism: 1,
        databases: server.databases.clone(),
        exclude_patterns: Vec::new(),
        fetch_all_databases: false,
        settings: mustel_core::postgres::settings::ConnectionSettings {
            host: server.host.clone(),
            port: server.port,
            user: server.username.clone(),
            password: Some(password.clone()),
            database: None,
            ssl_mode: mustel_core::postgres::tls::SslMode::Disable,
            ..mustel_core::postgres::settings::ConnectionSettings::default()
        },
    };

    Some((server, run, password))
}

#[test]
fn the_query_screen_path_produces_a_csv_that_reloads_into_table_rows() {
    let Some((_server, run, _password)) = server_from_url() else {
        eprintln!("pulado: defina MUSTEL_PG_TEST_URL para rodar contra um Postgres de verdade");
        return;
    };

    let output = tempfile::tempdir().expect("tempdir");
    let request = query::request_for_runs(
        vec![run],
        "SELECT 1 AS valor",
        output.path().display().to_string(),
    )
    .expect("monta a entrada");

    let runtime = mustel_gui::runtime::Runtime::new().expect("runtime");
    let outcome = runtime.block_on(query::run(
        &request,
        &mustel_core::events::NoopSink,
        &mustel_core::signal::cancellation_token(),
    ));

    let outcome = outcome.expect("a consulta roda");
    let query_run::QueryRunOutcome::Executed(outcome) = outcome else {
        panic!("esperava uma execução, veio {outcome:?}");
    };

    assert_eq!(outcome.successes, 1, "o banco respondeu");
    assert_eq!(outcome.total_rows, 1, "uma linha");

    let consolidated = outcome.consolidated.expect("houve linha, há consolidado");
    let text = std::fs::read_to_string(&consolidated).expect("lê o consolidado");
    let mut records = parse_records(&text).into_iter();
    let header = records.next().expect("cabeçalho");

    assert_eq!(
        header,
        vec![
            "Server".to_string(),
            "Database".to_string(),
            "valor".to_string()
        ],
        "o cabeçalho da linha do servidor + colunas da consulta"
    );
    assert_eq!(
        records.next().expect("a linha"),
        vec![
            "integracao".to_string(),
            "mustel_test".to_string(),
            "1".to_string()
        ],
        "Server, Database e o valor da consulta"
    );
}
