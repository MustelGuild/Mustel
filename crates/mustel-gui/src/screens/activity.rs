//! Settings › Activity: o que rodou e o que falhou, e o relatório anonimizado.
//!
//! A aba lê o [`Log`] que o `App` mantém (store + entradas dos 30 dias). O relatório é montado do
//! **disco** na abertura do modal (pega também execuções do CLI feitas com a janela aberta) e a
//! prévia mostra **todo** o conteúdo que será enviado — o corpo padrão e o anexo —, sem truncar.

use std::path::{Path, PathBuf};

use mustel_core::activity::{
    AppInfo, Entry, RedactContext, Redactor, Replacement, Status, op_label, render,
};
use mustel_core::config::ServerConfigEntry;
use mustel_core::config::model::AiProfile;
use mustel_core::datetime::{format_utc, format_utc_compact, now_ms};

use crate::activity_log::Log;

/// O contexto que o `App` entrega à aba.
pub(crate) struct Context<'a> {
    /// O log da janela.
    pub log: &'a mut Log,
    /// O cadastro (para a legenda da anonimização).
    pub servers: &'a [ServerConfigEntry],
    /// Os perfis de IA (idem).
    pub profiles: &'a [AiProfile],
    /// Onde os `.log` são salvos.
    pub reports_dir: &'a Path,
}

/// O filtro da tabela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Filter {
    /// Tudo.
    #[default]
    All,
    /// Só o que falhou.
    Errors,
}

/// O que o modal pode pedir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    Send,
    Cancel,
}

/// O estado da aba: filtro, seleção, modal e a última dica.
#[derive(Debug, Default)]
pub(crate) struct State {
    filter: Filter,
    selected: Option<usize>,
    report: Option<ReportState>,
    hint: Option<String>,
}

/// O modal de envio já montado.
#[derive(Debug)]
struct ReportState {
    /// O corpo do e-mail (mensagem padrão, editável).
    body: String,
    /// O texto do `.log` (o anexo).
    log: String,
    /// Os pares reais → token, só para a legenda da tela.
    legend: Vec<Replacement>,
    /// O anexo está em modo de edição?
    editing: bool,
    /// O último arquivo salvo.
    saved: Option<PathBuf>,
}

/// Altura reservada ao detalhe da linha selecionada.
const DETAIL_HEIGHT: f32 = 190.0;

impl State {
    /// Desenha a aba inteira.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, context: &mut Context<'_>) {
        let report_requested = self.toolbar(ui, context);
        self.hints(ui, context);

        {
            let entries = context.log.entries();
            self.table_and_detail(ui, entries);
        }

        if report_requested {
            self.open_report(context);
        }

