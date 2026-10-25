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

use mustel_core::config::model::GuiPrefs;
use mustel_core::config::model::SelectionQuery;
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
/// Largura da sidebar de servidores/bases.
///
/// **Fixa** (`exact_size`): com `resizable` + `default_size` o `Panel` do egui cresce para caber o
/// conteúdo e a sidebar ia parar no teto do intervalo (420) — mais de um quarto da janela. Com 400 a
/// sidebar acomoda a lista plana com o servidor em cada linha e o seletor no combo de filtro; a
/// janela mínima (880) garante ~450 para o SQL. Recolhida, a barra `▶` fica na área principal.
const SIDEBAR_WIDTH: f32 = 400.0;

/// Largura do painel da IA (fixa na v1, D23).
const AI_PANEL_WIDTH: f32 = 360.0;

/// Respiro entre a sidebar e o conteúdo à direita dela.
///
/// O painel desenha a própria linha divisória; sem o respiro o primeiro texto da área principal
/// encosta nela e a tela parece "colada".
const CONTENT_GAP: i8 = 12;

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
    /// Bancos descobertos e senha presente, por servidor (linha "Load autocomplete").
    reference: crate::reference::Reference,
    /// Aba Settings: preferências de aparência.
    settings: settings_screen::State,
    /// Queries salvas da aba **Query** (dono da lista, como `servers` é dono do cadastro).
    saved_queries: Vec<mustel_core::config::model::SavedQuery>,
    /// A sidebar de servidores/bases está aberta?
    sidebar_open: bool,
    /// O painel de conversa com a IA (aba Query) — o estado mora aqui porque um `Panel` do egui
    /// precisa ser desenhado antes do `CentralPanel`.
    ai: crate::ai_panel::AiChat,
    /// Perfis de IA configurados (lista do `gui.ai`, viva no app como as queries salvas).
    ai_profiles: Vec<mustel_core::config::model::AiProfile>,
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
        // O tema também vale no primeiro quadro: `System` (default) deixa a decisão ao SO.
        crate::theme::apply(&cc.egui_ctx, settings.theme());

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
            servers: servers_screen::State::new(servers).with_selectors(prefs.as_ref()),
            query,
            settings,
            saved_queries: prefs
                .as_ref()
                .map(|prefs| prefs.saved_queries.clone())
                .unwrap_or_default(),
            sidebar_open: true,
            ai: {
                let profiles = prefs
                    .as_ref()
                    .map(|prefs| prefs.ai.profiles.clone())
                    .unwrap_or_default();
                // A conversa volta como estava: o arquivo é de **estado**, não de configuração.
                let path = mustel_core::config::paths::ai_session_path();
                // Sem arquivo de sessão, o painel **nasce aberto** (como a sidebar esquerda);
                // com arquivo, volta como estava.
                let saved = path.as_deref().is_some_and(std::path::Path::exists);
                let session = path
                    .map(|path| mustel_core::usecases::ai_session::load(&path))
                    .unwrap_or_default();
                let mut chat = crate::ai_panel::AiChat::from_session(session, &profiles);
                chat.open = if saved { chat.open } else { true };
                chat
            },
            ai_profiles: prefs
                .as_ref()
                .map(|prefs| prefs.ai.profiles.clone())
                .unwrap_or_default(),
        })
    }

    /// Grava a conversa no arquivo de **estado** (D21/D22).
    ///
    /// Uma escrita por turno/reset/troca de contexto: o arquivo é pequeno (mensagens, sem DDL) e
    /// escrever por quadro seria desperdício.
    fn save_ai_session(&mut self) {
        self.ai.session.panel_open = self.ai.open;
        if let Some(path) = mustel_core::config::paths::ai_session_path()
            && let Err(error) = mustel_core::usecases::ai_session::save(&path, &self.ai.session)
        {
            self.status = format!("Could not save the AI session: {error}");
        }
    }

    /// O cadastro atual (as duas telas leem dele).
    fn servers_snapshot(&self) -> Vec<ServerConfigEntry> {
        self.servers.servers().to_vec()
    }

    /// A query de seleção salva com esse nome (sem diferenciar maiúsculas).
    fn selection_query(&self, name: &str) -> Option<SelectionQuery> {
        self.servers
            .selectors()
            .selections()
            .iter()
            .find(|query| query.name.eq_ignore_ascii_case(name))
            .cloned()
    }

    /// A seção `gui` inteira, composta das duas telas donas de pedaços dela.
    ///
    /// Nenhuma tela conhece a seção toda: Settings tem aparência e paginação, Connections tem os
    /// seletores. Gravar o que uma delas conhece apagaria o resto — foi por isso que o cadastro de
    /// seletores deixou de devolver `SaveGuiPrefs` e passou a devolver `SaveSelections`.
    fn gui_prefs(&self) -> GuiPrefs {
        crate::screens::compose_gui_prefs(
            self.settings.prefs(),
            self.servers.selectors().selections(),
            &self.saved_queries,
        )
    }

    /// O que a sidebar desenha, conforme a aba.
    ///
    /// Na aba **Query** é a escolha de bases (query salva, busca, servidores e checkboxes); na aba
    /// **Servers** é a lista do cadastro. As duas usam o mesmo painel esquerdo.
    fn sidebar_request(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
    ) -> Option<Request> {
        match self.screen {
            Screen::Query => {
                let selections = self.servers.selectors().selections().to_vec();
                self.query.sidebar_ui(ui, servers, &selections)
            }
            Screen::Servers => self.servers.sidebar_ui(ui),
            Screen::Settings => None,
        }
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
            // A lista de **queries salvas** mudou: compõe a seção `gui` inteira e grava.
            Request::SaveSavedQueries(queries) => {
                self.saved_queries = queries;
                let prefs = self.gui_prefs();
                match ConfigStore::with_default_path() {
                    Ok(store) => match gui_prefs::save(&store, Some(prefs)) {
                        Ok(()) => {
                            self.status = "Saved queries saved.".to_string();
                            ctx.request_repaint();
                        }
                        Err(error) => self.status = error.to_string(),
                    },
                    Err(error) => self.status = error.to_string(),
                }
            }
            // O cadastro de seletores mudou: compõe a seção `gui` inteira e grava pelo mesmo
            // caminho da aparência.
            Request::SaveSelections => {
                let prefs = self.gui_prefs();
                match ConfigStore::with_default_path() {
                    Ok(store) => match gui_prefs::save(&store, Some(prefs)) {
                        Ok(()) => {
                            let selections = self.servers.selectors().selections().to_vec();
                            self.query.retain_selection_queries(&selections);
                            self.status = "Selectors saved.".to_string();
                            ctx.request_repaint();
                        }
                        Err(error) => self.status = error.to_string(),
                    },
                    Err(error) => self.status = error.to_string(),
                }
            }
            // Gravar vai para o núcleo (mesmo caminho do cadastro).
            Request::SaveGuiPrefs(prefs) => {
                // Aplica **e** grava: é o commit da aba Settings. Aplicar aqui é o que faz trocar de
                // fonte surtir efeito imediato (a prévia ao vivo é do `ApplyGuiPrefs`).
                let chosen = crate::fonts::Chosen::from_prefs(Some(&prefs));
                let problems = crate::fonts::apply(ctx, &chosen);
                crate::theme::apply(ctx, prefs.theme);
                self.settings.set_warning(problems.first().cloned());
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                    crate::fonts::min_window_size(chosen.scale),
                ));

                match ConfigStore::with_default_path() {
                    Ok(store) => match gui_prefs::save(&store, Some(prefs)) {
                        Ok(()) => {
                            // A query em uso pode ter sido removida (ou renomeada) na aba Settings:
                            // o combo da aba Query não pode continuar apontando para o nome antigo.
                            let selections = self.servers.selectors().selections().to_vec();
                            self.query.retain_selection_queries(&selections);
                            self.status = "Appearance saved.".to_string();
                            ctx.request_repaint();
                        }
                        Err(error) => self.status = error.to_string(),
                    },
                    Err(error) => self.status = error.to_string(),
                }
            }
            // O `Test` da aba Settings roda o **mesmo** caminho do `⟳ Apply`: conexão, `load` e
            // `validate`. O que volta são números; a senha e a connection string não aparecem (D12).
            Request::TestSelection(query) => {
                let servers = self.servers_snapshot();
                let name = query.name.clone();
                self.status = format!("Testing selection query '{name}'...");
                let results = self.results.clone();
                let context = ctx.clone();
                let (token, _handle) = self.runtime.spawn(move |_token| async move {
                    let outcome = match query::selection(&query, &servers).await {
                        Ok((loaded, selection)) => Ok(query::summarise(&loaded, &selection)),
                        Err(error) => Err(error.to_string()),
                    };
                    let _ = results.send(JobResult::SelectionTested { name, outcome });
                    context.request_repaint();
                });
                let _ = token;
            }
            Request::CancelJob => self.cancel(),
            // Perfis de IA: a tela manda a lista inteira; o app compõe a seção `gui` e grava.
            Request::SaveAiProfiles {
                profiles,
                active,
                key,
            } => {
                self.ai_profiles = profiles.clone();
                self.settings.set_ai(profiles.clone(), active.clone());
                self.ai.selected_profile = active;
                let prefs = self.gui_prefs();
                let results = self.results.clone();
                self.spawn("save ai profiles", ctx, move |_token| async move {
                    // A chave vai **antes** de gravar a lista: se o cofre falhar, o erro aparece e o
                    // perfil já está na lista da tela (o dono tenta de novo digitando a chave).
                    let stored = match &key {
                        Some((profile, secret)) => mustel_core::usecases::ai::set_ai_key(
                            &mustel_core::credentials::SystemKeyring::with_service(
                                mustel_core::usecases::ai::AI_SERVICE_NAME,
                            ),
                            profile,
                            secret,
                        ),
                        None => Ok(()),
                    };
                    let result = match stored {
                        Ok(()) => match ConfigStore::with_default_path() {
                            Ok(store) => match gui_prefs::save(&store, Some(prefs)) {
                                Ok(()) => JobResult::AiProfilesSaved,
                                Err(error) => JobResult::Failed(error.to_string()),
                            },
                            Err(error) => JobResult::Failed(error.to_string()),
                        },
                        Err(error) => JobResult::Failed(error.to_string()),
                    };
                    let _ = results.send(result);
                });
            }
            Request::RemoveAiKey { profile } => {
                let results = self.results.clone();
                self.spawn("remove ai key", ctx, move |_token| async move {
                    let result = mustel_core::usecases::ai::remove_ai_key(
                        &mustel_core::credentials::SystemKeyring::with_service(
                            mustel_core::usecases::ai::AI_SERVICE_NAME,
                        ),
                        &profile,
                    )
                    .map(|_| JobResult::AiProfilesSaved)
                    .unwrap_or_else(|error| JobResult::Failed(error.to_string()));
                    let _ = results.send(result);
                });
            }
            Request::TestAiProfile { profile } => {
                let results = self.results.clone();
                let entry = self
                    .ai_profiles
                    .iter()
                    .find(|candidate| candidate.id == profile)
                    .cloned();
                self.spawn("test ai profile", ctx, move |_token| async move {
                    let result = match entry {
                        Some(entry) => {
                            match mustel_core::usecases::ai::ai_key(
                                &mustel_core::credentials::SystemKeyring::with_service(
                                    mustel_core::usecases::ai::AI_SERVICE_NAME,
                                ),
                                &entry.id,
                            ) {
                                Ok(Some(key)) => {
                                    match mustel_core::ai_transport::ReqwestTransport::new() {
                                        Ok(transport) => {
                                            let messages =
                                                vec![mustel_core::usecases::ai::ChatMessage::user(
                                                    "Reply with SELECT 1;",
                                                )];
                                            mustel_core::usecases::ai::generate(
                                                &transport,
                                                &entry,
                                                &key,
                                                "CREATE TABLE ping (id integer);",
                                                &messages,
                                            )
                                            .await
                                            .map(|sql| format!("OK: {sql}"))
                                            .map_err(|error| error.to_string())
                                        }
                                        Err(error) => Err(error.to_string()),
                                    }
                                }
                                Ok(None) => Err("No key stored for this profile.".to_string()),
                                Err(error) => Err(error.to_string()),
                            }
                        }
                        None => Err("That profile no longer exists.".to_string()),
                    };
                    let id = profile;
                    let _ = results.send(JobResult::AiTested {
                        profile: id,
                        result,
                    });
                });
            }
            Request::OpenSettings => {
                self.screen = Screen::Settings;
                ctx.request_repaint();
            }
            // O `Insert in editor` **substitui** o texto (D13) e fica na tela de query.
            Request::InsertEditorText(text) => {
                self.query.replace_editor_text(text);
                ctx.request_repaint();
            }
            // O painel da IA: abrir/fechar é estado de janela, não trabalho de núcleo.
            Request::ToggleAiPanel => {
                self.ai.open = !self.ai.open;
                if self.ai.open && self.ai.ddl.is_none() && !self.ai.selected_database.is_empty() {
                    // Abriu com banco escolhido: já pede o DDL (cache primeiro).
                    self.dispatch(
                        Request::LoadDdl {
                            server: self.ai.selected_server.clone(),
                            database: self.ai.selected_database.clone(),
                            refresh: false,
                        },
                        ctx,
                    );
                }
                self.save_ai_session();
                ctx.request_repaint();
            }
            Request::LoadDdl {
                server,
                database,
                refresh,
            } => {
                // Mesma forma do `LoadSchema`: sem senha no cofre, avisa e não tenta.
                let Some(entry) = self
                    .servers_snapshot()
                    .into_iter()
                    .find(|candidate| candidate.name == server)
                else {
                    self.status = format!("Server '{server}' not found.");
                    return;
                };
                let password = match query::stored_password(&entry.name) {
                    Ok(password) => password,
                    Err(error) => {
                        self.ai.apply_error(error.to_string());
                        return;
                    }
                };

                let results = self.results.clone();
                self.spawn("load schema", ctx, move |token| async move {
                    let result = query::ddl(&entry, &password, &database, refresh, &token)
                        .await
                        .map_err(|error| error.to_string());
                    let _ = results.send(JobResult::Ddl { result });
                });
            }
            Request::GenerateSql {
                profile,
                ddl,
                messages,
            } => {
                let results = self.results.clone();
                let profile = self
                    .ai_profiles
                    .iter()
                    .find(|candidate| candidate.id == profile)
                    .cloned();
                self.spawn("generate sql", ctx, move |_token| async move {
                    let result = match profile {
                        Some(profile) => match mustel_core::usecases::ai::ai_key(
                            &mustel_core::credentials::SystemKeyring::with_service(
                                mustel_core::usecases::ai::AI_SERVICE_NAME,
                            ),
                            &profile.id,
                        ) {
                            Ok(Some(key)) => {
                                match mustel_core::ai_transport::ReqwestTransport::new() {
                                    Ok(transport) => mustel_core::usecases::ai::generate(
                                        &transport, &profile, &key, &ddl, &messages,
                                    )
                                    .await
                                    .map_err(|error| error.to_string()),
                                    Err(error) => Err(error.to_string()),
                                }
                            }
                            Ok(None) => Err(format!(
                                "No key stored for '{}': set it in Settings › AI",
                                profile.name
                            )),
                            Err(error) => Err(error.to_string()),
                        },
                        None => Err("That AI profile no longer exists.".to_string()),
                    };
                    let _ = results.send(JobResult::SqlGenerated { result });
                });
            }
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
                let registry = self.servers_snapshot();
                let targets: Vec<ServerConfigEntry> = match &server {
                    Some(name) => registry
                        .into_iter()
                        .filter(|candidate| &candidate.name == name)
                        .collect(),
                    None => registry,
                };
                if targets.is_empty() {
                    self.status = match &server {
                        Some(name) => format!("Server '{name}' not found."),
                        None => "No server registered.".to_string(),
                    };
                    return;
                }

                // Fora do `spawn`: o `spawn` tem fila única ("Already running") e a descoberta não
                // pode esperar uma consulta longa terminar para o combo sair do "nenhum banco".
                //
                // Em melhor esforço: cada servidor emite o **seu** resultado, então um fora do ar
                // não impede os outros de aparecerem (R6).
                for entry in &targets {
                    self.reference.mark_pending(&entry.name);
                }
                self.status = match &server {
                    Some(name) => format!("Discovering databases on '{name}'..."),
                    None => format!("Discovering databases on {} server(s)...", targets.len()),
                };

                for entry in targets {
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
            }
            Request::ApplySelection { query, scope } => {
                let registry = self.servers_snapshot();
                let Some(saved) = self.selection_query(&query) else {
                    self.status = format!("Selection query '{query}' not found.");
                    return;
                };

                self.status = format!("Applying selection query '{query}'...");
                self.query.begin_apply(&query);
                let results = self.results.clone();
                let context = ctx.clone();
                let (token, _handle) = self.runtime.spawn(move |_token| async move {
                    let result = match query::selection(&saved, &registry).await {
                        Ok((loaded, selection)) => JobResult::Selection {
                            query: saved.name,
                            scope,
                            selection,
                            ignored_rows: loaded.ignored_rows,
                        },
                        Err(error) => JobResult::SelectionFailed {
                            query: saved.name,
                            error: error.to_string(),
                        },
                    };
                    let _ = results.send(result);
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
                targets,
                sql,
                output_directory,
            } => {
                // A entrada é montada fora do runtime: se faltar senha, a tela avisa na hora.
                //
                // O servidor vem do **cadastro** e as bases são as marcadas na tela (D15): a conexão
                // continua sendo a do cadastro, e o que a seleção decide é só *quais bases* rodam.
                let registry = self.servers_snapshot();
                let mut runs = Vec::with_capacity(targets.len());
                for (name, databases) in &targets {
                    let Some(server) = registry.iter().find(|server| &server.name == name) else {
                        self.status = format!("Server '{name}' not found.");
                        return;
                    };
                    let password = match query::stored_password(&server.name) {
                        Ok(password) => password,
                        Err(error) => {
                            self.status = error.to_string();
                            return;
                        }
                    };
                    match query::server_run_marked(server, &password, databases) {
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
            self.settings.apply(&result);

            // O DDL e a resposta da IA são do painel: aplicam no estado dele e gravam a sessão.
            match &result {
                JobResult::Ddl { result } => match result {
                    Ok(ddl) => {
                        self.ai.apply_ddl(ddl.clone());
                        self.save_ai_session();
                    }
                    Err(error) => self.ai.apply_error(error.clone()),
                },
                JobResult::SqlGenerated { result } => match result {
                    Ok(sql) => {
                        self.ai.apply_reply(sql.clone());
                        self.save_ai_session();
                    }
                    Err(error) => self.ai.apply_error(error.clone()),
                },
                _ => {}
            }

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
                JobResult::Ddl { .. } | JobResult::SqlGenerated { .. } => self.status.clone(),
                JobResult::AiProfilesSaved => "AI profiles saved.".to_string(),
                JobResult::AiTested { result, .. } => match result {
                    Ok(message) => format!("AI profile works: {message}"),
                    Err(error) => format!("AI profile failed: {error}"),
                },
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
                    // A referência do autocomplete lista onde a query de seleção **roda**: só o que
                    // dá para conectar (template e `datallowconn = false` ficam de fora). A
                    // sidebar usa o catálogo inteiro, por outro caminho.
                    self.reference.finish(
                        server,
                        crate::reference::Known {
                            databases: databases
                                .as_ref()
                                .map(|catalog| crate::query::usable_names(catalog))
                                .map_err(Clone::clone),
                            has_password: *has_password,
                        },
                    );

                    match databases {
                        Ok(catalog) => {
                            format!("'{server}' has {} database(s).", catalog.len())
                        }
                        Err(error) => {
                            format!("Could not list the databases of '{server}': {error}")
                        }
                    }
                }
                JobResult::SelectionTested { name, outcome } => match outcome {
                    Ok(outcome) => format!("Selection query '{name}': {}", outcome.summary()),
                    Err(error) => format!("Selection query '{name}' failed: {error}"),
                },
                JobResult::Selection {
                    query,
                    selection,
                    ignored_rows,
                    ..
                } => {
                    let outcome = query::summarise(
                        &mustel_core::usecases::custom_databases::LoadedSelection {
                            rows: Vec::new(),
                            ignored_rows: *ignored_rows,
                        },
                        selection,
                    );
                    format!("'{query}' applied: {}", outcome.summary())
                }
                JobResult::SelectionFailed { query, error } => {
                    format!("Could not apply selection query '{query}': {error}")
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
            // Um pedido por quadro: a barra (handle do painel), as sidebars e a tela escrevem nele.
            let mut request = None;

            ui.horizontal(|ui| {
                ui.heading("Mustel");
                ui.separator();
                // Abas com ícone da **fonte embutida**: o emoji vinha de fonte do sistema e ficava
                // fora da linha do texto (ver `tabs`).
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Query,
                    crate::icons::Icon::Magnifier,
                    "Query",
                )
                .clicked()
                {
                    self.screen = Screen::Query;
                }
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Servers,
                    crate::icons::Icon::Server,
                    "Connections",
                )
                .clicked()
                {
                    self.screen = Screen::Servers;
                    // Sempre começa no cadastro de servidores; o rascunho de um formulário de
                    // seletor sobrevive à troca de aba.
                    self.servers.reset_context();
                }
                if crate::tabs::tab(
                    ui,
                    self.screen == Screen::Settings,
                    crate::icons::Icon::Settings,
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
                ui.colored_label(crate::theme::danger(ui), error);
            }
            ui.separator();

            let servers = self.servers_snapshot();

            // Sidebar: **servidores e escolha de bases** moram à esquerda; a área principal fica com
            // o que é da execução (SQL, saída, resultados). A aba Settings não tem sidebar — é um
            // formulário só. `show_collapsible` cuida da animação e deixa uma alça para reabrir.
            if matches!(self.screen, Screen::Query | Screen::Servers) {
                // `show_collapsible` anima o painel e deixa uma alça na borda quando recolhido. Ele
                // escreve em `expanded` só quando o **arrasto da alça** muda o estado; o botão
                // `◀`/`▶` alterna `sidebar_open` direto. Por isso o valor do fechamento é descartado
                // e o do campo é que vale — escrever o do fechamento desfazia o clique.
                if self.sidebar_open {
                    let mut toggled = false;
                    let sidebar = egui::Panel::left("app_sidebar")
                        .exact_size(SIDEBAR_WIDTH)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Seta da fonte de ícones embutida: `«`/`»` saíam como caixinha
                                // na fonte embutida e o botão parecia um ícone quebrado.
                                if crate::icons::button(ui, crate::icons::Icon::ChevronLeft)
                                    .on_hover_text("Hide the servers panel")
                                    .clicked()
                                {
                                    toggled = true;
                                }
                                // O contador fica no topo: é o número que decide se dá para rodar.
                                // Na aba Servers ele não significa nada (a lista é o cadastro).
                                if self.screen == Screen::Query {
                                    ui.weak(format!("{} selected", self.query.marked_count()));
                                }
                            });
                            self.sidebar_request(ui, &servers)
                        });

                    if toggled {
                        self.sidebar_open = false;
                    }
                    request = sidebar.inner.or(request);
                }
                // Recolhida, a alça que reabre é desenhada **dentro** da área de conteúdo, na
                // mesma linha da alça do painel da IA (`reopen_handles`): uma linha por alça
                // deixava a de baixo numa banda vazia e ela lia como desalinhada.
            }

            // **Painel da IA** (aba Query): irmão do esquerdo, criado antes do conteúdo central.
            // Fechado por padrão e sem arrastar largura (D11/D23).
            if self.screen == Screen::Query && self.ai.open {
                let servers = self.servers_snapshot();
                let profiles = self.ai_profiles.clone();
                // Bancos do servidor escolhido no painel: os descobertos, ou os do cadastro.
                let configured: Vec<String> = servers
                    .iter()
                    .find(|entry| entry.name == self.ai.selected_server)
                    .map(|entry| entry.databases.clone())
                    .unwrap_or_default();
                let databases = self
                    .reference
                    .choices(&self.ai.selected_server, &configured);
                let has_text = self.query.editor_has_text();
                let other_job = self.running.as_ref().map(|running| running.label.clone());

                let panel = egui::Panel::right("ai_panel")
                    .exact_size(AI_PANEL_WIDTH)
                    .show(ui, |ui| {
                        crate::ai_panel::ui(
                            ui,
                            &mut self.ai,
                            &servers,
                            &profiles,
                            &databases,
                            has_text,
                            other_job.as_deref(),
                        )
                    });

                if let Some(ai_request) = panel.inner {
                    request = Some(ai_request).or(request);
                }
            }

            // **Padding entre a sidebar e o conteúdo.** Sem ele o texto da área principal encosta na
            // linha divisória do painel. Só existe quando a sidebar está visível: em Settings, ou com
            // a sidebar recolhida na alça `›`, o conteúdo usa a largura toda.
            let gap = if self.sidebar_open && matches!(self.screen, Screen::Query | Screen::Servers)
            {
                CONTENT_GAP
            } else {
                0
            };
            let main = egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: gap,
                    ..egui::Margin::ZERO
                })
                .show(ui, |ui| {
                    // Recolhidos, os painéis **não são painéis**: viram alças no topo da área de
                    // conteúdo — `›` à esquerda (sidebar) e `✦` à direita (IA). Um `Panel` aqui
                    // desenharia a divisória de altura inteira.
                    let sidebar_closed = !self.sidebar_open
                        && matches!(self.screen, Screen::Query | Screen::Servers);
                    let ai_closed = self.screen == Screen::Query && !self.ai.open;
                    let marked = if self.screen == Screen::Query {
                        Some(self.query.marked_count())
                    } else {
                        None
                    };
                    let (reopen_sidebar, reopen_ai) =
                        reopen_handles(ui, sidebar_closed, ai_closed, marked);
                    if reopen_sidebar {
                        self.sidebar_open = true;
                    }
                    if reopen_ai {
                        request = Some(Request::ToggleAiPanel);
                    }
                    match self.screen {
                        Screen::Servers => self.servers.ui(ui),
                        Screen::Query => {
                            // O esquema carregado vai como `Arc`: a tela guarda uma cópia para o desenho
                            // e o cache continua dono do original.
                            let schema = self
                                .schemas
                                .get(
                                    self.query.reference_server(),
                                    self.query.reference_database(),
                                )
                                .map(|schema| std::sync::Arc::new(schema.clone()));
                            // A paginação da lista vem da seção `gui` — o mesmo objeto que a aba Settings
                            // edita, então mexer lá vale no quadro seguinte.
                            let page_size = crate::screens::query::page_size(&self.gui_prefs());
                            self.query.set_page_size(page_size);
                            // O dono da lista é o app: a tela só desenha e devolve o pedido.
                            self.query.set_saved_queries(self.saved_queries.clone());
                            self.query.ui(ui, &servers, schema, &self.reference)
                        }
                        Screen::Settings => self.settings.ui(ui),
                    }
                });
            request = main.inner.or(request);

            if let Some(request) = request {
                self.dispatch(request, &ctx);
            }

            // O conteúdo fica no topo; o resto do painel é fundo, não área morta de outro widget.
            ui.allocate_space(ui.available_size());
        });
    }
}

