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

use mustel_core::postgres::client::connect;
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::postgres::value::{TextValue, format_cell};

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
