//! Tela de `query run` (T42): SQL, escolha dos servidores, execução e resultados.
//!
//! A consulta é a mesma do CLI: monta uma entrada por servidor escolhido ([`crate::query`]) e quem
//! executa é [`mustel_core::usecases::query_run`]. O que a tela faz com o resultado:
//!
//! - mostra o **progresso** que o `ChannelSink` entrega (um banco por linha, com o erro quando falha);
//! - lê o **consolidado** e mostra as linhas numa tabela **virtualizada** ([`egui::ScrollArea::show_rows`]
//!   desenha só as visíveis) — o CSV é o artefato que o núcleo escreveu, e [`parse_records`] já sabe
//!   relê-lo com as mesmas regras de aspas e BOM;
//! - o **export** é o próprio `query run`: os arquivos estão no diretório escolhido, e a tela mostra
//!   os caminhos;
//! - a leitura é limitada a [`MAX_DISPLAY_ROWS`] linhas: acima disto a tela avisa e o arquivo (que
//!   continua completo) é o que vale.
//!
//! [`parse_records`]: mustel_core::query::csv_writer::parse_records

use std::path::PathBuf;

use mustel_core::config::ServerConfigEntry;
use mustel_core::query::analyzer::{self, QueryType};
use mustel_core::query::csv_writer::parse_records;

use crate::request::{JobResult, Request};
use crate::smooth_scroll::SmoothScroll;
use crate::{desktop, picker::Picker, sql_completion, sql_editor};
use mustel_core::config::model::SelectionQuery;

/// Teto de linhas carregadas para a tabela. O arquivo não é truncado — só a leitura da tela.
pub const MAX_DISPLAY_ROWS: usize = 20_000;

/// Altura mínima reservada para a tabela quando a janela é pequena.
const MIN_TABLE_HEIGHT: f32 = 120.0;

/// Altura mínima do formulário rolável.
///
/// Abaixo disto a janela é considerada pequena demais: o formulário fica com esta altura e rola,
/// em vez de encolher até desaparecer.
const MIN_FORM_HEIGHT: f32 = 160.0;

/// Uma linha do progresso (um banco que terminou).
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressRow {
    /// Servidor.
    pub server: String,
    /// Banco.
    pub database: String,
    /// Linhas trazidas (0 no erro).
    pub rows: u64,
    /// Duração em segundos, para exibir.
    pub seconds: f64,
    /// Erro, vazio no sucesso.
    pub error: String,
}

/// Como a tela está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Nada rodando ainda.
    Idle,
    /// A consulta está no runtime de background.
    Running,
    /// Terminou.
    Done,
}

/// O que a confirmação de consulta destrutiva precisa mostrar.
///
/// Nasce quando a tela detecta SQL destrutivo e some quando o usuário confirma ou cancela.
struct Confirmation {
    /// Tipo detectado, para o título (`DELETE`, `UPDATE`, ...).
    description: String,
    /// Servidores que a consulta atinge.
    servers: usize,
    /// Bancos atingidos (só o número: descobrir os reais exigiria conectar).
    databases: usize,
    /// Trecho da consulta, para o usuário reconhecer o que ia rodar.
    preview: String,
    /// Pedido a executar se o usuário confirmar.
    request: Request,
}

/// Estado da tela de consulta.
pub struct State {
    /// SQL da caixa de texto.
    pub sql: String,
    /// Diretório base da saída (vira `<base>/<timestamp>/` no núcleo).
    pub output_directory: String,
    /// Nomes marcados na lista de servidores.
    selected: Vec<String>,
    /// Quais **bases** de quais servidores rodam (R1, R2, R7).
    selection: crate::selection::SelectionState,
    /// Cadastro como a tela o conhece (a descoberta precisa dos `excludePatterns` de cada servidor).
    registry: Vec<ServerConfigEntry>,
    /// Query de seleção escolhida no combo (`None` = seleção à mão).
    chosen_query: Option<String>,
    /// Servidor escolhido como **referência do autocomplete**.
    reference_server: String,
    /// Banco de referência do autocomplete.
    reference_database: String,
    /// Último esquema que o app confirmou ter carregado (para o rótulo).
    reference_loaded: Option<(String, String)>,
    /// Cópia do esquema de referência, para o motor de sugestão (evita emprestar o cache no desenho).
    reference_schema: Option<std::sync::Arc<mustel_core::usecases::schema::Schema>>,
    /// Fase atual.
    phase: Phase,
    /// Linhas de progresso, na ordem em que os bancos terminaram.
    progress: Vec<ProgressRow>,
    /// Cabeçalho do resultado (a primeira linha do consolidado).
    header: Vec<String>,
    /// Linhas do resultado, sem o cabeçalho.
    rows: Vec<Vec<String>>,
    /// Quantas linhas o arquivo tinha (pode ser mais que `rows`).
    total_rows: usize,
    /// Caminhos dos arquivos que o núcleo escreveu.
    files: Vec<(String, String)>,
    /// Resumo do núcleo.
    summary: Option<String>,
    /// Erro do último pedido.
    error: Option<String>,
    /// Consulta destrutiva esperando confirmação.
    confirmation: Option<Confirmation>,
    /// Seletor de pastas da caixa de saída (modal do próprio app).
    picker: Picker,
    /// Animação de rolagem do formulário (o botão "ir ao fim").
    form_scroll: SmoothScroll,
    /// Memória do popup entre quadros (pedido do `Ctrl+Space` e pausa depois de um aceite).
    completion_trigger: sql_completion::Trigger,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sql: "SELECT 1 AS valor".to_string(),
            output_directory: String::new(),
            selected: Vec::new(),
            selection: crate::selection::SelectionState::default(),
            registry: Vec::new(),
            chosen_query: None,
            reference_server: String::new(),
            reference_database: String::new(),
            reference_loaded: None,
            reference_schema: None,
            phase: Phase::Idle,
            progress: Vec::new(),
            header: Vec::new(),
            rows: Vec::new(),
            total_rows: 0,
            files: Vec::new(),
            summary: None,
            error: None,
            confirmation: None,
            picker: Picker::default(),
            form_scroll: SmoothScroll::default(),
            completion_trigger: sql_completion::Trigger::default(),
        }
    }
}

/// Distância do fim a partir da qual o botão "ir ao fim" some.
///
/// Quando o usuário já está no fim não há para onde ir — e o botão pararia em cima do
/// `Run query`, que é justamente o que ele quer clicar depois. Rolando de volta, ele reaparece.
const SCROLL_BUTTON_AT_END_PX: f32 = 8.0;

/// O botão "ir ao fim" deve ser desenhado?
///
/// Sim quando o conteúdo transborda **e** ainda há o que rolar. Função pura: a decisão é testável
/// sem janela, e é ela que evita o botão cobrir o `Run query` no fim da página.
pub fn should_show_scroll_button(content_height: f32, viewport_height: f32, offset: f32) -> bool {
    let max_offset = (content_height - viewport_height).max(0.0);
    max_offset > 1.0 && (max_offset - offset) > SCROLL_BUTTON_AT_END_PX
}