/// Alças que reabrem os painéis recolhidos, na **mesma linha** do topo da área de conteúdo.
///
/// Devolve `(reabrir a sidebar, reabrir o painel da IA)`; a linha só existe quando há alça para
/// desenhar. As duas dividem **uma** linha, uma em cada ponta: com uma linha por alça, a de baixo
/// caía numa banda vazia (nada à esquerda dela) e lia como desalinhada — foi o que apareceu com os
/// dois painéis recolhidos.
///
/// `marked` é a contagem que acompanha a alça da sidebar: ela só aparece com a sidebar recolhida,
/// porque aberta o número já está no topo do painel dela (desenhá-lo aqui o duplicava).
///
/// `Align::CENTER` vale aqui porque o `horizontal` dá à linha a altura de um controle. Sozinho no
/// conteúdo (sem a linha), o mesmo `Center` levava o botão para o meio da altura livre: y 693 no
/// diagnóstico, contra y 43 quando ele é ancorado no topo.
fn reopen_handles(
    ui: &mut egui::Ui,
    sidebar_closed: bool,
    ai_closed: bool,
    marked: Option<usize>,
) -> (bool, bool) {
    let mut reopen_sidebar = false;
    let mut reopen_ai = false;

    // Sem alça, sem linha: uma linha vazia no topo empurraria o conteúdo para baixo sem dizer nada.
    if sidebar_closed || ai_closed {
        ui.horizontal(|ui| {
            if sidebar_closed
                && crate::icons::button(ui, crate::icons::Icon::ChevronRight)
                    .on_hover_text("Show the servers panel")
                    .clicked()
            {
                reopen_sidebar = true;
            }
            if sidebar_closed && let Some(marked) = marked {
                ui.weak(format!("{marked} selected"));
            }
            if ai_closed {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::button(ui, crate::icons::Icon::Sparkles)
                        .on_hover_text("Show the AI panel")
                        .clicked()
                    {
                        reopen_ai = true;
                    }
                });
            }
        });
    }

    (reopen_sidebar, reopen_ai)
}

