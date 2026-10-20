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
use mustel_core::query::csv_writer::parse_records;

use crate::request::{JobResult, Request};

/// Teto de linhas carregadas para a tabela. O arquivo não é truncado — só a leitura da tela.
pub const MAX_DISPLAY_ROWS: usize = 20_000;

/// Altura de uma linha da tabela (a da fonte monoespaçada do `egui`).
const ROW_HEIGHT: f32 = 18.0;

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
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );

        ui.horizontal(|ui| {
            ui.label("Output directory");
            ui.add(egui::TextEdit::singleline(&mut self.output_directory).desired_width(360.0));
        });

        ui.separator();
        ui.label("Servers");
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

        ui.separator();
        ui.horizontal(|ui| {
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
                request = Some(Request::RunQuery {
                    servers: chosen,
                    sql: self.sql.clone(),
                    output_directory: self.output_directory.clone(),
                });
            }
            if running && ui.button("Cancel").clicked() {
                self.phase = Phase::Idle;
                request = Some(Request::CancelJob);
            }
            if running {
                ui.spinner();
            }
            if let Some(summary) = &self.summary {
                ui.label(summary);
            }
        });

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
        }

        request
    }

    /// A tabela **virtualizada**: só as linhas visíveis são desenhadas.
    fn results_table(&mut self, ui: &mut egui::Ui) {
        // Largura fixa por coluna, calculada do cabeçalho e das primeiras linhas: é o que mantém a
        // tabela legível sem medir cada célula de cada quadro.
        let widths: Vec<f32> = self
            .header
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let widest = self
                    .rows
                    .iter()
                    .take(200)
                    .map(|row| row.get(index).map_or(0, |cell| cell.chars().count()))
                    .chain(std::iter::once(name.chars().count()))
                    .max()
                    .unwrap_or(8);
                (widest.clamp(4, 40) as f32) * 7.5
            })
            .collect();

        let header = self.header.clone();
        egui::ScrollArea::horizontal()
            .id_salt("results_h")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, name) in header.iter().enumerate() {
                        let width = widths.get(index).copied().unwrap_or(80.0);
                        ui.add_sized(
                            [width, ROW_HEIGHT],
                            egui::Label::new(egui::RichText::new(name).strong()).truncate(),
                        );
                    }
                });

                let rows = &self.rows;
                egui::ScrollArea::vertical()
                    .id_salt("results_v")
                    .max_height(240.0)
                    .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                        for index in range {
                            let row = &rows[index];
                            ui.horizontal(|ui| {
                                for (column, cell) in row.iter().enumerate() {
                                    let width = widths.get(column).copied().unwrap_or(80.0);
                                    ui.add_sized(
                                        [width, ROW_HEIGHT],
                                        egui::Label::new(egui::RichText::new(cell).monospace())
                                            .truncate(),
                                    );
                                }
                            });
                        }
                    });
            });
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
}