/// Altura reservada para a tabela de resultados, quando há resultados.
///
/// A tela tem dois consumidores de altura que crescem sozinhos: o formulário (o SQL aumenta) e a
/// tabela (as linhas aumentam). Se os dois crescerem sem limite, um deles sai da janela. A regra é:
/// o formulário rola quando não cabe, e a tabela fica com uma fatia fixa — nunca os dois disputando
/// o mesmo espaço.
pub const RESULTS_PANEL_SHARE: f32 = 0.45;

/// Altura da tabela de resultados dentro de uma janela de `total` pixels.
///
/// Sem resultados, a tabela não reserva nada: o formulário usa a janela inteira.
/// Com resultados, a tabela fica entre [`MIN_TABLE_HEIGHT`] e [`RESULTS_PANEL_SHARE`] do total — o
/// que sobra é do formulário, que rola.
pub fn results_height(total: f32, has_results: bool) -> f32 {
    if !has_results {
        return 0.0;
    }

    (total * RESULTS_PANEL_SHARE).max(MIN_TABLE_HEIGHT)
}

impl State {
    /// Estado com o diretório padrão do cadastro.
    pub fn with_output_directory(output_directory: String) -> Self {
        Self {
            output_directory,
            ..Self::default()
        }
    }

    /// Esquece a query escolhida quando ela não existe mais (removida na aba Settings).
    ///
    /// Sem isto o combo da aba Query continuava mostrando o nome de uma query que já não existe, e
    /// o `⟳ Apply` falhava ao ser clicado.
    pub fn retain_selection_queries(&mut self, selections: &[SelectionQuery]) {
        let still_there = self.chosen_query.as_deref().is_some_and(|chosen| {
            selections
                .iter()
                .any(|query| query.name.eq_ignore_ascii_case(chosen))
        });
        if !still_there {
            self.chosen_query = None;
        }
    }

    /// Ajusta a seleção quando a lista de servidores muda (mantém o que ainda existe).
    pub fn sync_servers(&mut self, servers: &[ServerConfigEntry]) {
        self.registry = servers.to_vec();
        // A seleção de bases é por **nome**: servidor que saiu do cadastro leva as bases dele junto
        // (a lista não pode mostrar alvo que não existe mais).
        self.selection.retain_servers(servers);
        self.selection.sync_registry(servers);

        // O servidor de referência pode ter sido removido (ou renomeado) no cadastro. Sem esta
        // limpeza o combo continuava exibindo o nome antigo, o `⟳` tentava carregar um servidor que
        // não existe mais e o esquema guardado continuava alimentando as sugestões.
        let still_exists = servers
            .iter()
            .any(|server| server.name == self.reference_server);
        if !still_exists {
            self.reference_server.clear();
            self.reference_database.clear();
            self.reference_loaded = None;
            self.reference_schema = None;
        }

        // A referência do autocomplete é independente dos alvos da execução: ela nasce no primeiro
        // servidor do cadastro, não no primeiro marcado — senão o autocomplete só funcionaria depois
        // de marcar um servidor para rodar a consulta.
        if self.reference_server.is_empty()
            && let Some(first) = servers.first()
        {
            self.reference_server = first.name.clone();
        }
    }

    /// A consulta está rodando?
    pub fn is_running(&self) -> bool {
        self.phase == Phase::Running
    }

    /// Marca o inúcio da execução (limpa o que era da anterior).
    pub fn started(&mut self) {
        self.phase = Phase::Running;
        self.progress.clear();
        self.header.clear();
        self.rows.clear();
        self.total_rows = 0;
        self.files.clear();
        self.summary = None;
        self.error = None;
    }

    /// Aplica um evento de progresso vindo do núcleo.
    pub fn push_progress(&mut self, row: ProgressRow) {
        self.progress.push(row);
    }