        self.report_modal(ui, context.reports_dir);
    }

    /// A barra de ações. Devolve `true` quando `Report…` foi clicado.
    fn toolbar(&mut self, ui: &mut egui::Ui, context: &mut Context<'_>) -> bool {
        let mut report = false;

        ui.horizontal(|ui| {
            let filter = self.filter;
            ui.selectable_value(&mut self.filter, Filter::All, "All");
            ui.selectable_value(&mut self.filter, Filter::Errors, "Errors");
            if self.filter != filter {
                self.selected = None;
            }

            ui.separator();
            if ui.button("Refresh").clicked() {
                context.log.reload();
                self.selected = None;
            }
            if ui.button("Open folder").clicked() {
                match mustel_core::activity::log_dir() {
                    Some(dir) => {
                        if let Err(error) = crate::desktop::open_directory(&dir) {
                            self.hint = Some(format!("Could not open the log folder: {error}"));
                        }
                    }
                    None => {
                        self.hint =
                            Some("The activity log is not available on this system.".to_string());
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Report…").clicked() {
                    report = true;
                }
            });
        });

        report
    }

    /// O aviso do log e a última dica.
    fn hints(&self, ui: &mut egui::Ui, context: &Context<'_>) {
        if let Some(error) = context.log.error() {
            ui.colored_label(crate::theme::danger(ui), error);
        }
        if let Some(hint) = &self.hint {
            ui.weak(hint);
        }
    }

    /// A tabela virtualizada e o detalhe da linha selecionada.
    fn table_and_detail(&mut self, ui: &mut egui::Ui, entries: &[Entry]) {
        let indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.filter == Filter::All || entry.status == Status::Error)
            .map(|(index, _)| index)
            .rev()
            .collect();

        ui.label(format!("{} of {} entries", indices.len(), entries.len()));

        if entries.is_empty() {
            ui.add_space(8.0);
            ui.weak("Nothing ran yet. Queries, tests and errors will show up here.");
            return;
        }
        if indices.is_empty() {
            ui.add_space(8.0);
            ui.weak("No errors recorded.");
            return;
        }

        let table_height = (ui.available_height() - DETAIL_HEIGHT).max(120.0);
        let row_height = egui::TextStyle::Body.resolve(ui.style()).size + 6.0;
        let selected = self.selected;
        let mut clicked = None;

        {
            use egui_extras::{Column, TableBuilder};

            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .max_scroll_height(table_height)
                .column(Column::auto().at_least(140.0).clip(false))
                .column(Column::auto().at_least(70.0).clip(false))
                .column(Column::auto().at_least(120.0).clip(false))
                .column(Column::auto().at_least(70.0).clip(false))
                .column(Column::auto().at_least(100.0).clip(false))
                .column(Column::remainder().at_least(80.0));

            table
                .header(row_height, |mut row| {
                    for name in [
                        "TIME (UTC)",
                        "AREA",
                        "OPERATION",
                        "RESULT",
                        "TARGET",
                        "DETAIL",
                    ] {
                        row.col(|ui| {
                            ui.label(egui::RichText::new(name).strong());
                        });
                    }
                })
                .body(|body| {
                    body.rows(row_height, indices.len(), |mut row| {
                        let index = indices[row.index()];
                        let entry = &entries[index];
                        row.set_selected(selected == Some(index));

                        row.col(|ui| {
                            ui.label(format_utc(entry.at_ms));
                        });
                        row.col(|ui| {
                            ui.label(entry.area.label());
                        });
                        row.col(|ui| {
                            ui.label(op_label(&entry.op));
                        });
                        row.col(|ui| {
                            ui.label(status_text(ui, entry.status));
                        });
                        row.col(|ui| {
                            ui.label(entry.target.as_deref().unwrap_or("—"));
                        });
                        row.col(|ui| {
                            ui.label(detail_text(entry));
                        });

                        if row.response().clicked() {
                            clicked = Some(index);
                        }
                    });
                });
        }

        if let Some(index) = clicked {
            self.selected = Some(index);
        }

        egui::ScrollArea::vertical()
            .id_salt("activity_detail")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| self.detail_ui(ui, entries, &indices));
    }

    /// O detalhe da entrada selecionada.
    fn detail_ui(&mut self, ui: &mut egui::Ui, entries: &[Entry], indices: &[usize]) {
        ui.separator();

        let Some(index) = self.selected else {
            ui.weak("Select a row to see the details.");
            return;
        };
        if index >= entries.len() || !indices.contains(&index) {
            self.selected = None;
            ui.weak("Select a row to see the details.");
            return;
        }

        let entry = &entries[index];
        ui.horizontal(|ui| {
            ui.strong(op_label(&entry.op));
            ui.weak(format!(
                "{} · {} · {} UTC",
                entry.area.label(),
                entry.status.label(),
                format_utc(entry.at_ms)
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(entry_summary(entry));
                }
            });
        });

        if let Some(target) = &entry.target {
            ui.label(format!("Target: {target}"));
        }
        let detail = detail_text(entry);
        if !detail.is_empty() {
            ui.label(detail);
        }
        if let Some(error) = &entry.error {
            ui.add(
                egui::Label::new(egui::RichText::new(error).color(crate::theme::danger(ui))).wrap(),
            );
        }
        if let Some(sql) = &entry.sql {
            ui.add_space(2.0);
            ui.weak("SQL (local only)");
            ui.add(egui::Label::new(egui::RichText::new(sql).monospace()).wrap());
        }
    }

    /// Monta o relatório a partir do disco e abre o modal.
    fn open_report(&mut self, context: &mut Context<'_>) {
        context.log.reload();
        let entries = context.log.entries();
        let home = mustel_core::activity::home_dir();
        let config = mustel_core::config::paths::config_dir();
        let redactor = Redactor::new(
            &RedactContext {
                servers: context.servers,
                profiles: context.profiles,
                home: home.as_deref(),
                config: config.as_deref(),
            },
            entries,
        );
        let report = render(entries, &redactor, &AppInfo::current(), now_ms());

        self.report = Some(ReportState {
            body: default_body(),
            log: report.text,
            legend: report.legend,
            editing: false,
            saved: None,
        });
        self.hint = None;
    }

    /// O modal com a prévia completa e as ações.
    fn report_modal(&mut self, ui: &mut egui::Ui, reports_dir: &Path) {
        if self.report.is_none() {
            return;
        }

        let mut action = None;
        egui::Modal::new(egui::Id::unique("activity_report")).show(ui.ctx(), |ui| {
            let report = self.report.as_mut().expect("verificado acima");
            ui.set_width(840.0);
            ui.heading("Send report");
            ui.label(
                "This goes to support@mustelguild.dev. Review everything below: the message and \
                 the attached .log are exactly what leaves this computer.",
            );

            ui.add_space(4.0);
            // O corpo rola; o rodapé fica **fixo**, sempre visível. O teto cabe na altura mínima
            // da janela (560) com o cabeçalho e o rodapé.
            let max_height = 380.0;
            egui::ScrollArea::vertical()
                .id_salt("activity_report_body")
                .auto_shrink([false, false])
                .max_height(max_height)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Message").strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut report.body)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Attachment — the .log").strong());
                        let toggle = if report.editing {
                            "Done editing"
                        } else {
                            "Edit"
                        };
                        if ui.button(toggle).clicked() {
                            report.editing = !report.editing;
                        }
                    });

                    if report.editing {
                        ui.add(
                            egui::TextEdit::multiline(&mut report.log)
                                .code_editor()
                                .desired_rows(18)
                                .desired_width(f32::INFINITY),
                        );
                    } else {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&report.log).monospace()).wrap(),
                        );
                    }

                    egui::CollapsingHeader::new("What was anonymized (not sent)")
                        .default_open(true)
                        .show(ui, |ui| {
                            if report.legend.is_empty() {
                                ui.weak("Nothing needed replacing.");
                            } else {
                                for replacement in &report.legend {
                                    ui.label(format!(
                                        "{}: {} → {}",
                                        replacement.what, replacement.from, replacement.to
                                    ));
                                }
                            }
                            ui.weak(
                                "These values stay on this computer. SQL is omitted; URLs, \
                                 e-mails, IPs, paths and quoted terms are replaced by <…> in the \
                                 report.",
                            );
                        });
                });

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx()
                        .copy_text(format!("{}\n\n{}", report.body, report.log));
                }
                if ui.button("Save").clicked() {
                    action = Some(Action::Save);
                }
                if ui.button("Send").clicked() {
                    action = Some(Action::Send);
                }
                if let Some(path) = &report.saved {
                    ui.weak(format!("Saved to {}", path.display()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        action = Some(Action::Cancel);
                    }
                });
            });
        });

        match action {
            Some(Action::Save) => {
                self.save_report(reports_dir);
            }
            Some(Action::Send) => self.send_report(reports_dir),
            Some(Action::Cancel) => self.report = None,
            None => {}
        }
    }

    /// Grava o texto da prévia como `.log` e devolve o caminho.
    fn save_report(&mut self, reports_dir: &Path) -> Option<PathBuf> {
        let (text, file_name) = {
            let report = self.report.as_ref()?;
            (
                report.log.clone(),
                format!("mustel-report-{}.log", format_utc_compact(now_ms())),
            )
        };
        let path = reports_dir.join(file_name);

        if let Err(error) =
            std::fs::create_dir_all(reports_dir).and_then(|()| std::fs::write(&path, &text))
        {
            self.hint = Some(format!("Could not save the report: {error}"));
            return None;
        }

        if let Some(report) = self.report.as_mut() {
            report.saved = Some(path.clone());
        }
        Some(path)
    }

    /// Salva, abre o cliente de e-mail e fecha o modal.
    fn send_report(&mut self, reports_dir: &Path) {
        let Some(path) = self.save_report(reports_dir) else {
            return;
        };
        let (subject, body) = {
            let Some(report) = self.report.as_ref() else {
                return;
            };
            (
                format!("Mustel {} report", env!("CARGO_PKG_VERSION")),
                report.body.clone(),
            )
        };

        match crate::desktop::compose_email("support@mustelguild.dev", &subject, &body, Some(&path))
        {
            Ok(()) => {
                self.hint = Some(format!(
                    "Report saved to {}. Attach it if your mail app did not.",
                    path.display()
                ));
                self.report = None;
            }
            Err(error) => {
                self.hint = Some(format!(
                    "Could not open the e-mail app: {error}. The report is at {}.",
                    path.display()
                ));
            }
        }
    }
}