/// Abre o cadastro do caminho padrão e roda `action`; erro de configuração vira `JobResult::Failed`.
fn with_store(action: impl FnOnce(&ConfigStore) -> JobResult) -> JobResult {
    match ConfigStore::with_default_path() {
        Ok(store) => action(&store),
        Err(error) => JobResult::Failed(mustel_core::error::CoreError::from(error).to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O que a linha de alças pintou: textos com a posição, altura consumida e o que foi clicado.
    struct Painted {
        texts: Vec<(String, egui::Pos2)>,
        height: f32,
        row: f32,
        clicked: (bool, bool),
    }

    /// Desenha `reopen_handles` numa janela de 800 px de largura, com a fonte de ícones de verdade.
    ///
    /// Dois quadros: o primeiro monta o atlas da fonte dos ícones, e só o segundo tem a métrica
    /// definitiva dos glifos.
    fn render_handles(sidebar_closed: bool, ai_closed: bool, marked: Option<usize>) -> Painted {
        let ctx = egui::Context::default();
        let mut definitions = egui::FontDefinitions::default();
        crate::icons::install(&mut definitions);
        ctx.set_fonts(definitions);

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };

        let mut painted = Painted {
            texts: Vec::new(),
            height: 0.0,
            row: 0.0,
            clicked: (false, false),
        };
        for _ in 0..2 {
            let mut output = ctx.run_ui(input.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let top = ui.cursor().top();
                    painted.row = ui.spacing().interact_size.y;
                    painted.clicked = reopen_handles(ui, sidebar_closed, ai_closed, marked);
                    // Avanço do cursor, não `min_rect`: o retângulo mínimo do painel é a janela
                    // inteira e não diz quanta altura a linha comeu.
                    painted.height = ui.cursor().top() - top;
                });
            });
            painted.texts.clear();
            collect_text(&output.shapes, &mut painted.texts);
            output.textures_delta.clear();
        }
        painted
    }

    /// A posição do único glifo do ícone pedido.
    fn glyph_at(painted: &Painted, icon: crate::icons::Icon) -> egui::Pos2 {
        let needle = icon.glyph().to_string();
        let found: Vec<egui::Pos2> = painted
            .texts
            .iter()
            .filter(|(text, _)| *text == needle)
            .map(|(_, pos)| *pos)
            .collect();
        assert_eq!(
            found.len(),
            1,
            "um só {icon:?} na linha: {:#?}",
            painted.texts
        );
        found[0]
    }

    /// As duas alças dos painéis recolhidos dividem **uma linha**: o `›` da sidebar à esquerda e o
    /// `✦` da IA na ponta direita.
    ///
    /// A regressão: com uma alça por linha, o `✦` caía numa banda vazia abaixo do `›` (nada à
    /// esquerda dele) e lia como desalinhado — foi assim com os dois painéis recolhidos.
    /// CONTRACT: IF-247
    #[test]
    fn both_reopen_handles_share_the_top_row() {
        let painted = render_handles(true, true, Some(3));

        let chevron = glyph_at(&painted, crate::icons::Icon::ChevronRight);
        let sparkle = glyph_at(&painted, crate::icons::Icon::Sparkles);

        assert!(
            (chevron.y - sparkle.y).abs() <= 2.0,
            "as duas alças têm de estar na mesma linha: {chevron:?} vs {sparkle:?}"
        );
        assert!(chevron.x < 40.0, "o `›` abre a linha: {chevron:?}");
        assert!(
            sparkle.x > 700.0,
            "o `✦` fecha a linha, na ponta direita: {sparkle:?}"
        );
        assert!(
            painted.height < 2.0 * painted.row,
            "a linha das alças não ocupa duas: {} (linha = {})",
            painted.height,
            painted.row
        );
        assert_eq!(painted.clicked, (false, false), "nada foi clicado");
        assert!(
            painted.texts.iter().any(|(text, _)| text == "3 selected"),
            "a contagem acompanha a alça da sidebar recolhida: {:#?}",
            painted.texts
        );
    }

    /// Recolhida só a sidebar, a alça da IA continua na mesma linha do `›`.
    ///
    /// É o estado em que o desalinhamento não aparecia (o `›` morava fora da área de conteúdo);
    /// agora ele não pode mudar de linha só porque a sidebar fechou. E a contagem **não** se
    /// repete aqui: com a sidebar aberta, `0 selected` já está no topo do painel dela.
    #[test]
    fn the_ai_handle_keeps_the_row_when_the_sidebar_is_open() {
        let painted = render_handles(false, true, Some(0));

        let sparkle = glyph_at(&painted, crate::icons::Icon::Sparkles);
        assert!(
            sparkle.x > 700.0,
            "o `✦` fica na ponta direita da linha: {sparkle:?}"
        );
        assert!(
            painted.height < 2.0 * painted.row,
            "uma linha só, sem a alça da sidebar: {} (linha = {})",
            painted.height,
            painted.row
        );
        assert!(
            !painted
                .texts
                .iter()
                .any(|(text, _)| text.contains("selected")),
            "a contagem não se repete com a sidebar aberta: {:#?}",
            painted.texts
        );
    }

    /// Sem painel recolhido não há alça nem **linha**: o conteúdo começa no topo.
    #[test]
    fn no_collapsed_panel_means_no_row() {
        let painted = render_handles(false, false, None);

        let chevron = crate::icons::Icon::ChevronRight.glyph().to_string();
        let sparkle = crate::icons::Icon::Sparkles.glyph().to_string();
        assert!(
            !painted
                .texts
                .iter()
                .any(|(text, _)| *text == chevron || *text == sparkle),
            "nenhuma alça desenhada: {:#?}",
            painted.texts
        );
        assert!(
            painted.height < painted.row,
            "nenhuma linha consumida: {} (linha = {})",
            painted.height,
            painted.row
        );
    }

    /// Junta `(texto, posição)` de todo `Shape::Text`, descendo nos grupos.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<(String, egui::Pos2)>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text) => out.push((text.galley.text().to_string(), text.pos)),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }

        for clipped in shapes {
            walk(&clipped.shape, out);
        }
    }
}
