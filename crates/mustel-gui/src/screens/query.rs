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

/// Teto de linhas carregadas para a tabela. O arquivo não é truncado — só a leitura da tela.
pub const MAX_DISPLAY_ROWS: usize = 20_000;

/// Altura mínima reservada para a tabela quando a janela é pequena.
const MIN_TABLE_HEIGHT: f32 = 120.0;

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
}

impl Default for State {
    fn default() -> Self {
        Self {
            sql: "SELECT 1 AS valor".to_string(),
            output_directory: String::new(),
            selected: Vec::new(),
            phase: Phase::Idle,
            progress: Vec::new(),
            header: Vec::new(),
            rows: Vec::new(),
            total_rows: 0,
            files: Vec::new(),
            summary: None,
            error: None,
            confirmation: None,
        }
    }
}

impl State {
    /// Estado com o diretório padrão do cadastro.
    pub fn with_output_directory(output_directory: String) -> Self {
        Self {
            output_directory,
            ..Self::default()
        }
    }

    /// Ajusta a seleção quando a lista de servidores muda (mantém o que ainda existe).
    pub fn sync_servers(&mut self, servers: &[ServerConfigEntry]) {
        self.selected
            .retain(|name| servers.iter().any(|server| &server.name == name));
        if self.selected.is_empty() {
            // Nada marcado e servidor único é o caso comum: já vem selecionado, como no CLI.
            if let Some(only) = servers.first() {
                self.selected.push(only.name.clone());
            }
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

    /// Desenha a tela e devolve o pedido, quando houver.
    pub fn ui(&mut self, ui: &mut egui::Ui, servers: &[ServerConfigEntry]) -> Option<Request> {
        let mut request = None;
        let running = self.is_running();

        ui.label("SQL");
        ui.add(
            egui::TextEdit::multiline(&mut self.sql)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );

        ui.horizontal(|ui| {
            ui.label("Output directory");
            ui.add(egui::TextEdit::singleline(&mut self.output_directory).desired_width(360.0));
        });

        ui.separator();
        ui.label("Servers");
        // Lista curta e rolável: o espaço vertical é da **tabela**, que é a saída do comando.
        egui::ScrollArea::vertical()
            .id_salt("query_servers")
            .max_height(96.0)
            .show(ui, |ui| {
                for server in servers {
                    let mut checked = self.selected.contains(&server.name);
                    if ui
                        .checkbox(
                            &mut checked,
                            format!("{} ({}:{})", server.name, server.host, server.port),
                        )
                        .changed()
                    {
                        if checked {
                            self.selected.push(server.name.clone());
                        } else {
                            self.selected.retain(|name| name != &server.name);
                        }
                    }
                }
            });

        ui.separator();
        // O `horizontal` devolve o pedido em vez de escrever no `request` de fora: um `Option<Request>`
        // não é `Copy`, e atribuir dentro do closure o moveria para lá.
        let run_request = ui
            .horizontal(|ui| {
                let chosen: Vec<ServerConfigEntry> = servers
                    .iter()
                    .filter(|server| self.selected.contains(&server.name))
                    .cloned()
                    .collect();

                if ui
                    .add_enabled(
                        !running && !chosen.is_empty(),
                        egui::Button::new("Run query"),
                    )
                    .clicked()
                {
                    // A decisão de pedir confirmação fica numa função só, usada pelo botão e
                    // testável sem janela.
                    self.request_run(chosen.clone())
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

        if !self.rows.is_empty() {
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
            self.results_table(ui);
        } else {
            // Sem linhas, o espaço restante é da tabela do mesmo jeito: a janela não precisa
            // reorganizar o layout entre um quadro e outro.
            ui.allocate_space(ui.available_size());
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
    fn request_run(&mut self, chosen: Vec<ServerConfigEntry>) -> Option<Request> {
        let run = Request::RunQuery {
            servers: chosen.clone(),
            sql: self.sql.clone(),
            output_directory: self.output_directory.clone(),
        };

        if analyzer::analyze(&self.sql) != QueryType::Destructive {
            return Some(run);
        }

        self.confirmation = Some(Confirmation {
            description: analyzer::description(&self.sql).to_string(),
            servers: chosen.len(),
            databases: chosen
                .iter()
                .map(|server| {
                    // O legado conta 1 por servidor quando vai descobrir os bancos.
                    if server.fetch_all_databases {
                        1
                    } else {
                        server.databases.len().max(1)
                    }
                })
                .sum(),
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
    fn results_table(&mut self, ui: &mut egui::Ui) {
        use egui_extras::{Column, TableBuilder};

        let header = self.header.clone();
        let row_height = egui::TextStyle::Body.resolve(ui.style()).size + 6.0;
        // Lido **antes** do `TableBuilder`: ele toma `&mut Ui` emprestado.
        let height = ui.available_height().max(MIN_TABLE_HEIGHT);

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
            .max_scroll_height(height);

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
    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            ..ServerConfigEntry::default()
        }
    }

    #[test]
    fn a_single_server_starts_selected_like_the_cli() {
        let mut state = State::default();

        state.sync_servers(&[server("dev")]);

        assert_eq!(state.selected, vec!["dev".to_string()]);
    }

    #[test]
    fn the_selection_drops_servers_that_went_away() {
        let mut state = State::default();
        state.sync_servers(&[server("dev"), server("outro")]);
        state.selected.push("outro".to_string());

        state.sync_servers(&[server("dev")]);

        assert_eq!(state.selected, vec!["dev".to_string()]);
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

        let request = state.request_run(vec![server("dev")]);

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

        let request = state.request_run(vec![server("dev")]);

        assert!(matches!(request, Some(Request::RunQuery { .. })));
        assert!(state.confirmation.is_none());
    }

    #[test]
    fn a_server_that_discovers_databases_counts_as_one() {
        let mut state = State {
            sql: "DROP TABLE t".to_string(),
            ..State::default()
        };
        let auto = ServerConfigEntry {
            fetch_all_databases: true,
            databases: vec!["a".to_string(), "b".to_string()],
            ..server("dev")
        };

        let _ = state.request_run(vec![auto]);

        assert_eq!(
            state.confirmation.expect("abriu").databases,
            1,
            "quem descobre os bancos conta 1, como no CLI"
        );
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
        let _ = state.request_run(vec![server("dev")]);
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
