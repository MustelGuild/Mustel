//! Testes de integração contra um Postgres de verdade (T19).
//!
//! **Gate**: só rodam quando `MUSTEL_PG_TEST_URL` está definida, porque dependem de um servidor no ar.
//! Nesta máquina o Docker Desktop estava parado, então **não** foram executados durante a migração —
//! o que valida a camada até aqui são os testes unitários de `postgres/*`.
//!
//! Como rodar:
//! ```text
//! docker run -d --name mustel-t19-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=mustel_test -p 55432:5432 postgres:16
//! MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-core --test postgres_integration
//! ```

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mustel_core::postgres::client::{connect, create_database_if_missing};
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::postgres::value::{TextValue, format_cell};
use mustel_core::query::executor::{self, DatabaseTarget, ExecutionRequest, ServerPlan};
use mustel_core::signal::cancellation_token;

/// Configuração derivada de `MUSTEL_PG_TEST_URL`, ou `None` quando o gate não foi aberto.
///
/// Usa o próprio parser do driver: assim o teste não depende de mais uma biblioteca e exercita o
/// mesmo caminho de configuração que a aplicação usa.
fn settings() -> Option<ConnectionSettings> {
    let url = std::env::var("MUSTEL_PG_TEST_URL").ok()?;
    let config = tokio_postgres::Config::from_str(&url).ok()?;

    let host = config
        .get_hosts()
        .first()
        .map(|host| match host {
            tokio_postgres::config::Host::Tcp(name) => name.clone(),
        })
        .unwrap_or_else(|| "localhost".to_string());

    Some(ConnectionSettings {
        host,
        port: config.get_ports().first().copied().unwrap_or(5432),
        user: config.get_user().unwrap_or("postgres").to_string(),
        password: config
            .get_password()
            .map(|bytes| String::from_utf8_lossy(bytes).to_string()),
        database: config.get_dbname().map(str::to_string),
        ..ConnectionSettings::default()
    })
}

#[tokio::test]
async fn connects_and_reads_a_scalar() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL to run the integration tests");
        return;
    };

    let client = connect(&settings).await.expect("connect");
    let row = client
        .query_one("SELECT 1 AS one", &[])
        .await
        .expect("query");
    let value: i32 = row.get("one");
    assert_eq!(value, 1);
}

#[tokio::test]
async fn a_wrong_password_is_classified_for_the_exit_codes() {
    let Some(mut settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    settings.password = Some("senha-errada".to_string());
    let error = connect(&settings).await.expect_err("must fail");

    // Autenticação recusada **tem** SQLSTATE (`28P01`), então o oráculo devolve 10 (a mesma regra:
    // `NpgsqlException { SqlState: not null }` → 10). O 11 fica para falhas sem SQLSTATE.
    let mustel_core::error::CoreError::Postgres { sqlstate, .. } = &error else {
        panic!("falha de autenticação precisa ser erro de Postgres: {error:?}");
    };
    assert_eq!(
        sqlstate.as_deref(),
        Some("28P01"),
        "SQLSTATE de senha inválida"
    );
    assert_eq!(
        error.exit_code(),
        mustel_core::error::exit_code::POSTGRES_WITH_SQLSTATE,
        "com SQLSTATE o código é 10"
    );
}

#[tokio::test]
async fn an_unreachable_server_fails_instead_of_hanging() {
    let Some(mut settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    // Porta fechada na própria máquina: recusa rápida, sem depender de timeout longo.
    settings.port = 1;
    settings.connect_timeout = std::time::Duration::from_secs(2);

    let started = std::time::Instant::now();
    let error = connect(&settings).await.expect_err("must fail");
    assert!(
        matches!(error, mustel_core::error::CoreError::Postgres { .. }),
        "recusa de conexão precisa ser erro de Postgres: {error:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "não pode ficar pendurado no timeout"
    );
}

#[tokio::test]
/// CONTRACT: IF-071
/// CONTRACT: IF-072
/// CONTRACT: IF-165
async fn create_database_if_missing_creates_it_once() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    // Nome fixo para o teste conseguir limpar o que cria, mesmo se falhar no meio.
    let name = "mustel_it_banco_criado";
    drop_database(&settings, name).await;

    let created = create_database_if_missing(&settings, name)
        .await
        .expect("cria o banco");
    assert!(created, "a primeira chamada cria");

    let created_again = create_database_if_missing(&settings, name)
        .await
        .expect("confere o banco");
    assert!(!created_again, "a segunda chamada só confirma que existe");

    // E é um banco de verdade: aceita conexão.
    {
        let target = ConnectionSettings {
            database: Some(name.to_string()),
            ..settings.clone()
        };
        let client = connect(&target).await.expect("conecta no banco criado");
        let one: i32 = client
            .query_one("SELECT 1", &[])
            .await
            .expect("roda consulta")
            .get(0);
        assert_eq!(one, 1);
    }

    drop_database(&settings, name).await;
}

/// Derruba o banco de teste que este arquivo cria (o nome é fixo, então o teste é o dono dele).
async fn drop_database(settings: &ConnectionSettings, name: &str) {
    let maintenance = ConnectionSettings {
        database: Some("postgres".to_string()),
        ..settings.clone()
    };

    if let Ok(client) = connect(&maintenance).await {
        let _ = client
            .batch_execute(&format!("DROP DATABASE IF EXISTS \"{name}\""))
            .await;
    }
}

#[tokio::test]
async fn reads_the_types_the_csv_export_cares_about() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    let client = connect(&settings).await.expect("connect");
    let row = client
        .query_one(
            "SELECT true AS b, 9223372036854775807::int8 AS big, 1.5::float8 AS f, \
             'texto'::text AS t, '{\"a\":1}'::jsonb AS j, NULL::int AS n",
            &[],
        )
        .await
        .expect("query");

    // O que se verifica aqui é que cada tipo tem caminho de conversão definido; o texto que vai ao
    // CSV é conferido contra o oráculo no T32 (ver postgres/value.rs).
    assert!(row.get::<_, bool>("b"));
    assert_eq!(row.get::<_, i64>("big"), i64::MAX);
    assert!((row.get::<_, f64>("f") - 1.5).abs() < f64::EPSILON);
    assert_eq!(row.get::<_, &str>("t"), "texto");
    assert_eq!(row.get::<_, Option<i32>>("n"), None);
    assert_eq!(format_cell(&TextValue::Null), "");

    let json: serde_json::Value = row.get("j");
    assert_eq!(json["a"], 1);
}

