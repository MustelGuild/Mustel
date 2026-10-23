//! A janela do `mustel gui` (T42).
//!
//! O `App` é o **executor**: tem o runtime de background, o `ChannelSink` e o token do job em
//! andamento. As telas ([`crate::screens`]) só desenham e devolvem [`Request`]s; aqui eles viram
//! chamadas de caso de uso, e o resultado volta por canal para a tela que o pediu.
//!
//! Regras que valem aqui:
//!
//! - **nenhuma regra duplicada**: tudo passa por `mustel_core::usecases::{db_servers, query_run}`;
//! - **a UI nunca bloqueia**: todo job roda no runtime de background e avisa por canal +
//!   `ctx.request_repaint()`;
//! - **um job por vez**: o botão `Cancel` cancela o token do job atual.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::events::Status;
use mustel_core::signal::CancellationToken;
use mustel_core::usecases::{db_servers, gui_prefs, query_run};

use crate::actions;
use crate::events::{ChannelSink, GuiEvent};
use crate::query;
use crate::request::{JobResult, Request};
use crate::runtime::Runtime;
use crate::screens::{
    query as query_screen, servers as servers_screen, settings as settings_screen,
};

/// Qual tela está à frente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Servers,
    Query,
    Settings,
}

/// O job em andamento.
struct Running {
    /// Rótulo para a barra de estado.
    label: String,
    /// Token para cancelar.
    token: CancellationToken,
}

/// A aplicação.
pub struct App {
    runtime: Runtime,
    /// O contexto do `egui`, guardado para pedir repaint de fora do desenho (ao encadear jobs).
    ctx: egui::Context,
    /// Eventos do núcleo (progresso, avisos, resumo).
    events: Receiver<GuiEvent>,
    /// O sink que os jobs recebem.
    sink: Arc<ChannelSink>,
    /// Resultados dos jobs.
    results: Sender<JobResult>,
    results_rx: Receiver<JobResult>,
    screen: Screen,
    running: Option<Running>,
    status: String,
    config_error: Option<String>,
    servers: servers_screen::State,
    query: query_screen::State,
    /// Esquemas carregados para o autocomplete, por `(servidor, banco)`.
    schemas: crate::schema_cache::SchemaCache,
    /// Bancos descobertos e senha presente, por servidor (linha "Referência do autocomplete").
    reference: crate::reference::Reference,
    /// Aba Settings: preferências de aparência.
    settings: settings_screen::State,
}

impl App {
    /// Monta a aplicação, carregando o cadastro e os defaults.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        runtime: Runtime,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // O sink pede repaint no contexto do `egui` a cada evento: é o que acorda a janela sem
        // polling.
        let context = cc.egui_ctx.clone();
        let (sink, events) = ChannelSink::with_repaint(Box::new(move || context.request_repaint()));
        let (results, results_rx) = std::sync::mpsc::channel();

        let mut config_error = None;
        let (servers, output_directory) = match ConfigStore::with_default_path() {
            Ok(store) => {
                let defaults = query_run::defaults(&store);
                let servers = db_servers::list(&store);
                match (servers, defaults) {
                    (Ok(servers), Ok(defaults)) => (servers, defaults.output_directory),
                    (Err(error), _) | (_, Err(error)) => {
                        config_error = Some(error.to_string());
                        (Vec::new(), String::new())
                    }
                }
            }
            Err(error) => {
                config_error = Some(error.to_string());
                (Vec::new(), String::new())
            }
        };

        let mut query = query_screen::State::with_output_directory({
            // O campo mostra o caminho **absoluto**: é o mesmo que a execução usa e o que aparece
            // depois nos arquivos, então não há dúvida de onde o CSV foi parar.
            query::absolute(&output_directory).display().to_string()
        });
        query.sync_servers(&servers);