    /// Aplica o resultado final de um job.
    pub fn apply(&mut self, result: &JobResult) {
        match result {
            JobResult::Schema {
                server,
                database,
                schema,
            } => {
                if schema.is_ok() {
                    self.reference_loaded = Some((server.clone(), database.clone()));
                }
            }
            JobResult::QueryDone {
                consolidated,
                log,
                servers,
                successes,
                failures,
                total_rows,
            } => {
                self.phase = Phase::Done;
                self.summary = Some(format!(
                    "Servers: {servers} | Success: {successes} | Failed: {failures} | Total rows: {total_rows}"
                ));
                self.files.clear();
                if let Some(consolidated) = consolidated {
                    self.files.push((
                        "Consolidated".to_string(),
                        consolidated.display().to_string(),
                    ));
                    self.load_rows(consolidated);
                }
                self.files
                    .push(("Log".to_string(), log.display().to_string()));
            }
            JobResult::QueryWithoutDatabases => {
                self.phase = Phase::Done;
                self.summary = None;
                self.error =
                    Some("No accessible databases found across selected servers.".to_string());
            }
            JobResult::Failed(error) => {
                self.phase = Phase::Done;
                self.error = Some(error.clone());
            }
            // `⟳ Apply`: a seleção do escopo pedido é **substituída** (R1). A validação é a do
            // núcleo, e o que é barrado por `excludePatterns` entra na lista desabilitado (R9).
            JobResult::Selection {
                query,
                scope,
                selection,
                ..
            } => {
                self.selection.apply(selection, scope, query);
            }
            // `⟳ Discover`: preenche o universo do servidor. Base marcada que sumiu vira aviso (R7).
            JobResult::Databases {
                server,
                databases: Ok(databases),
                ..
            } => {
                let Some(entry) = self
                    .registry
                    .iter()
                    .find(|entry| &entry.name == server)
                    .cloned()
                else {
                    return;
                };
                let lost = self.selection.discover(&entry, databases);
                if !lost.is_empty() {
                    self.error = Some(format!(
                        "{} marked database(s) are gone from '{server}': {}",
                        lost.len(),
                        lost.iter()
                            .map(|(_, database)| database.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            _ => {}
        }
    }

    /// Lê o consolidado para a tabela, respeitando [`MAX_DISPLAY_ROWS`].
    fn load_rows(&mut self, path: &PathBuf) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.error = Some(format!("Could not read {}: {error}", path.display()));
                return;
            }
        };

        let mut records = parse_records(&text).into_iter();
        self.header = records.next().unwrap_or_default();
        let mut rows: Vec<Vec<String>> = records.collect();
        self.total_rows = rows.len();
        rows.truncate(MAX_DISPLAY_ROWS);
        self.rows = rows;
    }

    /// Desenha o **formulário** (SQL, saída, servidores, botões, progresso e arquivos).
    ///
    /// Separado de [`State::ui`] porque o formulário rola: a altura dele é a que sobra depois de a
    /// tabela de resultados reservar a fatia dela.
    /// Servidor escolhido como referência do autocomplete.
    pub fn reference_server(&self) -> &str {
        &self.reference_server
    }

    /// Banco escolhido como referência do autocomplete.
    pub fn reference_database(&self) -> &str {
        &self.reference_database
    }

    /// Guarda o esquema já carregado para o motor de sugestão.
    pub fn set_reference_schema(
        &mut self,
        schema: Option<std::sync::Arc<mustel_core::usecases::schema::Schema>>,
    ) {
        self.reference_schema = schema;
    }

    /// Linha "Referência do autocomplete": servidor, banco, `⟳` e o estado do carregamento.
    ///
    /// Fica **antes** do editor (decisão do dono, 2026-10-23): o esquema é o que alimenta as
    /// sugestões, então escolher o molde primeiro é a ordem em que a tela é usada.
    ///
    /// Devolve o pedido de carregamento quando o `⟳` é clicado.
    fn reference_row(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
        reference: &crate::reference::Reference,
    ) -> Option<Request> {
        let mut request = None;

        // Referência do autocomplete: **um** banco serve de molde para as sugestões. Explícito de
        // propósito — carregar no foco surpreenderia (e pode pedir senha que a janela não tem).
        ui.horizontal(|ui| {
            ui.label("Referência do autocomplete:");

            // Servidor: o primeiro marcado é o padrão.
            if self.reference_server.is_empty()
                && let Some(first) = servers
                    .iter()
                    .find(|server| self.selected.contains(&server.name))
            {
                self.reference_server = first.name.clone();
            }

            egui::ComboBox::from_id_salt("reference_server")
                .selected_text(if self.reference_server.is_empty() {
                    "—".to_string()
                } else {
                    self.reference_server.clone()
                })
                .show_ui(ui, |ui| {
                    for server in servers {
                        if ui
                            .selectable_value(
                                &mut self.reference_server,
                                server.name.clone(),
                                &server.name,
                            )
                            .clicked()
                        {
                            // Trocar de servidor invalida o banco escolhido.
                            self.reference_database.clear();
                        }
                    }
                });

            // Banco: os **descobertos no servidor**; sem descoberta, os do cadastro. O cadastro pode
            // ter só parte dos bancos (ou nenhum, com `fetchAllDatabases`).
            let configured: &[String] = servers
                .iter()
                .find(|server| server.name == self.reference_server)
                .map(|server| server.databases.as_slice())
                .unwrap_or(&[]);
            let databases = reference.choices(&self.reference_server, configured);

            // A escolha anterior pode não existir na lista nova (outro servidor, bancos mudados).
            if !databases.contains(&self.reference_database) {
                self.reference_database = databases.first().cloned().unwrap_or_default();
            }

            egui::ComboBox::from_id_salt("reference_database")
                .selected_text(if self.reference_database.is_empty() {
                    "—".to_string()
                } else {
                    self.reference_database.clone()
                })
                .show_ui(ui, |ui| {
                    if databases.is_empty() {
                        if reference.is_pending(&self.reference_server) {
                            ui.weak("descobrindo bancos…");
                        } else if reference.error(&self.reference_server).is_some() {
                            ui.weak("não deu para listar; nenhum banco no cadastro");
                        } else {
                            ui.weak("nenhum banco no cadastro");
                        }
                    }
                    for database in &databases {
                        ui.selectable_value(
                            &mut self.reference_database,
                            database.clone(),
                            database,
                        );
                    }
                });

            let can_load = reference.can_load(&self.reference_server, &self.reference_database);
            let load = ui.add_enabled(can_load, egui::Button::new("⟳"));
            let load = match reference
                .blocked_reason(&self.reference_server, &self.reference_database)
            {
                Some(reason) => load.on_disabled_hover_text(reason),
                None => load.on_hover_text("Carregar o esquema deste banco para o autocomplete"),
            };

            if load.clicked() {
                request = Some(Request::LoadSchema {
                    server: self.reference_server.clone(),
                    database: self.reference_database.clone(),
                });
            }

            // Rótulo do estado: o que está carregado, se algo.
            match &self.reference_loaded {
                Some((server, database))
                    if server == &self.reference_server && database == &self.reference_database =>
                {
                    ui.weak(format!("carregado ({database})"));
                }
                Some(_) => {
                    ui.weak("carregado outro banco");
                }
                None => {
                    ui.weak("não carregado");
                }
            }
        });

        request
    }

    fn form_ui(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
        selections: &[SelectionQuery],
        reference: &crate::reference::Reference,
    ) -> Option<Request> {
        let mut loaded_request: Option<Request> = None;
        let mut request = None;
        let running = self.is_running();

        // A referência vem **antes** do editor: é o molde que alimenta as sugestões.
        if let Some(load) = self.reference_row(ui, servers, reference) {
            loaded_request = Some(load);
        }

        ui.label("SQL");
        // A altura é calculada **antes** do widget: `TextEdit::multiline` toma `&mut self.sql`, e
        // ler o texto depois disso seria um segundo empréstimo.
        let rows = sql_editor::desired_rows(&self.sql);

        // A caixa acompanha o texto: cresce com quebras e com linhas longas, até um teto em que
        // passa a rolar. O realce é o mesmo em toda edição — o `layouter` é chamado a cada quadro,
        // mas o trabalho é proporcional ao texto, não à janela.
        let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = sql_editor::layout_job(ui, buffer.as_str(), wrap_width);
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };

        let text_edit = egui::TextEdit::multiline(&mut self.sql)
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .layouter(&mut layouter);

        // Autocomplete: o `CompletionPopup` do egui consome `Tab`/`Enter`/setas **antes** do
        // `TextEdit` e insere o texto aceito — não há consumo manual de teclas aqui.
        //
        // Gatilho, fronteira de palavra e pedido ao motor ficam em `sql_completion`: é lá que as
        // regras vivem (e são testadas sem janela).
        let force = ui.input(sql_completion::force_requested);
        let schema = self.reference_schema.clone();

        // O gatilho vive no estado da tela e atravessa o quadro: o `popup` pergunta a cada quadro se
        // deve abrir, e as respostas do atalho/aceite valem para os quadros seguintes.
        let mut trigger = std::mem::take(&mut self.completion_trigger);

        let output = egui::CompletionPopup::new(sql_completion::id())
            .word_boundary(sql_completion::word_boundary)
            .accept_keys([egui::Key::Tab, egui::Key::Enter])
            // Abaixo da caixa: o padrão do egui (`TOP_START`) desenha a lista **acima** do editor,
            // tapando o cabeçalho da janela. Se não houver espaço, o egui inverte sozinho.
            .align(egui::RectAlign::BOTTOM_START)
            .max_height(220.0)
            .show(ui, text_edit, |query| {
                sql_completion::candidates(&mut trigger, query, schema.as_deref(), force)
            });

        trigger.record_accepted(output.accepted.is_some());
        self.completion_trigger = trigger;

        // O popup já gravou o texto aceito no buffer; o retorno não tem o que fazer aqui.
        drop(output);

        ui.horizontal(|ui| {
            ui.label("Output directory");
            ui.add(egui::TextEdit::singleline(&mut self.output_directory).desired_width(250.0));

            // Dois botões ao lado da caixa: escolher (abre o seletor do app) e abrir (mostra a pasta
            // no gerenciador de arquivos). Ícone + tooltip para não roubar largura da caixa.
            if ui
                .button("📂 Escolher")
                .on_hover_text("Escolher a pasta de saída")
                .clicked()
            {
                let base =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let start = desktop::resolve_directory(&self.output_directory, &base);
                self.picker.open(&start);
            }

            let base = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let target = desktop::resolve_directory(&self.output_directory, &base);
            let can_open = desktop::can_open(&target);
            let tooltip = if can_open {
                "Abrir a pasta no gerenciador de arquivos".to_string()
            } else {
                format!("A pasta ainda não existe: {}", target.display())
            };

            let open = ui.add_enabled(can_open, egui::Button::new("🗂 Abrir"));
            let open = if can_open {
                open.on_hover_text(tooltip)
            } else {
                open.on_disabled_hover_text(tooltip)
            };
            if open.clicked()
                && let Err(error) = desktop::open_directory(&target)
            {
                self.error = Some(format!("Não consegui abrir a pasta: {error}"));
            }
        });

        // O seletor é um modal: desenha sobre a tela, então basta chamar aqui uma vez por frame.
        if let Some(chosen) = self.picker.show(ui.ctx()) {
            self.output_directory = chosen.display().to_string();
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Databases");

            // Combo das queries salvas na aba Settings (`— none` = seleção à mão).
            let chosen = self.chosen_query.clone();
            let label = chosen.clone().unwrap_or_else(|| "— none —".to_string());
            egui::ComboBox::from_id_salt("selection_query")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.chosen_query, None, "— none —");
                    for query in selections {
                        ui.selectable_value(
                            &mut self.chosen_query,
                            Some(query.name.clone()),
                            &query.name,
                        );
                    }
                });

