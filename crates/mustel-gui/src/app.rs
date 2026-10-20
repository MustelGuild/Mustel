//! A janela do `mustel gui` (T41).
//!
//! Escopo desta entrega: o encanamento e a primeira tela de verdade. A janela lista os servidores do
//! cadastro (lidos do núcleo), testa a conexão de um deles no runtime de background e mostra os
//! eventos do `ChannelSink` num painel. As telas de CRUD e do `query run` são o T42.
//!
//! Regras que valem aqui:
//!
//! - **nenhuma regra duplicada**: a lista vem de [`mustel_core::usecases::db_servers::list`] e o
//!   teste de conexão de [`mustel_core::usecases::db_servers::test_connection`]; a janela só desenha;
//! - **a UI nunca bloqueia**: o teste roda no runtime de background e o resultado chega por canal;
//! - **cancelar é do usuário**: o botão `Cancel` cancela o token do job em andamento.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::events::Status;
use mustel_core::signal::CancellationToken;
use mustel_core::usecases::db_servers;

use crate::events::{ChannelSink, GuiEvent};
use crate::query;
use crate::runtime::Runtime;

/// Como o job de teste de conexão terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    /// O teste passou, com os bancos encontrados.
    Databases(Vec<String>),
    /// O teste falhou, com a mensagem pronta do núcleo.
    Failed(String),
}

/// O que a janela mostra sobre o job em andamento.
struct Running {
    /// Rótulo do job (servidor ou "query").
    label: String,
    /// Token para cancelar.
    token: CancellationToken,
}

/// A aplicação.
pub struct App {
    runtime: Runtime,
    /// Eventos do núcleo (progresso, avisos, resumo) — a **mesma** trait que o CLI implementa.
    events: Receiver<GuiEvent>,
    /// O sink que os jobs do núcleo recebem; mora aqui porque o job só o empresta.
    sink: Arc<ChannelSink>,
    /// Resultado dos jobs que devolvem valor (teste de conexão).
    results: Sender<TestResult>,
    results_rx: Receiver<TestResult>,
    servers: Vec<ServerConfigEntry>,
    /// Erro ao ler o cadastro, quando houver.
    config_error: Option<String>,
    running: Option<Running>,
    log: Vec<String>,
    status: String,
    /// Último resultado de teste, se houver.
    last_test: Option<TestResult>,
    /// SQL da tela.
    sql: String,
}

impl App {
    /// Monta a aplicação, carregando o cadastro.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        runtime: Runtime,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // O sink pede repaint no contexto do `egui` a cada evento: é o que acorda a janela sem
        // polling. Ele é compartilhado com os jobs por `Arc` porque `emit` recebe `&self`.
        let context = cc.egui_ctx.clone();
        let (sink, events) = ChannelSink::with_repaint(Box::new(move || context.request_repaint()));
        let (results, results_rx) = std::sync::mpsc::channel();

        let (servers, config_error) = match ConfigStore::with_default_path() {
            Ok(store) => match db_servers::list(&store) {
                Ok(servers) => (servers, None),
                Err(error) => (Vec::new(), Some(error.to_string())),
            },
            Err(error) => (Vec::new(), Some(error.to_string())),
        };