        // Preferências de aparência: lidas do cadastro na abertura e aplicadas antes do primeiro
        // quadro (senão a janela pisca no tamanho padrão).
        let prefs = ConfigStore::with_default_path()
            .ok()
            .and_then(|store| mustel_core::usecases::gui_prefs::load(&store).ok())
            .flatten();
        let settings = settings_screen::State::with_prefs(prefs.as_ref());
        crate::fonts::apply(&cc.egui_ctx, settings.chosen());

        Ok(Self {
            runtime,
            ctx: cc.egui_ctx.clone(),
            events,
            sink: Arc::new(sink),
            results,
            results_rx,
            screen: Screen::Query,
            running: None,
            status: "Ready.".to_string(),
            config_error,
            schemas: crate::schema_cache::SchemaCache::default(),
            reference: crate::reference::Reference::default(),
            servers: servers_screen::State::new(servers),
            query,
            settings,
        })
    }

    /// O cadastro atual (as duas telas leem dele).
    fn servers_snapshot(&self) -> Vec<ServerConfigEntry> {
        self.servers.servers().to_vec()
    }

    /// Executa um pedido de tela.
    fn dispatch(&mut self, request: Request, ctx: &egui::Context) {
        match request {
            // Aplicar é síncrono: mudar fonte é mexer no contexto do egui, não trabalho de núcleo.
            Request::ApplyGuiPrefs(chosen) => {
                let problems = crate::fonts::apply(ctx, &chosen);
                self.settings.set_warning(problems.first().cloned());

                // O mínimo acompanha a escala: fonte maior precisa de janela maior para o layout
                // continuar com a mesma folga que tinha em 1.0.
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                    crate::fonts::min_window_size(chosen.scale),
                ));
            }
            // Gravar vai para o núcleo (mesmo caminho do cadastro).
            Request::SaveGuiPrefs(prefs) => {
                // Aplica **e** grava: é o commit da aba Settings. Aplicar aqui é o que faz trocar de
                // fonte surtir efeito imediato (a prévia ao vivo é do `ApplyGuiPrefs`).
                let chosen = crate::fonts::Chosen::from_prefs(Some(&prefs));
                let problems = crate::fonts::apply(ctx, &chosen);
                self.settings.set_warning(problems.first().cloned());
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                    crate::fonts::min_window_size(chosen.scale),
                ));

                match ConfigStore::with_default_path() {
                    Ok(store) => match gui_prefs::save(&store, Some(prefs)) {
                        Ok(()) => {
                            self.status = "Appearance saved.".to_string();
                            ctx.request_repaint();
                        }
                        Err(error) => self.status = error.to_string(),
                    },
                    Err(error) => self.status = error.to_string(),
                }
            }
            Request::CancelJob => self.cancel(),
            Request::ReloadServers => {
                let results = self.results.clone();
                self.spawn("reload servers", ctx, move |_token| async move {
                    let result = with_store(actions::load_servers);
                    let _ = results.send(result);
                });
            }
            Request::SaveServer { original, server } => {
                let results = self.results.clone();
                self.spawn("save server", ctx, move |_token| async move {
                    let result = with_store(|store| {
                        actions::save_server(store, original.as_deref(), &server)
                    });
                    let _ = results.send(result);
                });
            }
            Request::RemoveServer(name) => {
                let results = self.results.clone();
                self.spawn("remove server", ctx, move |_token| async move {
                    let result = with_store(|store| actions::remove_server(store, &name));
                    let _ = results.send(result);
                });
            }
            Request::SetPassword { name, password } => {
                let results = self.results.clone();
                self.spawn("set password", ctx, move |_token| async move {
                    let result =
                        with_store(|store| actions::save_password(store, &name, &password));
                    let _ = results.send(result);
                });
            }
            Request::TestConnection(server) => {
                let name = server.name.clone();
                let password = match query::stored_password(&name) {
                    Ok(password) => password,
                    Err(error) => {
                        self.status = error.to_string();
                        return;
                    }
                };
                let results = self.results.clone();
                self.status = format!("Testing connection to {name}...");
                self.spawn("test connection", ctx, move |_token| async move {
                    let result = match db_servers::test_connection(&server, &password).await {
                        Ok(report) => JobResult::Tested {
                            server: name,
                            databases: report.databases,
                        },
                        Err(error) => JobResult::Failed(error.to_string()),
                    };
                    let _ = results.send(result);
                });
            }
            Request::DiscoverDatabases { server } => {
                let Some(entry) = self
                    .servers_snapshot()
                    .into_iter()
                    .find(|candidate| candidate.name == server)
                else {
                    self.status = format!("Server '{server}' not found.");
                    return;
                };

                // Fora do `spawn`: o `spawn` tem fila única ("Already running") e a descoberta não
                // pode esperar uma consulta longa terminar para o combo sair do "nenhum banco".
                self.reference.mark_pending(&server);
                self.status = format!("Discovering databases on '{server}'...");
                let results = self.results.clone();
                let context = ctx.clone();
                let (token, _handle) = self.runtime.spawn(move |_token| async move {
                    let (databases, has_password) = match query::stored_password(&entry.name) {
                        Ok(password) => (
                            query::databases(&entry, &password)
                                .await
                                .map_err(|error| error.to_string()),
                            true,
                        ),
                        Err(error) => (Err(error.to_string()), false),
                    };

                    let _ = results.send(JobResult::Databases {
                        server: entry.name,
                        databases,
                        has_password,
                    });
                    context.request_repaint();
                });
                let _ = token;
            }
            Request::LoadSchema { server, database } => {
                let Some(entry) = self
                    .servers_snapshot()
                    .into_iter()
                    .find(|candidate| candidate.name == server)
                else {
                    self.status = format!("Server '{server}' not found.");
                    return;
                };

                // Sem senha no cofre a janela **não** pergunta: avisa e não carrega.
                let password = match query::stored_password(&entry.name) {
                    Ok(password) => password,
                    Err(error) => {
                        self.status = error.to_string();
                        return;
                    }
                };

                let results = self.results.clone();
                let target = server.clone();
                let database_for_job = database.clone();
                self.status = format!("Loading schema of '{database}' on '{server}'...");
                self.spawn("load schema", ctx, move |_token| async move {
                    let result = match query::schema(&entry, &password, &database_for_job).await {
                        Ok(schema) => JobResult::Schema {
                            server: target,
                            database: database_for_job,
                            schema: Ok(schema),
                        },
                        Err(error) => JobResult::Schema {
                            server: target,
                            database: database_for_job,
                            schema: Err(error.to_string()),
                        },
                    };
                    let _ = results.send(result);
                });
            }
            Request::RunQuery {
                servers,
                sql,
                output_directory,
            } => {
                // A entrada é montada fora do runtime: se faltar senha, a tela avisa na hora.
                let mut runs = Vec::with_capacity(servers.len());
                for server in &servers {
                    let password = match query::stored_password(&server.name) {
                        Ok(password) => password,
                        Err(error) => {
                            self.status = error.to_string();
                            return;
                        }
                    };
                    match query::server_run(server, &password) {
                        Ok(run) => runs.push(run),
                        Err(error) => {
                            self.status = error.to_string();
                            return;
                        }
                    }
                }

                let request = match query::request_for_runs(runs, &sql, output_directory) {
                    Ok(request) => request,
                    Err(error) => {
                        self.status = error.to_string();
                        return;
                    }
                };

                self.query.started();
                self.status = "Running query...".to_string();
                let sink = Arc::clone(&self.sink);
                let results = self.results.clone();
                self.spawn("query", ctx, move |token| async move {
                    let result = match query::run(&request, sink.as_ref(), &token).await {
                        Ok(query_run::QueryRunOutcome::NoAccessibleDatabases) => {
                            JobResult::QueryWithoutDatabases
                        }
                        Ok(query_run::QueryRunOutcome::Executed(outcome)) => JobResult::QueryDone {
                            consolidated: outcome.consolidated,
                            log: outcome.log,
                            servers: request.servers.len(),
                            successes: outcome.successes,
                            failures: outcome.failures,
                            total_rows: outcome.total_rows,
                        },
                        Err(error) => JobResult::Failed(error.to_string()),
                    };
                    let _ = results.send(result);
                });
            }
        }
    }

    /// Dispara um job, guardando o token para o `Cancel`.
    fn spawn<F, Fut>(&mut self, label: &str, ctx: &egui::Context, job: F)
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        if let Some(running) = &self.running {
            self.status = format!("Already running: {}.", running.label);
            return;
        }

        let context = ctx.clone();
        let (token, _handle) = self.runtime.spawn(move |token| async move {
            job(token).await;
            // Thread de background não toca na UI: pede repaint para a janela ler o canal.
            context.request_repaint();
        });

        self.running = Some(Running {
            label: label.to_string(),
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

    /// Aplica o que chegou dos canais.
    fn drain(&mut self) {
        // Eventos de núcleo: progresso da consulta.
        while let Ok(event) = self.events.try_recv() {
            match event {
                GuiEvent::DatabaseFinished {
                    server,
                    database,
                    status,
                    rows,
                    duration,
                    error,
                } => self.query.push_progress(query_screen::ProgressRow {
                    server,
                    database,
                    rows: if status == Status::Success { rows } else { 0 },
                    seconds: duration.as_secs_f64(),
                    error,
                }),
                GuiEvent::DiscoveringDatabases { server } => {
                    self.status = format!("Discovering databases on {server}...");
                }
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
                GuiEvent::DiscoveryFailed { server, error } => {
                    self.status = format!("Auto-discovery failed for '{server}': {error}");
                }
                GuiEvent::FallingBackToConfiguredDatabases { server } => {
                    self.status =
                        format!("Falling back to the configured databases of '{server}'.");
                }
                GuiEvent::NoAccessibleDatabases => {
                    self.status = "No accessible databases found.".to_string();
                }
                // Os avisos de `backup`/`restore` não têm tela nesta entrega (backlog da GUI).
                GuiEvent::FilesWritten { .. }
                | GuiEvent::ServerWithoutDatabases { .. }
                | GuiEvent::NoDatabasesFound
                | GuiEvent::NoDumpFilesFound
                | GuiEvent::SkippingDatabase { .. }
                | GuiEvent::DatabaseStepFailed { .. } => {}
            }
        }

        // Resultados dos jobs.
        while let Ok(result) = self.results_rx.try_recv() {
            // A descoberta de bancos roda fora da fila única do app: ela não pode desligar o
            // "Running" de uma consulta que continua em andamento.
            if !matches!(result, JobResult::Databases { .. }) {
                self.running = None;
            }
            self.servers.apply(&result);
            self.query.apply(&result);

            self.status = match &result {
                JobResult::Servers(servers) => {
                    self.query.sync_servers(servers);
                    format!("{} server(s) loaded.", servers.len())
                }
                JobResult::Saved(name) => format!("Server '{name}' saved."),
                JobResult::Removed(name) => format!("Server '{name}' removed."),
                JobResult::PasswordSaved(name) => format!("Password saved for '{name}'."),
                JobResult::Tested { server, databases } => {
                    format!("'{server}' connected: {} database(s)", databases.len())
                }
                JobResult::QueryDone { total_rows, .. } => {
                    format!("Query finished: {total_rows} row(s).")
                }
                JobResult::QueryWithoutDatabases => {
                    "No accessible databases found across selected servers.".to_string()
                }
                JobResult::Schema {
                    server,
                    database,
                    schema,
                } => match schema {
                    Ok(schema) => {
                        self.schemas.insert(server, database, schema.clone());
                        format!(
                            "Schema of '{database}' loaded: {} relation(s), {} function(s).",
                            schema.tables.len(),
                            schema.functions.len()
                        )
                    }
                    Err(error) => format!("Could not load the schema: {error}"),
                },
                JobResult::Databases {
                    server,
                    databases,
                    has_password,
                } => {
                    self.reference.finish(
                        server,
                        crate::reference::Known {
                            databases: databases.clone(),
                            has_password: *has_password,
                        },
                    );

                    match databases {
                        Ok(names) => format!("'{server}' has {} database(s).", names.len()),
                        Err(error) => {
                            format!("Could not list the databases of '{server}': {error}")
                        }
                    }
                }
                JobResult::Failed(error) => error.clone(),
            };

            // Um `Save` com senha encadeia o pedido de guardar no cofre, na mesma ordem do CLI.
            if matches!(result, JobResult::Saved(_))
                && let Some(password) = self.servers.take_pending_password()
                && let JobResult::Saved(name) = &result
            {
                let name = name.clone();
                self.dispatch(Request::SetPassword { name, password }, &self.ctx.clone());
            }

            // Senha nova (ou servidor removido): a descoberta daquele servidor está velha.
            match &result {
                JobResult::PasswordSaved(name) | JobResult::Removed(name) => {
                    self.reference.forget(name);
                }
                _ => {}
            }

            // Servidor removido: o esquema carregado dele também sai da memória.
            if let JobResult::Removed(name) = &result {
                self.schemas.forget_server(name);
            }

            // Cadastro mudou: recarrega a lista para refletir o disco.
            if matches!(
                result,
                JobResult::Saved(_) | JobResult::Removed(_) | JobResult::PasswordSaved(_)
            ) {
                self.dispatch(Request::ReloadServers, &self.ctx.clone());
            }
        }
    }
}

impl eframe::App for App {
    /// Estado: consome os canais. Aqui **nada** é desenhado (é o contrato do `eframe`).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.ctx = ctx.clone();

        // `CentralPanel` (e não `Frame::central_panel`): o painel **ocupa** toda a área restante e
        // pinta o fundo dela. Com o `Frame` ele encolhia até o tamanho do conteúdo, e o resto da
        // janela ficava com o fundo cru do `eframe` — a área escura abaixo do conteúdo.
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Mustel");
                ui.separator();
                // Abas com ícone **desenhado**: emoji vinha de fonte do sistema e ficava fora da
                // linha do texto (ver `tabs`).
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Query,
                    crate::tabs::Icon::Query,
                    "Query",
                )
                .clicked()
                {
                    self.screen = Screen::Query;
                }
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Servers,
                    crate::tabs::Icon::Servers,
                    "Servers",
                )
                .clicked()
                {
                    self.screen = Screen::Servers;
                }
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Settings,
                    crate::tabs::Icon::Settings,
                    "Settings",
                )
                .clicked()
                {
                    self.screen = Screen::Settings;
                }
                ui.separator();
                ui.label(&self.status);
                if let Some(running) = &self.running {
                    ui.spinner();
                    ui.label(format!("[{}]", running.label));
                }
            });
            if let Some(error) = &self.config_error {
                ui.colored_label(egui::Color32::RED, error);
            }
            ui.separator();

            let servers = self.servers_snapshot();
            let request = match self.screen {
                Screen::Servers => self.servers.ui(ui),
                Screen::Query => {
                    // O esquema carregado vai como `Arc`: a tela guarda uma cópia para o desenho e o
                    // cache continua dono do original.
                    let schema = self
                        .schemas
                        .get(
                            self.query.reference_server(),
                            self.query.reference_database(),
                        )
                        .map(|schema| std::sync::Arc::new(schema.clone()));
                    self.query.ui(ui, &servers, schema, &self.reference)
                }
                Screen::Settings => self.settings.ui(ui),
            };

            if let Some(request) = request {
                self.dispatch(request, &ctx);
            }

            // O conteúdo fica no topo; o resto do painel é fundo, não área morta de outro widget.
            ui.allocate_space(ui.available_size());
        });
    }
}

/// Abre o cadastro do caminho padrão e roda `action`; erro de configuração vira `JobResult::Failed`.
fn with_store(action: impl FnOnce(&ConfigStore) -> JobResult) -> JobResult {
    match ConfigStore::with_default_path() {
        Ok(store) => action(&store),
        Err(error) => JobResult::Failed(mustel_core::error::CoreError::from(error).to_string()),
    }
}