/// O texto do status colorido.
fn status_text(ui: &egui::Ui, status: Status) -> egui::RichText {
    let text = egui::RichText::new(status.label());
    match status {
        Status::Ok => text.color(crate::theme::success(ui)),
        Status::Error => text.color(crate::theme::danger(ui)),
        Status::Cancelled => text.color(crate::theme::warning(ui)),
        Status::Info => text.weak(),
    }
}

/// `detalhe · duração`, quando houver.
fn detail_text(entry: &Entry) -> String {
    let mut detail = entry.detail.clone().unwrap_or_default();
    if let Some(duration_ms) = entry.duration_ms {
        if !detail.is_empty() {
            detail.push_str(" · ");
        }
        detail.push_str(&format!("{duration_ms} ms"));
    }
    detail
}

/// Um resumo de uma linha para o `Copy` do detalhe.
fn entry_summary(entry: &Entry) -> String {
    let mut summary = format!(
        "{} UTC  {}  {}  {}",
        format_utc(entry.at_ms),
        entry.area.label(),
        op_label(&entry.op),
        entry.status.label()
    );
    if let Some(target) = &entry.target {
        summary.push_str(&format!("  target: {target}"));
    }
    if let Some(error) = &entry.error {
        summary.push('\n');
        summary.push_str(error);
    }
    summary
}

