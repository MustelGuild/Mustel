//! O cadastro de **seletores** (as queries de seleção de bases), na aba Connections.
//!
//! O seletor é **global**: ele lê de um servidor (`Reads from`), mas o que ele marca pode cair em
//! qualquer servidor do cadastro — quem decide os alvos são as linhas que a query devolve. Por isso
//! ele não mora dentro de um servidor; mora ao lado deles, na segunda sub-aba de Connections.
//!
//! Como as outras telas, esta não fala com o núcleo: desenha e devolve
//! [`Request`]s. Quem executa é o [`App`](crate::app::App).

use std::collections::BTreeMap;

use mustel_core::config::ServerConfigEntry;
use mustel_core::config::model::{GuiPrefs, SelectionQuery};

use crate::request::Request;
use crate::sql_editor;

/// O que já se sabe das bases de um servidor.
///
/// O `⟳` do campo `Database` precisa mostrar cada desfecho: era um botão que, quando a descoberta
/// falhava, não escrevia nada e parecia morto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// Pedido emitido, resposta ainda não chegou.
    Loading,
    /// Bases do servidor, em ordem.
    Ready(Vec<String>),
    /// Não deu para listar (a mensagem já vem pronta do núcleo).
    Failed(String),
}

/// Estado do cadastro de seletores.
#[derive(Debug, Default)]
pub struct State {
    /// Queries salvas.
    selections: Vec<SelectionQuery>,
    /// Formulário aberto, quando houver (vive na área principal).
    form: Option<Form>,
    /// Nome da query marcada para remoção (confirmação em dois toques).
    confirming_remove: Option<String>,
    /// Bases descobertas por servidor (o `⟳` do campo Database).
    discovered: BTreeMap<String, Discovery>,
    /// Resultado do último `Test`, pronto para a tela.
    test_result: Option<String>,
    /// Último recado da seção.
    message: Option<String>,
}

/// O formulário de uma query de seleção (só grava no `Save`).
#[derive(Debug, Clone, Default)]
struct Form {
    /// Nome que a query tinha quando o formulário abriu (`None` = nova).
    original: Option<String>,
    /// Campos em edição.
    name: String,
    /// Servidor do cadastro **de onde a query lê** (`Reads from`).
    server: String,
    /// Banco desse servidor.
    database: String,
    /// O SQL.
    sql: String,
    /// Erro de validação do formulário (nome vazio ou repetido).
    error: Option<String>,
}

impl Form {
    /// Formulário de uma query nova, já apontando para o primeiro servidor do cadastro.
    fn new(servers: &[ServerConfigEntry]) -> Self {
        Self {
            server: servers
                .first()
                .map(|server| server.name.clone())
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// Formulário aberto para editar a query existente.
    fn edit(query: &SelectionQuery) -> Self {
        Self {
            original: Some(query.name.clone()),
            name: query.name.clone(),
            server: query.server.clone(),
            database: query.database.clone(),
            sql: query.sql.clone(),
            error: None,
        }
    }

    /// Troca o servidor do `Reads from` e **descarta o banco** escolhido.
    ///
    /// O banco pertence ao servidor: manter o nome ao trocar deixaria o formulário apontando para um
    /// banco que talvez não exista no servidor novo (e o seletor só falharia na hora de rodar, longe
    /// daqui). Escolher o mesmo servidor não muda nada e devolve `false` — a descoberta só é pedida
    /// de novo quando houve troca de verdade.
    fn choose_server(&mut self, chosen: &str) -> bool {
        if chosen == self.server {
            return false;
        }
        self.server = chosen.to_string();
        self.database.clear();
        true
    }

    /// A query como ficou, se o formulário estiver válido.
    ///
    /// O nome **não** é aparado só para exibir: a comparação com os outros é sem diferenciar
    /// maiúsculas e sem espaços nas pontas (mesma regra dos servidores).
    fn to_query(&self) -> Result<SelectionQuery, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("The query needs a name.".to_string());
        }
        if self.server.trim().is_empty() {
            return Err("Pick the server where the query runs.".to_string());
        }
        if self.database.trim().is_empty() {
            return Err("The query needs a database where it runs.".to_string());
        }
        if self.sql.trim().is_empty() {
            return Err("The query needs SQL.".to_string());
        }
        Ok(SelectionQuery {
            name: name.to_string(),
            server: self.server.trim().to_string(),
            database: self.database.trim().to_string(),
            sql: self.sql.clone(),
        })
    }
}