            // `⟳ Apply` substitui a seleção do escopo (R1). Sem query escolhida não há o que
            // aplicar: o botão fica desabilitado em vez de adivinhar.
            let can_apply = self.chosen_query.is_some() && !running;
            if ui
                .add_enabled(can_apply, egui::Button::new("⟳ Apply to all"))
                .on_hover_text("Mark the databases the saved query returns, on every server")
                .clicked()
                && let Some(query) = &self.chosen_query
            {
                request = Some(Request::ApplySelection {
                    query: query.clone(),
                    scope: crate::selection::ApplyScope::All,
                });
            }

            // `⟳ Discover` enche o universo (R6), em melhor esforço por servidor.
            if ui
                .add_enabled(!running, egui::Button::new("⟳ Discover all"))
                .on_hover_text("List the databases of every registered server")
                .clicked()
            {
                request = Some(Request::DiscoverDatabases { server: None });
            }

            ui.weak(format!("{} selected", self.selection.marked_count()));
        });

        // Busca e paginação: a seleção é guardada por nome, então filtrar/virar página não perde
        // nada (R7).
        ui.horizontal(|ui| {
            let mut search = self.selection.search().to_string();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut search)
                        .hint_text("Search database…")
                        .desired_width(200.0),
                )
                .changed()
            {
                self.selection.set_search(search);
            }

            let page = self.selection.page(servers);
            let pages = self.selection.page_count(servers);
            if ui.add_enabled(page > 0, egui::Button::new("‹")).clicked() {
                self.selection.turn_page(servers, false);
            }
            ui.label(format!("{} / {}", page + 1, pages));
            if ui
                .add_enabled(page + 1 < pages, egui::Button::new("›"))
                .clicked()
            {
                self.selection.turn_page(servers, true);
            }
        });

        // A lista: cabeçalho por servidor (com os botões dele) e as bases embaixo.
        egui::ScrollArea::vertical()
            .id_salt("query_databases")
            .max_height(150.0)
            .show(ui, |ui| {
                let mut current: Option<String> = None;
                for row in self.selection.page_rows(servers) {
                    if current.as_deref() != Some(row.server.as_str()) {
                        let Some(server) = servers.iter().find(|server| server.name == row.server)
                        else {
                            continue;
                        };
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.strong(format!("{} ({}:{})", server.name, server.host, server.port));

                            if ui
                                .add_enabled(
                                    self.chosen_query.is_some() && !running,
                                    egui::Button::new("⟳ Apply"),
                                )
                                .on_hover_text("Mark what the saved query returns for this server")
                                .clicked()
                                && let Some(query) = &self.chosen_query
                            {
                                request = Some(Request::ApplySelection {
                                    query: query.clone(),
                                    scope: crate::selection::ApplyScope::Server(
                                        server.name.clone(),
                                    ),
                                });
                            }

                            if ui
                                .add_enabled(!running, egui::Button::new("⟳ Discover"))
                                .clicked()
                            {
                                request = Some(Request::DiscoverDatabases {
                                    server: Some(server.name.clone()),
                                });
                            }

                            ui.weak(format!(
                                "{} of {}",
                                self.selection.marked_on(&server.name),
                                self.selection.known_on(&server.name)
                            ));
                        });
                        current = Some(row.server.clone());
                    }

                    // Base barrada por `excludePatterns`: aparece **desabilitada**, com o padrão no
                    // tooltip — a exclusão não pode ser silenciosa (R9).
                    //
                    // Nome vazio é servidor sem base conhecida: mostra a dica e deixa o `⟳ Discover`
                    // do cabeçalho como única ação.
                    if row.database.is_empty() {
                        ui.weak("no database listed yet — use ⟳ Discover");
                        continue;
                    }

                    let mut checked = row.marked;
                    let response = match &row.info.blocked_by {
                        Some(pattern) => {
                            // Base barrada: aparece **desabilitada**, com o padrão no tooltip — a
                            // exclusão não pode ser silenciosa (R9).
                            let inside = ui
                                .add_enabled_ui(false, |ui| {
                                    ui.checkbox(&mut checked, &row.database)
                                })
                                .inner;
                            inside.on_disabled_hover_text(format!(
                                "Excluded by the server's excludePatterns: {pattern}"
                            ))
                        }
                        None => ui.checkbox(&mut checked, &row.database),
                    };

                    // Aviso de credencial (D8): a linha não é marcada sozinha, e o motivo aparece
                    // no tooltip — senão o dono veria uma base a menos sem saber por quê.
                    let response = match &row.info.warning {
                        Some(warning) => response.on_hover_text(warning),
                        None => response,
                    };
                    if response.changed() {
                        self.selection
                            .set_marked(&row.server, &row.database, checked);
                    }
                }
            });

        ui.separator();
        // O `horizontal` devolve o pedido em vez de escrever no `request` de fora: um `Option<Request>`
        // não é `Copy`, e atribuir dentro do closure o moveria para lá.
        let run_request = ui
            .horizontal(|ui| {
                let targets = self.selection.targets(servers);

                if ui
                    .add_enabled(
                        !running && !targets.is_empty(),
                        egui::Button::new("Run query"),
                    )
                    .clicked()
                {
                    // A decisão de pedir confirmação fica numa função só, usada pelo botão e
                    // testável sem janela.
                    self.request_run(targets)
                } else {
                    None
                }
            })
            .inner;

        if run_request.is_some() {
            request = run_request;
        }

        let cancel_request = ui
            .horizontal(|ui| {
                let mut cancel = None;
                if running && ui.button("Cancel").clicked() {
                    self.phase = Phase::Idle;
                    cancel = Some(Request::CancelJob);
                }
                if running {
                    ui.spinner();
                }
                if let Some(summary) = &self.summary {
                    ui.label(summary);
                }
                cancel
            })
            .inner;

        if cancel_request.is_some() {
            request = cancel_request;
        }

        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
        }

        if !self.progress.is_empty() {
            ui.separator();
            ui.label("Progress");
            egui::ScrollArea::vertical()
                .id_salt("progress")
                .max_height(120.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for row in &self.progress {
                        let line = if row.error.is_empty() {
                            format!(
                                "✓ {}/{} — {} rows — {:.1}s",
                                row.server, row.database, row.rows, row.seconds
                            )
                        } else {
                            format!("✗ {}/{} — {}", row.server, row.database, row.error)
                        };
                        ui.monospace(line);
                    }
                });
        }

        if !self.files.is_empty() {
            ui.separator();
            for (label, path) in &self.files {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{label}: {path}"));
                    if ui.button("Copy path").clicked() {
                        ui.ctx().copy_text(path.clone());
                    }
                });
            }
        }

        loaded_request.or(request)
    }

    /// Desenha a tela: o formulário rola, a tabela de resultados fica com a fatia de baixo.
    ///
    /// Dois consumidores de altura crescem com o uso — o SQL e as linhas do resultado. Sem divisão,
    /// um deles sai da janela e não há como alcançá-lo (bug relatado: com o editor alto, a barra de
    /// rolagem não existia). A divisão é explícita em [`results_height`] e testada.
    /// Pede a rolagem suave até o fim do formulário.
    ///
    /// Separado do desenho para ser testável: o clique é do egui, mas a decisão (quando animar e de
    /// onde para onde) é do app.
    pub fn scroll_to_end(&mut self, current: f32, max_offset: f32) {
        self.form_scroll.start(current, max_offset);
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
        selections: &[SelectionQuery],
        schema: Option<std::sync::Arc<mustel_core::usecases::schema::Schema>>,
        reference: &crate::reference::Reference,
    ) -> Option<Request> {
        self.set_reference_schema(schema);
        let total = ui.available_height();
        let has_results = !self.rows.is_empty();
        let table_height = results_height(total, has_results);
        let form_height = (total - table_height).max(MIN_FORM_HEIGHT);

        // Barra **sólida** (não flutuante): o padrão do egui é `floating`, que só aparece enquanto o
        // usuário rola — e some depois. Numa tela que cresce sozinha, a barra precisa ser visível
        // o tempo todo, senão ninguém descobre que há mais conteúdo abaixo.
        let dt = ui.input(|input| input.stable_dt);
        let animated_offset = self.form_scroll.step(dt);

        let area = ui
            .scope(|ui| {
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("query_form")
                    .max_height(form_height)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);

                // Enquanto anima, o offset vem da animação; fora disso o `ScrollArea` obedece ao
                // usuário (roda, arrasto da barra, teclado).
                if let Some(offset) = animated_offset {
                    scroll = scroll.vertical_scroll_offset(offset);
                }

                scroll.show(ui, |ui| self.form_ui(ui, servers, selections, reference))
            })
            .inner;
        let mut request = area.inner;

        // A linha de referência precisa dos bancos do servidor escolhido: pede a descoberta uma vez
        // por servidor (o app guarda a resposta e a tela não repete o pedido a cada quadro).
        if request.is_none() && reference.unknown(self.reference_server()) {
            request = Some(Request::DiscoverDatabases {
                server: Some(self.reference_server().to_string()),
            });
        }

        if animated_offset.is_some() {
            // Sem isto a animação pararia no primeiro quadro: ninguém pediria o próximo.
            ui.ctx().request_repaint();
        }

        // Botão flutuante de "ir ao fim": desenhado **depois** do `ScrollArea`, sobre a região dele.
        // Fica no canto **inferior direito** (o esquerdo é do `Run query`) e some quando já se está no
        // fim, para não cobrir o botão de executar.
        let content_height = area.content_size.y;
        let viewport_height = area.inner_rect.height();
        let max_offset = (content_height - viewport_height).max(0.0);
        let offset = area.state.clamped_offset().y;

        if should_show_scroll_button(content_height, viewport_height, offset) {
            let size = egui::vec2(30.0, 24.0);
            // Recuo da borda direita: a barra sólida ocupa ~12 px, e o botão não deve encostar nela.
            let rect = egui::Rect::from_min_size(
                egui::pos2(
                    area.inner_rect.right() - size.x - 22.0,
                    area.inner_rect.bottom() - size.y - 10.0,
                ),
                size,
            );

            let button = ui.put(rect, egui::Button::new("▼").small());
            let button = button.on_hover_text("Ir para o fim da página");

            if button.clicked() {
                self.scroll_to_end(offset, max_offset);
                ui.ctx().request_repaint();
            }
        }

        if has_results {
            ui.separator();
            ui.label(format!(
                "Results ({} row(s){})",
                self.total_rows,
                if self.total_rows > self.rows.len() {
                    format!(", showing the first {}", self.rows.len())
                } else {
                    String::new()
                }
            ));
            self.results_table(ui, table_height);
        }

        // O popup de confirmação destrutiva é desenhado por cima de tudo.
        if self.confirmation.is_some() {
            request = self.confirmation_modal(ui.ctx()).or(request);
        }

        request
    }

    /// Decide o que o botão `Run query` faz: executa direto ou pede confirmação.
    ///
    /// Consulta destrutiva **sempre** pede confirmação aqui — a GUI não tem o `--no-confirm` do
    /// CLI, e apagar dado por engano não tem desfazer. O que decide é o `analyzer` do núcleo, o
    /// mesmo que o CLI usa, então a classificação é a mesma dos dois lados.
    fn request_run(&mut self, targets: Vec<(String, Vec<String>)>) -> Option<Request> {
        let run = Request::RunQuery {
            targets: targets.clone(),
            sql: self.sql.clone(),
            output_directory: self.output_directory.clone(),
        };

        if analyzer::analyze(&self.sql) != QueryType::Destructive {
            return Some(run);
        }

        self.confirmation = Some(Confirmation {
            description: analyzer::description(&self.sql).to_string(),
            servers: targets.len(),
            // As bases são **explícitas**: o plano roda exatamente as marcadas (D15).
            databases: targets.iter().map(|(_, databases)| databases.len()).sum(),
            preview: preview_of(&self.sql),
            request: run,
        });

        None
    }

    /// O popup de confirmação de consulta destrutiva.
    ///
    /// Mesmo conteúdo que o CLI mostra (tipo, servidores, bancos e um trecho da consulta), com ícone
    /// de alerta; a decisão é do usuário e o padrão seguro é **cancelar**.
    fn confirmation_modal(&mut self, ctx: &egui::Context) -> Option<Request> {
        let Some(confirmation) = &self.confirmation else {
            return None;
        };

        let mut decision: Option<bool> = None;
        let response =
            egui::Modal::new(egui::Id::unique("destructive_query_dialog")).show(ctx, |ui| {
                ui.set_max_width(460.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚠")
                            .size(28.0)
                            .color(egui::Color32::from_rgb(200, 60, 40)),
                    );
                    ui.label(
                        egui::RichText::new("WARNING: DESTRUCTIVE QUERY DETECTED")
                            .strong()
                            .color(egui::Color32::from_rgb(200, 60, 40)),
                    );
                });

                ui.add_space(6.0);
                egui::Grid::new("destructive_details")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Query Type");
                        ui.monospace(&confirmation.description);
                        ui.end_row();
                        ui.label("Servers Affected");
                        ui.monospace(confirmation.servers.to_string());
                        ui.end_row();
                        ui.label("Databases Affected");
                        ui.monospace(format!("~{}", confirmation.databases));
                        ui.end_row();
                        ui.label("Preview");
                        ui.monospace(&confirmation.preview);
                        ui.end_row();
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // `Cancel` primeiro: o padrão seguro é não executar.
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                    ui.add_space(8.0);
                    if ui
                        .button(
                            egui::RichText::new("Run anyway")
                                .color(egui::Color32::from_rgb(200, 60, 40)),
                        )
                        .clicked()
                    {
                        decision = Some(true);
                    }
                });
            });

        // Clicar fora do popup equivale a cancelar.
        if response.backdrop_response.clicked() {
            decision = Some(false);
        }

        match decision {
            Some(true) => {
                let confirmation = self.confirmation.take()?;
                Some(confirmation.request)
            }
            Some(false) => {
                self.confirmation = None;
                self.error = Some("Query execution cancelled by user.".to_string());
                None
            }
            None => None,
        }
    }

    /// A tabela **virtualizada**: só as linhas visíveis são desenhadas.
    ///
    /// `TableBuilder` e não `ScrollArea::show_rows`: a rolagem vertical é da **tabela**, então a barra
    /// fica na borda direita do painel (com o `ScrollArea` aninhado ela ficava na largura do
    /// conteúdo, no meio da tela), o cabeçalho fica fixo e as colunas são redimensionáveis.
    fn results_table(&mut self, ui: &mut egui::Ui, height: f32) {
        use egui_extras::{Column, TableBuilder};

        let header = self.header.clone();
        let row_height = egui::TextStyle::Body.resolve(ui.style()).size + 6.0;

        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            // `auto_shrink([false, false])`: sem isso a tabela encolhe até o conteúdo e a barra de
            // rolagem fica na largura das colunas, no meio da tela. Com isso ela ocupa a área toda e
            // a barra encosta na borda direita.
            .auto_shrink([false, false])
            // Barra **sempre visível**: com 20 000 linhas ela é a indicação de que há mais conteúdo
            // embaixo, e deixa claro onde termina a tabela. Com `VisibleWhenNeeded` ela só aparece
            // depois do primeiro quadro de rolagem.
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            // A tabela ocupa o que sobrou da janela.
            .max_scroll_height(height.max(MIN_TABLE_HEIGHT));

        for index in 0..header.len() {
            // A última coluna fica com o resto da largura; as outras se ajustam ao conteúdo e podem
            // ser arrastadas. `clip(false)` é o que evita o `19…` de antes.
            table = if index + 1 == header.len() {
                table.column(Column::remainder().at_least(60.0))
            } else {
                table.column(Column::auto().at_least(60.0).clip(false))
            };
        }

        table
            .header(row_height, |mut row| {
                for name in &header {
                    row.col(|ui| {
                        ui.label(egui::RichText::new(name).strong());
                    });
                }
            })
            .body(|body| {
                let rows = &self.rows;
                body.rows(row_height, rows.len(), |mut row| {
                    let index = row.index();
                    for cell in &rows[index] {
                        row.col(|ui| {
                            ui.label(egui::RichText::new(cell).monospace());
                        });
                    }
                });
            });
    }
}