/// A mensagem padrão do e-mail (versão + SO), editável no modal.
fn default_body() -> String {
    format!(
        "Mustel {} report\nSystem: {} ({})\nGenerated: {} UTC\n\nThe anonymized activity log is attached.\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        format_utc(now_ms())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::activity::{Area, Entry, Status};

    const AT: u64 = 20_763 * 86_400_000 + 12 * 3_600_000;

    fn entry(op: &str, status: Status, detail: &str) -> Entry {
        Entry::new(Area::Query, op, status)
            .at(AT)
            .with_detail(detail)
    }

    fn log_with(entries: Vec<Entry>) -> Log {
        Log::in_memory(entries)
    }

    fn server() -> ServerConfigEntry {
        ServerConfigEntry {
            name: "Local".to_string(),
            host: "127.0.0.1".to_string(),
            username: "postgres".to_string(),
            ..ServerConfigEntry::default()
        }
    }

    fn painted(
        state: &mut State,
        log: &mut Log,
        servers: &[ServerConfigEntry],
        reports_dir: &Path,
    ) -> Vec<String> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            ..Default::default()
        };
        let mut output = None;
        // Dois quadros: o `Modal` mede o conteúdo no primeiro e desenha no segundo.
        for _ in 0..2 {
            let state = &mut *state;
            let log = &mut *log;
            let mut out = ctx.run_ui(input.clone(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let mut context = Context {
                        log,
                        servers,
                        profiles: &[],
                        reports_dir,
                    };
                    state.ui(ui, &mut context);
                });
            });
            out.textures_delta.clear();
            output = Some(out);
        }
        let output = output.expect("dois quadros");

        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        texts
    }

    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
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

    /// CONTRACT: IF-266
    #[test]
    fn the_table_lists_the_entries() {
        let mut state = State::default();
        let mut log = log_with(vec![entry("query.run", Status::Ok, "42 rows")]);
        let reports = std::env::temp_dir();

        let texts = painted(&mut state, &mut log, &[], &reports);
        let everything = texts.join("\n");

        assert!(everything.contains("Query"), "{everything}");
        assert!(everything.contains("42 rows"), "{everything}");
        assert!(everything.contains("1 of 1 entries"), "{everything}");
    }

    #[test]
    fn the_errors_filter_hides_successes() {
        let mut state = State {
            filter: Filter::Errors,
            ..State::default()
        };
        let mut log = log_with(vec![
            entry("query.run", Status::Ok, "42 rows"),
            entry("connection.test", Status::Error, "refused"),
        ]);
        let reports = std::env::temp_dir();

        let texts = painted(&mut state, &mut log, &[], &reports);
        let everything = texts.join("\n");

        assert!(everything.contains("Connection test"), "{everything}");
        assert!(
            !everything.contains("42 rows"),
            "o sucesso sumiu: {everything}"
        );
        assert!(everything.contains("1 of 2 entries"), "{everything}");
    }

    #[test]
    fn an_empty_log_says_so() {
        let mut state = State::default();
        let mut log = log_with(Vec::new());
        let reports = std::env::temp_dir();

        let texts = painted(&mut state, &mut log, &[], &reports);

        assert!(
            texts.iter().any(|text| text.contains("Nothing ran yet")),
            "{texts:?}"
        );
    }

    #[test]
    fn the_detail_shows_the_selected_entry() {
        let mut state = State {
            selected: Some(1),
            ..State::default()
        };
        let mut log = log_with(vec![
            entry("query.run", Status::Ok, "42 rows"),
            entry("connection.test", Status::Error, "refused"),
        ]);
        let reports = std::env::temp_dir();

        let texts = painted(&mut state, &mut log, &[], &reports);
        let everything = texts.join("\n");

        assert!(everything.contains("refused"), "{everything}");
        assert!(!everything.contains("Select a row"));
    }

    /// CONTRACT: IF-267
    #[test]
    fn the_report_contains_the_whole_log_without_names() {
        let mut state = State::default();
        let mut log = log_with(vec![
            entry("query.run", Status::Ok, "run 0"),
            entry("query.run", Status::Ok, "run 1")
                .with_target("Local")
                .with_sql("select * from clientes"),
        ]);
        let reports = std::env::temp_dir();
        let servers = [server()];
        let mut context = Context {
            log: &mut log,
            servers: &servers,
            profiles: &[],
            reports_dir: &reports,
        };

        state.open_report(&mut context);

        let report = state.report.as_ref().expect("abriu o relatório");
        assert!(report.log.contains("run 0"), "{}", report.log);
        assert!(report.log.contains("run 1"), "{}", report.log);
        assert!(!report.log.contains("Local"), "{}", report.log);
        assert!(!report.log.contains("select *"), "{}", report.log);
        assert!(report.log.contains("server-1"), "{}", report.log);
        assert!(
            report
                .legend
                .iter()
                .any(|replacement| replacement.from == "Local"),
            "a legenda mostra o valor real"
        );
    }

    #[test]
    fn the_preview_paints_the_body_and_the_log() {
        let mut state = State::default();
        let mut log = log_with(vec![entry("query.run", Status::Ok, "42 rows")]);
        let reports = std::env::temp_dir();
        let mut context = Context {
            log: &mut log,
            servers: &[],
            profiles: &[],
            reports_dir: &reports,
        };
        state.open_report(&mut context);

        let texts = painted(&mut state, &mut log, &[], &reports);
        let everything = texts.join("\n");

        assert!(everything.contains("Send report"), "{everything}");
        assert!(everything.contains("Mustel"), "{everything}");
        assert!(everything.contains("42 rows"), "{everything}");
        assert!(everything.contains("What was anonymized"), "{everything}");
    }

    /// CONTRACT: IF-267
    #[test]
    fn saving_writes_exactly_the_preview() {
        let dir = tempfile::tempdir().expect("temporário");
        let mut state = State::default();
        let mut log = log_with(vec![entry("query.run", Status::Ok, "42 rows")]);
        let mut context = Context {
            log: &mut log,
            servers: &[],
            profiles: &[],
            reports_dir: dir.path(),
        };
        state.open_report(&mut context);
        state
            .report
            .as_mut()
            .expect("aberto")
            .log
            .push_str("\nlinha acrescentada pelo usuário\n");

        let path = state.save_report(dir.path()).expect("salvou");
        let written = std::fs::read_to_string(path).expect("lê");

        assert_eq!(
            written,
            state.report.as_ref().expect("aberto").log,
            "o .log é byte a byte a prévia editada"
        );
    }
}
