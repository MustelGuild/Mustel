//! O encanamento do T41, de ponta a ponta: um caso de uso **real** do núcleo, num runtime de
//! background, com os eventos chegando ao canal que a janela lê e pedindo repaint ao `egui`.
//!
//! É o teste que prova o que a janela promete: o núcleo emite, o sink copia, a janela acorda. Sem
//! ele, `ChannelSink` e `request_repaint()` seriam só tipos.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::postgres::tls::SslMode;
use mustel_core::query::plan::ServerRun;
use mustel_core::usecases::query_run::QueryRunRequest;

use mustel_gui::events::{ChannelSink, GuiEvent};
use mustel_gui::query;
use mustel_gui::runtime::Runtime;

/// Servidor que não responde: a consulta falha, mas o caminho inteiro (plano, execução, resumo,
/// caminhos) emite eventos de verdade.
fn unreachable_server() -> ServerRun {
    ServerRun {
        name: "fora-do-ar".to_string(),
        max_parallelism: 1,
        databases: vec!["mydb".to_string()],
        exclude_patterns: Vec::new(),
        fetch_all_databases: false,
        settings: ConnectionSettings {
            host: "127.0.0.1".to_string(),
            port: 1,
            user: "postgres".to_string(),
            password: Some("segredo".to_string()),
            // `Require` falha na hora (a TLS não está ligada), então o teste não paga as
            // retentativas do executor.
            ssl_mode: SslMode::Require,
            connect_timeout: std::time::Duration::from_millis(1),
            ..ConnectionSettings::default()
        },
    }
}

#[test]
fn a_real_core_job_reaches_the_window_and_asks_for_a_repaint() {
    let output = tempfile::tempdir().expect("tempdir");
    let request = QueryRunRequest {
        servers: vec![unreachable_server()],
        sql: Arc::from("SELECT 1"),
        output_directory: output.path().to_path_buf(),
        max_parallelism: 1,
        command_timeout: std::time::Duration::from_secs(30),
        all: false,
        exclude: std::collections::HashSet::new(),
    };

    // O aviso de repaint passa pelo contexto **real** do `egui`: é a chamada que a janela faz.
    let context = egui::Context::default();
    let repaints = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&repaints);
    let (sink, events) = ChannelSink::with_repaint(Box::new(move || {
        context.request_repaint();
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    let sink = Arc::new(sink);

    let runtime = Runtime::new().expect("cria o runtime");
    let (_token, handle) = runtime.spawn(move |token| async move {
        let _ = query::run(&request, sink.as_ref(), &token).await;
    });
    runtime.block_on(handle).expect("o job termina sem pânico");

    // O sink entregou tudo o que o núcleo emitiu, já em forma própria.
    let received: Vec<GuiEvent> = events.try_iter().collect();
    assert!(
        received.iter().any(|event| matches!(
            event,
            GuiEvent::DatabaseFinished { status, .. }
                if *status == mustel_core::events::Status::Error
        )),
        "o banco que não respondeu vira evento: {received:?}"
    );
    assert!(
        received
            .iter()
            .any(|event| matches!(event, GuiEvent::Summary { .. })),
        "e o resumo também: {received:?}"
    );
    assert!(
        repaints.load(Ordering::SeqCst) > 0,
        "cada evento pediu repaint ao `egui`"
    );
}

#[test]
fn a_cancelled_job_runs_no_database() {
    let output = tempfile::tempdir().expect("tempdir");
    let request = QueryRunRequest {
        servers: vec![unreachable_server()],
        sql: Arc::from("SELECT 1"),
        output_directory: output.path().to_path_buf(),
        max_parallelism: 1,
        command_timeout: std::time::Duration::from_secs(30),
        all: false,
        exclude: std::collections::HashSet::new(),
    };

    let (sink, events) = ChannelSink::with_repaint(Box::new(|| {}));
    let sink = Arc::new(sink);
    let runtime = Runtime::new().expect("cria o runtime");

    let (token, handle) = runtime.spawn(move |token| async move {
        let _ = query::run(&request, sink.as_ref(), &token).await;
    });
    // Cancela antes de o job começar: o executor consulta o token antes de cada banco.
    token.cancel();
    runtime.block_on(handle).expect("o job termina sem pânico");

    let received: Vec<GuiEvent> = events.try_iter().collect();
    assert!(
        !received
            .iter()
            .any(|event| matches!(event, GuiEvent::DatabaseFinished { .. })),
        "nenhum banco executou: {received:?}"
    );
    // O resumo continua saindo, com zeros: é o comportamento do caso de uso (cancelar não é erro
    // dele; quem traduz para exit 130 é o CLI).
    assert!(
        received.iter().any(|event| matches!(
            event,
            GuiEvent::Summary {
                successes: 0,
                failures: 0,
                ..
            }
        )),
        "o resumo sai zerado: {received:?}"
    );
}