impl State {
    /// Estado com as queries que estavam gravadas.
    pub fn with_prefs(prefs: Option<&GuiPrefs>) -> Self {
        Self {
            selections: prefs
                .map(|prefs| prefs.selections.clone())
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// As queries salvas, para quem lê a lista inteira (a aba Query monta o combo com elas).
    pub fn selections(&self) -> &[SelectionQuery] {
        &self.selections
    }

    /// Aplica o que voltou de um job (descoberta de bases e o `Test`).
    pub fn apply(&mut self, result: &crate::request::JobResult) {
        use crate::request::JobResult;

        match result {
            JobResult::Databases {
                server, databases, ..
            } => {
                let state = match databases {
                    // O combo `Database` escolhe **onde a query roda**: só entra o que dá para
                    // conectar (`template0` recusa conexão).
                    Ok(catalog) => Discovery::Ready(crate::query::usable_names(catalog)),
                    Err(error) => Discovery::Failed(error.clone()),
                };
                self.discovered.insert(server.clone(), state);
            }
            JobResult::SelectionTested { name, outcome } => {
                self.test_result = Some(match outcome {
                    Ok(outcome) => format!("'{name}' — {}", outcome.summary()),
                    Err(error) => format!("'{name}' failed: {error}"),
                });
            }
            _ => {}
        }
    }

    /// A **sidebar** desta sub-aba: a lista de seletores.
    ///
    /// `servers` entra aqui só para o formulário novo já nascer apontando para o primeiro servidor
    /// do cadastro — o mesmo comportamento que a seção tinha na aba Settings.
    pub fn sidebar_ui(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
    ) -> Option<Request> {
        let mut request = None;

        ui.horizontal(|ui| {
            if ui.button("Add query").clicked() {
                // O formulário novo precisa das bases do servidor de origem: pedir já a descoberta
                // evita o primeiro clique num combo vazio.
                self.form = Some(Form::new(servers));
                self.confirming_remove = None;
                self.test_result = None;
            }
        });
        if let Some(message) = &self.message {
            ui.weak(message);
        }

        ui.separator();
        let selections = self.selections.clone();
        egui::ScrollArea::vertical()
            .id_salt("selectors")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for query in selections {
                    let frame = egui::Frame::group(ui.style());
                    let margins = frame.total_margin().sum().x;
                    let width = (ui.available_width() - margins).max(0.0);
                    frame.show(ui, |ui| {
                        ui.set_width(width);
                        ui.label(&query.name);
                        // O seletor continua no cadastro mesmo sem a fonte; mas não roda, e a
                        // linha diz isso — esconder deixaria o dono sem saber por que falha.
                        match crate::selection::missing_source(&query, servers) {
                            Some(missing) => {
                                ui.horizontal(|ui| {
                                    crate::icons::inline_colored(
                                        ui,
                                        crate::icons::Icon::Warning,
                                        crate::theme::warning(ui),
                                    );
                                    ui.colored_label(
                                        crate::theme::warning(ui),
                                        format!("source server '{missing}' is gone"),
                                    );
                                });
                            }
                            None => {
                                ui.weak(format!("{} · {}", query.server, query.database));
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Edit").clicked() {
                                self.form = Some(Form::edit(&query));
                                self.confirming_remove = None;
                                self.test_result = None;
                            }
                            if ui.button("Test").clicked() {
                                request = Some(Request::TestSelection(query.clone()));
                            }

                            let removing = self.confirming_remove.as_deref() == Some(&query.name);
                            let label = if removing { "Confirm remove" } else { "Remove" };
                            if ui.button(label).clicked() && self.tap_remove(&query.name) {
                                request = Some(Request::SaveSelections);
                            }
                        });
                    });
                }
            });

