//! Frontend gráfico do Mustel (egui).
//!
//! Consome os mesmos casos de uso do `mustel-core` em processo, sem IPC (D9): recebe eventos por um
//! [`ChannelSink`], roda o trabalho em um runtime tokio de background e redesenha via
//! `ctx.request_repaint()`. Ligado ao binário `mustel` pelo subcomando `gui` (T41).
//!
//! Como o encanamento se junta:
//!
//! ```text
//! janela (thread principal, egui)            runtime tokio de background
//! App::update ──lê── Receiver<GuiEvent>      job: usecases::query_run::run(&request, sink, token)
//!             ──lê── Receiver<TestResult>              │
//!                                                   sink: ChannelSink
//!                                                      │ envia + request_repaint()
//!                                                      ▼
//!                                              a janela acorda e desenha
//! ```
//!
//! A crate inteira fica atrás da feature `gui` do binário: `--no-default-features` compila um CLI sem
//! `egui` (verificado no T41).

pub mod actions;
pub mod app;
pub mod desktop;
pub mod events;
pub mod picker;
pub mod query;
pub mod request;
pub mod runtime;
pub mod screens;
pub mod smooth_scroll;
pub mod sql_editor;

pub use app::App;
pub use events::{ChannelSink, GuiEvent};
pub use runtime::Runtime;

/// Erro ao abrir a janela.
#[derive(Debug, thiserror::Error)]
pub enum GuiError {
    /// O runtime tokio de background não pôde ser criado.
    #[error("failed to start the background runtime: {0}")]
    Runtime(#[from] std::io::Error),
    /// O `eframe` recusou abrir a janela (o evento mais comum é não haver servidor gráfico).
    #[error("failed to open the window: {0}")]
    Window(String),
}

/// Abre a janela e roda até ela ser fechada.
pub fn run() -> Result<(), GuiError> {
    let runtime = Runtime::new()?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 620.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Mustel"),
        ..Default::default()
    };

    eframe::run_native(
        "Mustel",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, runtime)?))),
    )
    .map_err(|error| GuiError::Window(error.to_string()))
}