#[tokio::test]
/// CONTRACT: IF-039
async fn the_query_stream_hands_the_rows_one_by_one() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    let mut stream = mustel_core::postgres::client::QueryStream::open(
        &settings,
        "SELECT n AS id, 'texto ' || n AS texto FROM generate_series(1, 500) AS n",
        Duration::from_secs(30),
    )
    .await
    .expect("abre a consulta");

    assert_eq!(stream.columns(), ["id".to_string(), "texto".to_string()]);

    let mut rows = Vec::new();
    while let Some(row) = stream.next_row().await.expect("linha") {
        rows.push(row);
    }

    assert_eq!(rows.len(), 500, "o fluxo entrega todas as linhas");
    assert_eq!(rows[0], ["1".to_string(), "texto 1".to_string()]);
    assert_eq!(rows[499], ["500".to_string(), "texto 500".to_string()]);
}

#[tokio::test]
async fn the_command_timeout_fires_on_a_slow_read() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    // `pg_sleep` devolve `void` (uma linha) depois de dormir; com o timeout de 1s a execução estoura,
    // como o `CommandTimeout` do Npgsql. O estouro pode acontecer na abertura (o servidor só manda a
    // descrição das colunas quando começa a produzir o resultado) ou na leitura da linha.
    let error = match mustel_core::postgres::client::QueryStream::open(
        &settings,
        "SELECT pg_sleep(3)",
        Duration::from_secs(1),
    )
    .await
    {
        Err(error) => error,
        Ok(mut stream) => stream.next_row().await.expect_err("estoura no timeout"),
    };

    assert!(
        matches!(error, mustel_core::error::CoreError::Timeout(_)),
        "erro de timeout, não de Postgres: {error:?}"
    );
}

/// Roda o executor com dois servidores, um banco cada, e devolve o tempo.
async fn run_two_servers(settings: &ConnectionSettings, max_parallelism: usize) -> f64 {
    let directory = tempfile::tempdir().expect("tempdir");
    let request = ExecutionRequest {
        servers: ["um", "dois"]
            .iter()
            .map(|name| ServerPlan {
                name: (*name).to_string(),
                max_parallelism: 1,
                databases: vec![DatabaseTarget {
                    database: settings.database.clone().unwrap_or_default(),
                    settings: settings.clone(),
                }],
            })
            .collect(),
        sql: Arc::from("SELECT pg_sleep(2)"),
        directory: directory.path().to_path_buf(),
        timestamp: "ts".to_string(),
        max_parallelism,
        command_timeout: Duration::from_secs(60),
    };

    let started = Instant::now();
    let outcome = executor::execute(
        &request,
        &mustel_core::events::NoopSink,
        &cancellation_token(),
    )
    .await
    .expect("executa");
    let elapsed = started.elapsed().as_secs_f64();

    assert_eq!(outcome.successes, 2, "os dois servidores responderam");
    assert_eq!(outcome.failures, 0);
    assert!(
        outcome.consolidated.is_some(),
        "com linhas dos dois, sai consolidado"
    );

    elapsed
}

#[tokio::test]
async fn servers_run_in_parallel_up_to_the_configured_degree() {
    let Some(settings) = settings() else {
        eprintln!("skipped: set MUSTEL_PG_TEST_URL");
        return;
    };

    // Cada servidor dorme 2s. Com grau 2 os dois dormem juntos (~2s); com grau 1, um depois do outro
    // (~4s). É o `defaults.MaxParallelism` do `Parallel.ForEachAsync` externo do legado.
    let parallel = run_two_servers(&settings, 2).await;
    let serial = run_two_servers(&settings, 1).await;

    assert!(
        serial > parallel * 1.5,
        "grau 1 tinha de ser bem mais lento: paralelo {parallel:.1}s, serial {serial:.1}s"
    );
}