        request
    }

    /// Abre o formulário de uma query nova (usado pelo teste da troca de contexto).
    #[cfg(test)]
    pub fn open_new_for_test(&mut self, servers: &[ServerConfigEntry]) {
        self.form = Some(Form::new(servers));
    }

    /// Há formulário aberto? (a sub-aba precisa saber para não perder o rascunho)
    pub fn has_form(&self) -> bool {
        self.form.is_some()
    }

    /// A **área principal**: o formulário do seletor e o resultado do `Test`.
    pub fn ui(&mut self, ui: &mut egui::Ui, servers: &[ServerConfigEntry]) -> Option<Request> {
        let mut request = None;

        if self.form.is_none() {
            ui.weak("Pick a selector in the list, or add a new one.");
            if let Some(result) = &self.test_result {
                ui.add_space(4.0);
                ui.weak(result);
            }
            return None;
        }

        // O formulário sai do estado enquanto é desenhado: ele precisa de `&mut self` para guardar
        // o resultado da descoberta, e o empréstimo não pode durar o desenho inteiro.
        let mut form = self.form.take().expect("acabou de ser verificado");
        let mut save = false;
        let mut cancel = false;

        ui.group(|ui| {
            ui.label(if form.original.is_some() {
                "Edit query"
            } else {
                "New query"
            });

            egui::Grid::new("selector_form")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.name);
                    ui.end_row();

                    // `Reads from`: onde o SQL roda — **não** onde ele marca. O rótulo mudou junto
                    // com a mudança de lugar do cadastro, porque "Server" aqui parecia dizer que o
                    // seletor pertence àquele servidor.
                    ui.label("Reads from");
                    ui.horizontal(|ui| {
                        // Servidor: combo **com busca**, o mesmo dos outros campos de escolha — a
                        // lista cresce e achar o nome rolando não é escolha.
                        let server_items: Vec<crate::search_combo::Item<'_>> = servers
                            .iter()
                            .map(|server| {
                                crate::search_combo::Item::new(
                                    server.name.as_str(),
                                    server.name == form.server,
                                )
                            })
                            .collect();
                        let button = if form.server.is_empty() {
                            "— pick a server —"
                        } else {
                            form.server.as_str()
                        };
                        let changed = crate::search_combo::search_combo_sized(
                            ui,
                            "selector_server",
                            button,
                            &server_items,
                            !server_items.is_empty(),
                            crate::search_combo::Size::default(),
                        )
                        .map(|index| form.choose_server(&servers[index].name))
                        .unwrap_or(false);
                        ui.label("/");
                        let databases = self.ready_databases(&form.server).unwrap_or_default();
                        let items: Vec<crate::search_combo::Item<'_>> = databases
                            .iter()
                            .map(|database| {
                                crate::search_combo::Item::new(
                                    database.as_str(),
                                    database == &form.database,
                                )
                            })
                            .collect();
                        if let Some(index) = crate::search_combo::search_combo(
                            ui,
                            "selector_database",
                            &form.database,
                            &items,
                            !form.server.is_empty(),
                        ) {
                            form.database = databases[index].clone();
                        }

                        let can_discover = !form.server.is_empty();
                        let loading = self.is_loading(&form.server);
                        if crate::icons::button_enabled(
                            ui,
                            crate::icons::Icon::Refresh,
                            can_discover && !loading,
                        )
                        .on_hover_text("List the databases of this server")
                        .clicked()
                        {
                            self.discovered
                                .insert(form.server.clone(), Discovery::Loading);
                            request = Some(Request::DiscoverDatabases {
                                server: Some(form.server.clone()),
                            });
                        }
                        if loading {
                            ui.spinner();
                        }

                        // Escolher o servidor **já carrega** as bases.
                        if changed && let Some(discover) = self.ensure_discovered(&form.server) {
                            request = Some(discover);
                        }
                    });
                    ui.end_row();

                    ui.label("");
                    ui.vertical(|ui| match self.discovered.get(&form.server) {
                        Some(Discovery::Failed(error)) => {
                            ui.colored_label(
                                crate::theme::warning(ui),
                                format!("Could not list the databases: {error}"),
                            );
                        }
                        Some(Discovery::Ready(databases)) if !databases.is_empty() => {
                            ui.weak(format!(
                                "{} database(s) on '{}'. Use the arrow to pick one.",
                                databases.len(),
                                form.server
                            ));
                        }
                        _ => {}
                    });
                    ui.end_row();

                    ui.label("SQL");
                    ui.vertical(|ui| {
                        // O realce é o mesmo do editor da aba Query — e aqui o editor tem a largura
                        // toda da área principal.
                        let rows = sql_editor::desired_rows(&form.sql);
                        let mut layouter =
                            |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                                let mut job =
                                    sql_editor::layout_job(ui, buffer.as_str(), wrap_width);
                                job.wrap.max_width = wrap_width;
                                ui.fonts_mut(|fonts| fonts.layout_job(job))
                            };

                        ui.add(
                            egui::TextEdit::multiline(&mut form.sql)
                                .id_salt("selector_sql")
                                .desired_rows(rows)
                                .desired_width(f32::INFINITY)
                                .layouter(&mut layouter),
                        );
                    });
                    ui.end_row();
                });

            if let Some(error) = &form.error {
                ui.colored_label(crate::theme::warning(ui), error);
            }

            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

        if let Some(result) = &self.test_result {
            ui.add_space(4.0);
            ui.weak(result);
        }

        if save {
            self.form = Some(form.clone());
            if self.commit_form(&form) {
                // A tela **não** monta a seção `gui`: ela só conhece os seletores, e gravar o que
                // ela conhece apagaria a aparência (era o cuidado que existia em Settings). Quem
                // compõe é o app, que tem as duas telas.
                request = request.or(Some(Request::SaveSelections));
            }
        } else if cancel {
            self.form = None;
            self.test_result = None;
            self.message = None;
        } else {
            self.form = Some(form);
        }

        request
    }

    /// As bases conhecidas do servidor, quando a descoberta já terminou bem.
    fn ready_databases(&self, server: &str) -> Option<Vec<String>> {
        match self.discovered.get(server) {
            Some(Discovery::Ready(databases)) => Some(databases.clone()),
            _ => None,
        }
    }

    /// A descoberta desse servidor está em andamento?
    fn is_loading(&self, server: &str) -> bool {
        matches!(self.discovered.get(server), Some(Discovery::Loading))
    }

    /// Pede a descoberta do servidor se ainda não se sabe das bases dele.
    fn ensure_discovered(&mut self, server: &str) -> Option<Request> {
        if server.is_empty() || self.discovered.contains_key(server) {
            return None;
        }
        self.discovered
            .insert(server.to_string(), Discovery::Loading);
        Some(Request::DiscoverDatabases {
            server: Some(server.to_string()),
        })
    }

    /// Guarda o `Save` do formulário na lista — recusando nome vazio ou repetido.
    ///
    /// Devolve `true` quando a lista mudou (o app grava a seção `gui` inteira).
    fn commit_form(&mut self, form: &Form) -> bool {
        let query = match form.to_query() {
            Ok(query) => query,
            Err(error) => {
                if let Some(open) = &mut self.form {
                    open.error = Some(error);
                }
                return false;
            }
        };

        // Nome repetido é recusado **em vez de** sobrescrever: com dois nomes iguais, o combo não
        // consegue dizer qual query está em uso.
        let clashes = self.selections.iter().any(|existing| {
            existing.name.eq_ignore_ascii_case(&query.name)
                && form
                    .original
                    .as_deref()
                    .is_none_or(|original| !existing.name.eq_ignore_ascii_case(original))
        });
        if clashes {
            if let Some(open) = &mut self.form {
                open.error = Some(format!("There is already a query named '{}'.", query.name));
            }
            return false;
        }

        match form.original.as_deref() {
            // Editar: o nome original sai mesmo que o novo seja outro (renomear funciona).
            Some(original) => {
                let position = self
                    .selections
                    .iter()
                    .position(|existing| existing.name.eq_ignore_ascii_case(original));
                match position {
                    Some(index) => self.selections[index] = query.clone(),
                    None => self.selections.push(query.clone()),
                }
            }
            None => self.selections.push(query.clone()),
        }

        self.form = None;
        self.message = Some(format!("Query '{}' saved.", query.name));
        self.test_result = None;
        true
    }

    /// O clique em `Remove`: o primeiro **arma**, o segundo remove.
    ///
    /// Devolve `true` quando a query saiu da lista. Sem os dois toques, um clique perdido apagaria
    /// uma query que talvez seja a única do dono.
    fn tap_remove(&mut self, name: &str) -> bool {
        let armed = self
            .confirming_remove
            .as_deref()
            .is_some_and(|armed| armed.eq_ignore_ascii_case(name));
        if armed {
            self.remove(name);
            true
        } else {
            self.confirming_remove = Some(name.to_string());
            false
        }
    }

    /// Remove a query pelo nome.
    fn remove(&mut self, name: &str) {
        self.selections
            .retain(|query| !query.name.eq_ignore_ascii_case(name));
        self.confirming_remove = None;
        self.message = Some(format!("Query '{name}' removed."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: "127.0.0.1".to_string(),
            username: "postgres".to_string(),
            ..ServerConfigEntry::default()
        }
    }

    fn query(name: &str) -> SelectionQuery {
        SelectionQuery {
            name: name.to_string(),
            server: "configurador".to_string(),
            database: "configurador".to_string(),
            sql: "select host, port, database from tenants".to_string(),
        }
    }

    fn form_for(query: &SelectionQuery) -> Form {
        Form::edit(query)
    }

    /// O formulário novo já aponta para o primeiro servidor do cadastro, e o servidor escolhido
    /// entra na query gravada.
    /// CONTRACT: IF-230
    #[test]
    fn a_new_form_starts_on_the_first_registered_server() {
        let registry = vec![server("configurador"), server("Local")];
        let mut form = Form::new(&registry);
        assert_eq!(form.server, "configurador", "primeiro do cadastro");

        form.name = "Produção".to_string();
        form.database = "configurador".to_string();
        form.sql = "select 1".to_string();

        let saved = form.to_query().expect("válido");
        assert_eq!(saved.server, "configurador");
    }

    /// Sem servidor escolhido, o formulário recusa (o servidor tem de ser do cadastro).
    /// CONTRACT: IF-230
    #[test]
    fn a_form_without_a_server_is_refused() {
        let mut form = Form::new(&[]);
        form.name = "Produção".to_string();
        form.database = "configurador".to_string();
        form.sql = "select 1".to_string();

        assert!(form.to_query().unwrap_err().contains("server"));
    }

    /// Trocar o servidor do `Reads from` **descarta o banco**; reescolher o mesmo, não.
    ///
    /// O banco pertence ao servidor: arrastá-lo para outro deixaria o formulário apontando para um
    /// banco inexistente, e o erro só apareceria na hora de rodar o seletor.
    #[test]
    fn choosing_another_server_drops_the_database() {
        let mut form = Form::new(&[server("Local"), server("Local 2")]);
        form.database = "configurador".to_string();

        assert!(
            !form.choose_server("Local"),
            "o mesmo servidor não muda nada"
        );
        assert_eq!(form.database, "configurador");

        assert!(form.choose_server("Local 2"));
        assert_eq!(form.server, "Local 2");
        assert!(
            form.database.is_empty(),
            "o banco do servidor anterior não vale aqui: {:?}",
            form.database
        );
    }

    /// O `Reads from` usa o **combo com busca** (seta da fonte embutida), como os outros campos de
    /// escolha — não o `ComboBox` do egui.
    #[test]
    fn the_reads_from_row_uses_the_search_combo() {
        let ctx = egui::Context::default();
        let servers = [server("Local"), server("Local 2")];
        let mut state = State::with_prefs(None);
        state.form = Some(Form::new(&servers));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_max_width(900.0);
                    let _ = state.ui(ui, &servers);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        let arrow = crate::icons::Icon::ChevronDown.glyph().to_string();
        let arrows = texts.iter().filter(|text| text.contains(&arrow)).count();
        assert!(
            arrows >= 1,
            "o servidor e o banco são combos com busca (seta embutida): {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("Local")),
            "o servidor atual fica no botão: {texts:?}"
        );
    }

    /// Escolher o servidor **já pede** a descoberta, e não repete o pedido enquanto carrega.
    ///
    /// Era o bug relatado: as bases só apareciam se o dono apertasse o `⟳`, e quando a descoberta
    /// falhava nada era mostrado — o botão parecia morto.
    /// CONTRACT: IF-232
    #[test]
    fn picking_a_server_starts_the_discovery_only_once() {
        let mut state = State::with_prefs(None);

        let request = state.ensure_discovered("Local");
        assert_eq!(
            request,
            Some(Request::DiscoverDatabases {
                server: Some("Local".to_string())
            })
        );
        assert!(state.is_loading("Local"), "a tela mostra o carregamento");

        // Já está carregando: não dispara outro pedido.
        assert_eq!(state.ensure_discovered("Local"), None);

        // Sem servidor escolhido, não há o que descobrir.
        assert_eq!(state.ensure_discovered(""), None);
    }

    /// O desfecho da descoberta — inclusive a **falha** — fica guardado e visível.
    /// CONTRACT: IF-232
    #[test]
    fn a_failed_discovery_is_reported_instead_of_silent() {
        let mut state = State::with_prefs(None);
        state.ensure_discovered("Local");

        state.apply(&crate::request::JobResult::Databases {
            server: "Local".to_string(),
            databases: Err("No password stored for 'Local'".to_string()),
            has_password: false,
        });

        assert!(!state.is_loading("Local"), "parou de carregar");
        assert_eq!(
            state.ready_databases("Local"),
            None,
            "não há lista para escolher"
        );
        assert!(
            matches!(state.discovered.get("Local"), Some(Discovery::Failed(message)) if message.contains("No password")),
            "a mensagem fica guardada para a tela mostrar"
        );

        // Sucesso posterior substitui a falha (o `⟳` serve para tentar de novo).
        state.apply(&crate::request::JobResult::Databases {
            server: "Local".to_string(),
            databases: Ok(crate::selection::catalog(["dev", "test2"])),
            has_password: true,
        });
        assert_eq!(
            state.ready_databases("Local"),
            Some(vec!["dev".to_string(), "test2".to_string()])
        );
    }

    /// O desfecho da descoberta não muda a lista de queries nem é gravado no `Save`.
    /// CONTRACT: IF-232
    #[test]
    fn the_discovered_list_is_not_part_of_the_saved_query() {
        let mut state = State::with_prefs(None);
        state.ensure_discovered("Local");
        state.apply(&crate::request::JobResult::Databases {
            server: "Local".to_string(),
            databases: Ok(crate::selection::catalog(["dev"])),
            has_password: true,
        });

        assert!(state.selections.is_empty(), "nada foi gravado");
    }

    /// Textos pintados no quadro, para conferir o que a tela desenhou sem abrir janela.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => {
                    let content = text.galley.text().trim().to_string();
                    if !content.is_empty() {
                        out.push(content);
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

    /// Sanidade do harness: um `Popup` aberto pinta o conteúdo no segundo quadro?
    ///
    /// Se este falhar, o problema não é o picker e sim o jeito de desenhar popup no teste.
    #[test]
    fn sanity_a_plain_popup_paints_its_content() {
        let ctx = egui::Context::default();
        let mut shown = false;
        let mut texts: Vec<String> = Vec::new();

        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let button = ui.button("open");
                    let inner = egui::Popup::from_response(&button).open(true).show(|ui| {
                        ui.label("CONTEUDO");
                    });
                    shown = inner.is_some();
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(shown, "o popup devolveu InnerResponse");
        assert!(
            texts.iter().any(|text| text == "CONTEUDO"),
            "o conteúdo do popup foi pintado: {texts:?}"
        );
    }

    /// Nome vazio é recusado, e a lista não muda.
    /// CONTRACT: IF-230
    #[test]
    fn an_empty_name_is_refused() {
        let mut state = State::with_prefs(None);
        state.form = Some(Form::default());

        assert!(!state.commit_form(&Form::default()));
        assert!(state.selections.is_empty(), "nada entrou");
        assert!(
            state
                .form
                .as_ref()
                .and_then(|form| form.error.as_deref())
                .is_some_and(|error| error.contains("name")),
            "o formulário continua aberto com o erro"
        );
    }

    /// Nome repetido é recusado **em vez de** sobrescrever.
    /// CONTRACT: IF-230
    #[test]
    fn a_duplicate_name_is_refused_instead_of_overwritten() {
        let mut state = State::with_prefs(None);
        state.form = Some(form_for(&query("Produção")));
        assert!(state.commit_form(&form_for(&query("Produção"))));

        // Mesma query, nome com maiúsculas e espaços diferentes: ainda é o mesmo nome.
        let mut other = query(" produção ");
        other.sql = "select 1".to_string();
        state.form = Some(form_for(&other));
        assert!(!state.commit_form(&form_for(&other)), "recusa");

        assert_eq!(state.selections.len(), 1, "não sobrescreveu nem duplicou");
        assert_eq!(
            state.selections[0].sql,
            query("Produção").sql,
            "a original continua intacta"
        );
    }

    /// Editar preserva o nome original e permite renomear.
    /// CONTRACT: IF-230
    #[test]
    fn editing_keeps_the_original_name_and_allows_renaming() {
        let mut state = State::with_prefs(None);
        let original = query("Produção");
        state.form = Some(form_for(&original));
        assert!(state.commit_form(&form_for(&original)));

        // Editar sem mudar o nome: uma entrada, conteúdo novo.
        let mut edited = original.clone();
        edited.sql = "select * from tenants where ambiente = 'prod'".to_string();
        assert!(state.commit_form(&form_for(&edited)));
        assert_eq!(state.selections.len(), 1, "não duplicou ao editar");
        assert_eq!(state.selections[0].sql, edited.sql);

        // Renomear: o formulário continua com o nome **antigo** como `original` — é o que a tela
        // faz, porque só o campo muda. Substitui a mesma entrada, sem deixar a antiga para trás.
        let mut renamed = form_for(&edited);
        renamed.name = "Produção 2".to_string();
        assert!(state.commit_form(&renamed));
        assert_eq!(state.selections.len(), 1);
        assert_eq!(state.selections[0].name, "Produção 2");
    }

    /// Remover exige **dois** toques.
    /// CONTRACT: IF-230
    #[test]
    fn removing_needs_two_taps() {
        let mut state = State::with_prefs(None);
        let saved = query("Produção");
        state.form = Some(form_for(&saved));
        assert!(state.commit_form(&form_for(&saved)));

        assert!(!state.tap_remove("Produção"), "o primeiro toque só arma");
        assert_eq!(state.selections.len(), 1, "a query continua lá");
        assert_eq!(state.confirming_remove.as_deref(), Some("Produção"));

        assert!(state.tap_remove("Produção"), "o segundo remove");
        assert!(state.selections.is_empty());
        assert_eq!(state.confirming_remove, None, "o alarme desarma");

        // Tocar em **outra** query não aproveita a confirmação da primeira.
        state.form = Some(form_for(&query("A")));
        assert!(state.commit_form(&form_for(&query("A"))));
        state.form = Some(form_for(&query("B")));
        assert!(state.commit_form(&form_for(&query("B"))));
        assert!(!state.tap_remove("A"));
        assert!(!state.tap_remove("B"), "armar B desarma A");
        assert_eq!(state.selections.len(), 2);
    }

    /// O `Test` mostra números e o erro agregado, e **nunca** a senha (D12).
    /// CONTRACT: IF-231
    #[test]
    fn the_test_result_shows_counts_and_never_the_password() {
        let mut state = State::with_prefs(None);

        state.apply(&crate::request::JobResult::SelectionTested {
            name: "Produção".to_string(),
            outcome: Ok(crate::query::SelectionOutcome {
                servers: 2,
                databases: 12,
                blocked: 3,
                warnings: 1,
                ignored_rows: 4,
            }),
        });
        let text = state.test_result.clone().expect("tem resultado");
        assert!(text.contains("2 server(s)"), "servidores: {text}");
        assert!(text.contains("12 database(s)"), "bases: {text}");
        assert!(text.contains("3 blocked"), "excluídas: {text}");
        assert!(text.contains("4 row(s) ignored"), "lixo contado: {text}");

        // Erro agregado: a mensagem do núcleo aparece, o segredo não.
        state.apply(&crate::request::JobResult::SelectionTested {
            name: "Produção".to_string(),
            outcome: Err("2 host(s) not registered (3 row(s)): 10.0.0.9, 10.0.0.7".to_string()),
        });
        let text = state.test_result.clone().expect("tem erro");
        assert!(text.contains("10.0.0.9"), "lista o host: {text}");
        assert!(!text.contains("segredo"));
        assert!(!text.to_lowercase().contains("password="));
    }
}