        Ok(Self {
            runtime,
            events,
            sink: Arc::new(sink),
            results,
            results_rx,
            servers,
            config_error,
            running: None,
            log: Vec::new(),
            status: "Ready.".to_string(),
            last_test: None,
            sql: "SELECT 1 AS valor".to_string(),
        })
    }

    /// Recarrega a lista de servidores do cadastro.
    fn reload_servers(&mut self) {
        let result = ConfigStore::with_default_path()
            .map_err(mustel_core::error::CoreError::from)
            .and_then(|store| db_servers::list(&store));

        match result {
            Ok(servers) => {
                self.servers = servers;
                self.config_error = None;
                self.status = "Servers reloaded.".to_string();
            }
            Err(error) => {
                self.servers.clear();
                self.config_error = Some(error.to_string());
            }
        }
    }

    /// Testa a conexão de um servidor no runtime de background.
    fn test_connection(&mut self, server: ServerConfigEntry, ctx: &egui::Context) {
        if let Some(running) = &self.running {
            self.status = format!("Already running: {}.", running.label);
            return;
        }

        let password = match query::stored_password(&server.name) {
            Ok(password) => password,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };

        self.status = format!(
            "Testing connection to {} ({}:{})...",
            server.name, server.host, server.port
        );
        self.last_test = None;

        let results = self.results.clone();
        let context = ctx.clone();
        let label = server.name.clone();
        let (token, _handle) = self.runtime.spawn(move |_token| async move {
            let outcome = db_servers::test_connection(&server, &password).await;
            let result = match outcome {
                Ok(report) => TestResult::Databases(report.databases),
                Err(error) => TestResult::Failed(error.to_string()),
            };

            // Thread de background não toca na UI: manda pelo canal e pede repaint.
            let _ = results.send(result);
            context.request_repaint();
        });

        self.running = Some(Running {
            label: format!("test {label}"),
            token,
        });
    }

    /// Roda o `query run` do núcleo contra um servidor, com o SQL da tela.
    fn run_query(&mut self, server: ServerConfigEntry, ctx: &egui::Context) {
        if let Some(running) = &self.running {
            self.status = format!("Already running: {}.", running.label);
            return;
        }

        let password = match query::stored_password(&server.name) {
            Ok(password) => password,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };

        let request = match query::request_for(&server, &self.sql, &password) {
            Ok(request) => request,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };

        self.log.clear();
        self.status = format!("Running query on {}...", server.name);
        self.last_test = None;

        let sink = Arc::clone(&self.sink);
        let context = ctx.clone();
        let results = self.results.clone();
        let label = server.name.clone();
        let (token, _handle) = self.runtime.spawn(move |token| async move {
            let outcome = query::run(&request, sink.as_ref(), &token).await;
            let result = match outcome {
                Ok(_) => TestResult::Databases(Vec::new()),
                Err(error) => TestResult::Failed(error.to_string()),
            };
            let _ = results.send(result);
            context.request_repaint();
        });

        self.running = Some(Running {
            label: format!("query on {label}"),
            token,
        });
    }

    /// Cancela o job em andamento.
    fn cancel(&mut self) {
        if let Some(running) = self.running.take() {
            running.token.cancel();
            self.status = "Cancel requested.".to_string();
        }
    }

    /// Aplica o resultado de um job, quando chega.
    fn drain_results(&mut self) {
        while let Ok(result) = self.results_rx.try_recv() {
            self.status = match &result {
                TestResult::Databases(databases) => {
                    if databases.is_empty() {
                        "No accessible databases found".to_string()
                    } else {
                        format!("Connection successful: {} database(s)", databases.len())
                    }
                }
                TestResult::Failed(error) => error.clone(),
            };
            self.last_test = Some(result);
            self.running = None;
        }
    }

    /// Consome os eventos que chegaram do núcleo desde o quadro anterior.
    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                GuiEvent::DatabaseFinished {
                    server,
                    database,
                    status,
                    rows,
                    duration,
                    error,
                } => self.log.push(match status {
                    Status::Success => format!(
                        "✓ {server}/{database} — {rows} rows — {:.1}s",
                        duration.as_secs_f64()
                    ),
                    Status::Error => format!("✗ {server}/{database} — {error}"),
                }),
                GuiEvent::ServerWithoutDatabases { server } => {
                    self.log.push(format!("— {server} — no databases"));
                }
                GuiEvent::DiscoveringDatabases { server } => {
                    self.status = format!("Discovering databases on {server}...");
                }
                GuiEvent::DiscoveryFailed { server, error } => self.log.push(format!(
                    "Warning: Auto-discovery failed for '{server}': {error}"
                )),
                GuiEvent::FallingBackToConfiguredDatabases { .. } => {
                    self.log
                        .push("Falling back to configured databases.".to_string());
                }
                GuiEvent::NoAccessibleDatabases => self
                    .log
                    .push("No accessible databases found across selected servers.".to_string()),
                GuiEvent::FilesWritten { .. } => self.log.push("Files written.".to_string()),
                GuiEvent::Summary {
                    servers,
                    successes,
                    failures,
                    total_rows,
                } => {
                    self.status = format!(
                        "Servers: {servers} | Success: {successes} | Failed: {failures} | Total rows: {total_rows}"
                    );
                }
                GuiEvent::NoDatabasesFound => self.log.push("No databases found.".to_string()),
                GuiEvent::NoDumpFilesFound => self
                    .log
                    .push("No .dump files found in the directory.".to_string()),
                GuiEvent::SkippingDatabase { database } => self
                    .log
                    .push(format!("Skipping invalid database name: '{database}'")),
                GuiEvent::DatabaseStepFailed {
                    step,
                    database,
                    error,
                } => {
                    let command = match step {
                        mustel_core::events::DatabaseStep::Backup => "backup",
                        mustel_core::events::DatabaseStep::Restore => "restore",
                    };
                    self.log.push(format!(
                        "Failed to {command} database '{database}': {error}"
                    ));
                }
            }
        }
    }
}

impl eframe::App for App {
    /// Estado: consome os canais. Aqui **nada** é desenhado (é o contrato do `eframe`).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.drain_results();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // O `Ui` do `eframe` vem sem margem nem fundo de painel: este `Frame` devolve o visual de
        // painel central que o resto do app espera.
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            self.central(ui, &ctx);
        });
    }
}

impl App {
    /// O conteúdo da janela.
    fn central(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("Mustel");
            ui.separator();
            ui.label(&self.status);
            if self.running.is_some() {
                ui.spinner();
                if ui.button("Cancel").clicked() {
                    self.cancel();
                }
            }
        });
        if let Some(error) = &self.config_error {
            ui.colored_label(egui::Color32::RED, error);
        }
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(format!("{} server(s) configured", self.servers.len()));
            if ui.button("Reload").clicked() {
                self.reload_servers();
            }
        });

        ui.label("SQL");
        ui.add(
            egui::TextEdit::multiline(&mut self.sql)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        ui.separator();

        let servers = self.servers.clone();
        egui::ScrollArea::vertical()
            .id_salt("servers")
            .max_height(160.0)
            .show(ui, |ui| {
                for server in servers {
                    ui.horizontal(|ui| {
                        ui.monospace(format!(
                            "{:<16} {}:{}",
                            server.name, server.host, server.port
                        ));
                        if ui.button("Test connection").clicked() {
                            self.test_connection(server.clone(), ctx);
                        }
                        if ui.button("Run query").clicked() {
                            self.run_query(server.clone(), ctx);
                        }
                    });
                }
            });

        if let Some(result) = &self.last_test {
            ui.separator();
            match result {
                TestResult::Databases(databases) => {
                    if databases.is_empty() {
                        ui.label("No accessible databases found");
                    } else {
                        ui.label(format!("Databases found: {}", databases.join(", ")));
                    }
                }
                TestResult::Failed(error) => {
                    ui.colored_label(egui::Color32::RED, error);
                }
            }
        }

        ui.separator();
        ui.label("Events");
        egui::ScrollArea::vertical()
            .id_salt("events")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.monospace(line);
                }
            });
    }
}