/// Trecho da consulta para o usuário reconhecer o que ia rodar (200 caracteres, como o CLI).
fn preview_of(sql: &str) -> String {
    let preview: String = sql.chars().take(200).collect();

    if sql.chars().count() > 200 {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IF-201: clicar no botão anima do offset atual até o fim; se já está no fim, não liga animação.
    /// CONTRACT: IF-201
    #[test]
    fn the_scroll_to_end_button_starts_a_smooth_animation() {
        let mut state = State::default();

        state.scroll_to_end(0.0, 400.0);
        assert!(
            state.form_scroll.is_active(),
            "clicar precisa começar a rolagem suave"
        );

        let mut already_there = State::default();
        already_there.scroll_to_end(400.0, 400.0);
        assert!(
            !already_there.form_scroll.is_active(),
            "já no fim não anima nada"
        );
    }

    /// IF-201: o botão não aparece quando não há overflow nem quando já se chegou ao fim — é isso
    /// que impede ele de cobrir o `Run query` na página rolada até embaixo.
    /// CONTRACT: IF-201
    #[test]
    fn the_scroll_button_hides_when_there_is_nothing_left_to_scroll() {
        // Conteúdo cabe na janela: nada a rolar.
        assert!(!should_show_scroll_button(200.0, 400.0, 0.0));

        // Transborda, no topo: aparece.
        assert!(should_show_scroll_button(800.0, 400.0, 0.0));

        // Transborda, no meio: continua aparecendo.
        assert!(should_show_scroll_button(800.0, 400.0, 200.0));

        // Chegou ao fim: some, porque não há para onde ir e ele cobriria o `Run query`.
        assert!(!should_show_scroll_button(800.0, 400.0, 400.0));

        // Rollou de volta para cima: reaparece.
        assert!(should_show_scroll_button(800.0, 400.0, 100.0));
    }

    /// IF-200: com resultados, a tabela reserva uma fatia e o formulário fica com o resto — os dois
    /// nunca disputam a janela inteira.
    /// CONTRACT: IF-200
    #[test]
    fn the_results_panel_takes_a_share_and_leaves_the_form_the_rest() {
        let total = 800.0;
        let table = results_height(total, true);

        assert!(table > 0.0, "com resultados a tabela reserva espaço");
        assert!(
            table >= MIN_TABLE_HEIGHT,
            "nunca menor que o mínimo ({table})"
        );
        assert!(
            table < total,
            "a tabela não pode comer a janela inteira ({table} de {total})"
        );
        assert!(
            total - table >= MIN_FORM_HEIGHT,
            "sobra pelo menos o mínimo para o formulário"
        );
    }

    /// Sem resultados, nada é reservado: o formulário usa a janela toda e rola se precisar.
    /// CONTRACT: IF-200
    #[test]
    fn without_results_the_form_has_the_whole_window() {
        assert_eq!(results_height(800.0, false), 0.0);
    }

    /// Janela pequena: a tabela nunca cai abaixo do mínimo, e o formulário mantém o mínimo dele.
    #[test]
    fn a_small_window_keeps_both_minimums() {
        // Com 300 px a fatia de 45% daria 135, que já passa do mínimo da tabela.
        let table = results_height(300.0, true);
        assert!(
            table >= MIN_TABLE_HEIGHT,
            "a tabela nunca cai abaixo do mínimo ({table})"
        );

        // Numa janela minúscula a fatia seria menor que o mínimo, e o piso entra em ação.
        let tiny = results_height(100.0, true);
        assert_eq!(tiny, MIN_TABLE_HEIGHT, "o piso da tabela entra em ação");

        let form = (300.0 - table).max(MIN_FORM_HEIGHT);
        assert!(
            form >= MIN_FORM_HEIGHT,
            "o formulário mantém o mínimo dele ({form})"
        );
    }

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            ..ServerConfigEntry::default()
        }
    }

    /// Nenhum servidor vem marcado por padrão: quem escolhe os alvos é quem vai executar.
    ///
    /// Muda o contrato anterior ("servidor único já vem marcado, como no CLI"): na janela a lista
    /// está sempre visível e o `Run query` já habilitado é fácil de disparar sem querer.
    /// CONTRACT: IF-217
    #[test]
    fn nothing_is_selected_by_default() {
        let mut state = State::default();

        state.sync_servers(&[server("dev")]);
        assert!(state.selected.is_empty(), "nem com um servidor só");

        state.sync_servers(&[server("dev"), server("outro")]);
        assert!(state.selected.is_empty(), "nem com vários");
    }

    /// A referência do autocomplete nasce no primeiro servidor do cadastro, sem depender da seleção.
    ///
    /// CONTRACT: IF-217
    #[test]
    fn the_autocomplete_reference_does_not_depend_on_the_selection() {
        let mut state = State::default();

        state.sync_servers(&[server("dev"), server("outro")]);

        assert_eq!(state.reference_server, "dev", "primeiro do cadastro");
        assert_eq!(
            state.selection.marked_count(),
            0,
            "e continua sem base marcada para rodar"
        );
    }

    /// Marcar à mão continua valendo, e servidor que sai do cadastro leva a seleção dele junto.
    #[test]
    fn the_selection_drops_servers_that_went_away() {
        let mut state = State::default();
        state.sync_servers(&[server("dev"), server("outro")]);
        state
            .selection
            .discover(&server("dev"), &["a".to_string(), "b".to_string()]);
        state
            .selection
            .discover(&server("outro"), &["c".to_string()]);
        state.selection.set_marked("dev", "a", true);
        state.selection.set_marked("outro", "c", true);
        assert_eq!(state.selection.marked_count(), 2);

        state.sync_servers(&[server("dev")]);

        assert_eq!(state.selection.marked_on("dev"), 1, "o que ficou, ficou");
        assert_eq!(
            state.selection.marked_on("outro"),
            0,
            "servidor fora do cadastro não tem alvo"
        );
    }

    /// Remover o servidor de referência limpa a escolha, o rótulo e o esquema.
    ///
    /// Bug relatado: apagando todos os servidores, o combo continuava mostrando o nome do que já não
    /// existia, e o esquema velho seguia alimentando as sugestões.
    ///
    /// Sobrando outro servidor, a referência passa para ele: o autocomplete continua com um molde
    /// válido, sem o usuário ter de reescolher.
    /// CONTRACT: IF-216
    #[test]
    fn removing_the_reference_server_clears_the_reference_state() {
        let mut state = State::default();
        state.sync_servers(&[server("Local"), server("outro")]);
        state.reference_server = "Local".to_string();
        state.reference_database = "dev".to_string();
        state.reference_loaded = Some(("Local".to_string(), "dev".to_string()));
        state.reference_schema = Some(std::sync::Arc::new(
            mustel_core::usecases::schema::Schema::default(),
        ));

        // O servidor sai do cadastro, mas ainda há outro.
        state.sync_servers(&[server("outro")]);

        assert_eq!(
            state.reference_server, "outro",
            "a referência passa para quem sobrou"
        );
        assert!(
            state.reference_database.is_empty(),
            "o banco do servidor que saiu não vale para o novo"
        );
        assert!(
            state.reference_loaded.is_none(),
            "o rótulo volta a 'não carregado'"
        );
        assert!(
            state.reference_schema.is_none(),
            "as sugestões perdem o molde antigo"
        );

        // Sem servidor nenhum, não sobra nada — é o caso do relato.
        state.sync_servers(&[]);
        assert!(
            state.reference_server.is_empty(),
            "o combo não guarda fantasma"
        );
        assert!(state.reference_database.is_empty());
    }

    #[test]
    fn the_consolidated_file_becomes_the_table_rows() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("consolidated.csv");
        // O formato que o próprio núcleo escreve: BOM, CRLF e a linha `Server,Database,...`.
        std::fs::write(
            &path,
            "\u{feff}Server,Database,valor\r\ndev,mydb,1\r\ndev,mydb,2\r\n",
        )
        .expect("escreve");

        let mut state = State::default();
        state.apply(&JobResult::QueryDone {
            consolidated: Some(path),
            log: directory.path().join("log.csv"),
            servers: 1,
            successes: 2,
            failures: 0,
            total_rows: 2,
        });

        assert_eq!(
            state.header,
            vec![
                "Server".to_string(),
                "Database".to_string(),
                "valor".to_string()
            ],
            "o BOM não vira parte do primeiro nome"
        );
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.total_rows, 2);
        assert_eq!(state.rows[1], vec!["dev", "mydb", "2"]);
        assert_eq!(state.files.len(), 2, "consolidado e log");
    }

    #[test]
    fn the_display_cap_keeps_the_file_untouched() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("consolidated.csv");
        let mut content = String::from("\u{feff}Server,Database,valor\r\n");
        for index in 0..(MAX_DISPLAY_ROWS + 10) {
            content.push_str(&format!("dev,mydb,{index}\r\n"));
        }
        std::fs::write(&path, &content).expect("escreve");

        let mut state = State::default();
        state.apply(&JobResult::QueryDone {
            consolidated: Some(path),
            log: directory.path().join("log.csv"),
            servers: 1,
            successes: 1,
            failures: 0,
            total_rows: 1,
        });

        assert_eq!(state.total_rows, MAX_DISPLAY_ROWS + 10, "o total lido");
        assert_eq!(
            state.rows.len(),
            MAX_DISPLAY_ROWS,
            "só o teto fica na memória"
        );
    }

    #[test]
    fn a_failed_run_shows_the_core_message() {
        let mut state = State::default();
        state.started();
        assert!(state.is_running());

        state.apply(&JobResult::Failed(
            "No server responded successfully.".to_string(),
        ));

        assert!(!state.is_running());
        assert_eq!(
            state.error.as_deref(),
            Some("No server responded successfully.")
        );
    }

    #[test]
    fn the_destructive_check_uses_the_core_analyzer() {
        // O que a tela consulta antes de abrir o popup: o mesmo `analyzer` do CLI.
        assert_eq!(analyzer::analyze("SELECT 1"), QueryType::Safe);
        assert_eq!(analyzer::analyze("DELETE FROM t"), QueryType::Destructive);
        assert_eq!(
            analyzer::analyze("UPDATE t SET a = 1"),
            QueryType::Destructive
        );
        assert_eq!(
            analyzer::analyze("DROP TABLE t"),
            QueryType::Destructive,
            "`DROP` é o caso que mais dói se passar"
        );
    }

    #[test]
    fn the_confirmation_preview_truncates_like_the_cli() {
        assert_eq!(preview_of("DELETE FROM t"), "DELETE FROM t");

        let long = "DELETE FROM t WHERE ".to_string() + &"x".repeat(400);
        let preview = preview_of(&long);
        assert_eq!(preview.chars().count(), 203, "200 + reticências");
        assert!(preview.ends_with("..."), "{preview}");
    }

    #[test]
    fn destructive_sql_opens_the_confirmation_instead_of_running() {
        let mut state = State {
            sql: "DELETE FROM t".to_string(),
            ..State::default()
        };

        let request = state.request_run(vec![("dev".to_string(), vec!["dev".to_string()])]);

        assert!(request.is_none(), "nada roda antes de confirmar");
        let confirmation = state.confirmation.as_ref().expect("abriu a confirmação");
        assert_eq!(confirmation.servers, 1);
        assert_eq!(confirmation.databases, 1, "um banco no cadastro");
        assert_eq!(confirmation.preview, "DELETE FROM t");
        assert!(
            matches!(confirmation.request, Request::RunQuery { .. }),
            "a confirmação guarda o pedido pronto para executar"
        );
    }

    #[test]
    fn safe_sql_runs_without_asking() {
        let mut state = State {
            sql: "SELECT 1".to_string(),
            ..State::default()
        };

        let request = state.request_run(vec![("dev".to_string(), vec!["dev".to_string()])]);

        assert!(matches!(request, Some(Request::RunQuery { .. })));
        assert!(state.confirmation.is_none());
    }

    #[test]
    fn a_server_that_discovers_databases_counts_as_one() {
        let mut state = State {
            sql: "DROP TABLE t".to_string(),
            ..State::default()
        };
        let _ = state.request_run(vec![(
            "dev".to_string(),
            vec!["a".to_string(), "b".to_string()],
        )]);

        assert_eq!(
            state.confirmation.expect("abriu").databases,
            2,
            "a seleção é explícita: conta as bases marcadas, não descobre (D15)"
        );
    }

    /// Esquecer a query escolhida quando ela sai do cadastro (aba Settings).
    ///
    /// Sem isto o combo da aba Query continuava exibindo o nome de uma query que já não existia.
    /// CONTRACT: IF-230
    #[test]
    fn removing_the_query_in_use_resets_the_combo() {
        let mut state = State {
            chosen_query: Some("Smoke".to_string()),
            ..State::default()
        };

        // Ainda existe (com outra caixa): o combo fica.
        state.retain_selection_queries(&[SelectionQuery {
            name: "smoke".to_string(),
            server: "Local".to_string(),
            database: "dev".to_string(),
            sql: "select 1".to_string(),
        }]);
        assert_eq!(state.chosen_query.as_deref(), Some("Smoke"));

        // Removida: volta para `— none —`.
        state.retain_selection_queries(&[]);
        assert_eq!(state.chosen_query, None);
    }

    /// A linha de referência pede a descoberta do servidor que ainda não foi descoberto.
    ///
    /// CONTRACT: IF-211
    #[test]
    fn the_reference_row_asks_for_the_databases_of_an_unknown_server() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        state.sync_servers(&[server("local")]);
        let reference = crate::reference::Reference::default();

        let mut request = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                request = state.ui(ui, &[server("local")], &[], None, &reference);
            });
        });
        output.textures_delta.clear();

        assert_eq!(
            request,
            Some(Request::DiscoverDatabases {
                server: Some("local".to_string())
            }),
            "sem descoberta, a linha pede os bancos do servidor"
        );

        // Já descoberto: não pede de novo.
        let mut known = crate::reference::Reference::default();
        known.finish(
            "local",
            crate::reference::Known {
                databases: Ok(vec!["dev".to_string(), "test2".to_string()]),
                has_password: true,
            },
        );
        let mut request = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                request = state.ui(ui, &[server("local")], &[], None, &known);
            });
        });
        output.textures_delta.clear();
        assert_eq!(request, None, "descoberta conhecida não vira pedido");
    }

    /// A linha de referência vem **antes** do editor de SQL.
    ///
    /// A ordem é a decisão (o molde alimenta as sugestões, então se escolhe antes de escrever), e
    /// ela não cabe num teste de regra pura: aqui o quadro é desenhado sem janela e as posições
    /// dos rótulos são conferidas no que o egui realmente emitiu.
    /// CONTRACT: IF-214
    #[test]
    fn the_reference_row_comes_before_the_sql_editor() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        state.sync_servers(&[server("dev")]);
        let reference = crate::reference::Reference::default();

        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.ui(ui, &[server("dev")], &[], None, &reference);
            });
        });

        // Onde cada rótulo foi pintado (eixo Y da tela).
        let mut heights: Vec<(String, f32)> = Vec::new();
        collect_text(&output.shapes, &mut heights);
        output.textures_delta.clear();

        let position = |label: &str| {
            heights
                .iter()
                .find(|(text, _)| text == label)
                .map(|(_, y)| *y)
                .unwrap_or_else(|| panic!("o rótulo {label:?} não foi desenhado: {heights:?}"))
        };

        let reference_y = position("Referência do autocomplete:");
        let sql_y = position("SQL");

        assert!(
            reference_y < sql_y,
            "a referência ({reference_y}) precisa vir antes do editor ({sql_y})"
        );
    }

    /// Junta `(texto, y)` de todo `Shape::Text`, descendo nos grupos.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<(String, f32)>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
            match shape {
                egui::Shape::Text(text) => {
                    let content = text.galley.text().trim().to_string();
                    if !content.is_empty() {
                        out.push((content, text.pos.y));
                    }
                }
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

    #[test]
    fn the_modal_renders_without_a_window() {
        // O caminho de desenho do popup é exercitado num contexto do `egui` sem janela: é o que pega
        // erro de layout/borrow antes de alguém abrir a GUI.
        let ctx = egui::Context::default();
        let mut state = State {
            sql: "DELETE FROM t".to_string(),
            ..State::default()
        };
        let _ = state.request_run(vec![("dev".to_string(), vec!["dev".to_string()])]);
        assert!(state.confirmation.is_some());

        let output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            // Clone do contexto: o `show` empresta `&Context` e o popup precisa dele ao mesmo
            // tempo.
            let inner = ctx.clone();
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.confirmation_modal(&inner);
                let _ = ui;
            });
        });

        assert!(!output.shapes.is_empty(), "o popup desenhou alguma coisa");
        // O `epaint` exige que as texturas do quadro sejam consumidas ou limpas antes do descarte.
        let mut textures = output.textures_delta;
        textures.clear();
    }
}
