//! Tela **Settings**: preferências de aparência da janela (hoje, fonte).
//!
//! A tela não grava nada: devolve [`Request::SaveGuiPrefs`] e quem persiste é o app, através de
//! `usecases::gui_prefs` — a mesma separação das outras telas (`Request` → `App` → núcleo).
//!
//! A lista de famílias do sistema é cara de montar (o `fontique` varre o sistema), então é carregada
//! **uma vez**, na primeira vez que a aba abre, e guardada na tela.

use std::collections::BTreeMap;

use mustel_core::config::ServerConfigEntry;
use mustel_core::config::model::{GuiPrefs, MAX_FONT_SCALE, MIN_FONT_SCALE, SelectionQuery};

use crate::fonts::{self, Chosen, EMBEDDED_MONO, EMBEDDED_PROPORTIONAL};
use crate::request::Request;
use crate::sql_editor;

/// O que já se sabe das bases de um servidor.
///
/// O `⟳` era um botão que só escrevia uma linha fraca embaixo do SQL — e, quando a descoberta
/// falhava (servidor fora do ar, senha ausente no cofre), **não escrevia nada**: o botão parecia
/// morto. O estado guarda os três desfechos para a tela poder mostrar cada um.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Discovery {
    /// Pedido emitido, resposta ainda não chegou.
    Loading,
    /// Bases do servidor, em ordem.
    Ready(Vec<String>),
    /// Não deu para listar (a mensagem já vem pronta do núcleo).
    Failed(String),
}

/// Estado da aba Settings.
#[derive(Debug, Default)]
pub struct State {
    /// Escolha em edição (aplicada ao vivo e gravada ao soltar os controles).
    chosen: Chosen,
    /// Queries salvas de seleção de bases (a segunda seção da aba, §4.6).
    selections: Vec<SelectionQuery>,
    /// Formulário de query aberto, quando houver.
    selection_form: Option<SelectionForm>,
    /// Nome da query marcada para remoção (confirmação em dois toques).
    confirming_remove: Option<String>,
    /// Bases descobertas por servidor (o `⟳` do campo Database).
    discovered: BTreeMap<String, Discovery>,
    /// Resultado do último `Test`, pronto para a tela.
    test_result: Option<String>,
    /// Último recado da seção de queries.
    selection_message: Option<String>,
    /// Famílias instaladas no sistema (`None` = ainda não carregadas).
    families: Option<Vec<String>>,
    /// Aviso do que não pôde ser aplicado (fonte ausente).
    warning: Option<String>,
}

/// O formulário de uma query de seleção (só grava no `Save`).
#[derive(Debug, Clone, Default)]
struct SelectionForm {
    /// Nome que a query tinha quando o formulário abriu (`None` = nova).
    original: Option<String>,
    /// Campos em edição.
    name: String,
    /// Servidor do cadastro onde o SQL roda.
    server: String,
    /// Banco desse servidor.
    database: String,
    /// O SQL.
    sql: String,
    /// Texto de busca do dropdown de base (só da tela, não é gravado).
    db_filter: String,
    /// O dropdown de base está aberto?
    ///
    /// O estado é da tela, não da memória do `egui`: popup aberto por memória some no quadro
    /// seguinte se ninguém o mantiver aberto, e consultar a memória *antes* de reabrir fazia o
    /// popup nunca aparecer (era o "botão que não faz nada").
    db_open: bool,
    /// Erro de validação do formulário (nome vazio ou repetido).
    error: Option<String>,
}

impl SelectionForm {
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
            db_filter: String::new(),
            db_open: false,
            error: None,
        }
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
    /// Estado com as preferências que estavam gravadas.
    pub fn with_prefs(prefs: Option<&GuiPrefs>) -> Self {
        Self {
            chosen: Chosen::from_prefs(prefs),
            selections: prefs
                .map(|prefs| prefs.selections.clone())
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// A escolha atual (o app usa para aplicar no contexto).
    pub fn chosen(&self) -> &Chosen {
        &self.chosen
    }

    /// As queries salvas, para quem lê a seção inteira.
    pub fn selections(&self) -> &[SelectionQuery] {
        &self.selections
    }

    /// Aplica o que voltou de um job (descoberta de bases e o `Test`).
    pub fn apply(&mut self, result: &crate::request::JobResult) {
        use crate::request::JobResult;

        match result {
            // Descoberta: guarda o desfecho, inclusive a falha — senão o `⟳` parece não ter feito
            // nada (era o bug relatado).
            JobResult::Databases {
                server, databases, ..
            } => {
                let state = match databases {
                    Ok(databases) => Discovery::Ready(databases.clone()),
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
            // O resto é das outras telas.
            _ => {}
        }
    }

    /// Guarda o `Save` do formulário na lista — recusando nome vazio ou repetido.
    ///
    /// Devolve `true` quando a lista mudou (o app grava a seção `gui` inteira).
    fn commit_form(&mut self, form: &SelectionForm) -> bool {
        let query = match form.to_query() {
            Ok(query) => query,
            Err(error) => {
                if let Some(open) = &mut self.selection_form {
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
            if let Some(open) = &mut self.selection_form {
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

        self.selection_form = None;
        self.selection_message = Some(format!("Query '{}' saved.", query.name));
        self.test_result = None;
        true
    }

    /// Remove a query pelo nome.
    fn remove(&mut self, name: &str) -> bool {
        let before = self.selections.len();
        self.selections
            .retain(|query| !query.name.eq_ignore_ascii_case(name));
        self.confirming_remove = None;
        if self.selections.len() == before {
            return false;
        }
        self.selection_message = Some(format!("Query '{name}' removed."));
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
            self.remove(name)
        } else {
            self.confirming_remove = Some(name.to_string());
            false
        }
    }

    /// A seção `gui` completa do estado: aparência **e** queries salvas.
    ///
    /// Gravar é escrever a seção inteira, então montar só a aparência apagaria as queries.
    pub fn prefs(&self) -> GuiPrefs {
        let mut prefs = GuiPrefs::default();
        self.chosen.apply_to(&mut prefs);
        prefs.selections = self.selections.clone();
        prefs
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
    ///
    /// É o que faz escolher o servidor no formulário já carregar a lista — o `⟳` deixa de ser
    /// obrigatório para ver as bases. Servidor já carregado (ou carregando) não repete o pedido.
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

    /// Guarda um aviso vindo da aplicação (fonte ausente, por exemplo).
    pub fn set_warning(&mut self, warning: Option<String>) {
        self.warning = warning;
    }

    /// Desenha a aba e devolve o pedido de gravação quando algo mudou.
    ///
    /// `servers` é o **cadastro**: a seção de queries de seleção só escolhe entre servidores já
    /// cadastrados (nunca inventa um).
    pub fn ui(&mut self, ui: &mut egui::Ui, servers: &[ServerConfigEntry]) -> Option<Request> {
        ui.heading("Appearance");
        ui.label("These preferences are stored in the `gui` section of `mustel.jsonc`.");
        ui.separator();

        // Carrega as famílias na primeira renderização: enumerar o sistema custa centenas de ms.
        if self.families.is_none() {
            self.families = Some(fonts::system_families());
            // Pede um quadro novo para o combo já aparecer completo.
            ui.ctx().request_repaint();
        }
        let families = self.families.clone().unwrap_or_default();

        let mut changed = false;
        let mut apply_now = false;

        changed |= self.family_combo(
            ui,
            Combo {
                label: "Interface font",
                id: "settings_interface_font",
                embedded: EMBEDDED_PROPORTIONAL,
                which: Family::Interface,
            },
            &families,
            &mut apply_now,
        );
        changed |= self.family_combo(
            ui,
            Combo {
                label: "Editor font",
                id: "settings_editor_font",
                embedded: EMBEDDED_MONO,
                which: Family::Editor,
            },
            &families,
            &mut apply_now,
        );

        ui.separator();
        ui.label("Text size");
        let mut scale = self.chosen.scale;
        let slider = ui.add(
            egui::Slider::new(&mut scale, MIN_FONT_SCALE..=MAX_FONT_SCALE)
                .step_by(0.05)
                .fixed_decimals(2)
                .suffix("x"),
        );
        if slider.changed() {
            self.chosen.scale = fonts::clamp_scale(scale);
            apply_now = true;
        }
        // Grava quando o usuário **solta** o controle: arrastar não pode gerar uma escrita por pixel.
        if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
            changed = true;
        }
        ui.weak(format!(
            "Between {MIN_FONT_SCALE:.2}x and {MAX_FONT_SCALE:.2}x. The window's minimum size grows with the font."
        ));

        ui.separator();
        if ui.button("Restore defaults").clicked() {
            self.chosen = Chosen::default();
            apply_now = true;
            changed = true;
        }

        if let Some(warning) = &self.warning {
            ui.colored_label(egui::Color32::YELLOW, warning);
        }

        // Gravar é o "commit": persiste **e** aplica. Sem isso, trocar a fonte mandava só
        // `SaveGuiPrefs` (porque `changed` também era verdadeiro) e a janela só mudava quando o
        // tamanho era mexido — era o bug relatado.
        //
        // A segunda seção entra no mesmo commit: a seção `gui` é gravada inteira.
        let (selection_request, selections_changed) = self.selections_ui(ui, servers);
        changed |= selections_changed;

        // O pedido da seção (Test, `⟳`) vence o commit de aparência: os dois não ocorrem no mesmo
        // quadro, mas se ocorrerem a ação explícita do botão é a que o usuário pediu.
        if selection_request.is_some() {
            return selection_request;
        }

        decision(self.chosen.clone(), self.prefs(), changed, apply_now)
    }

    /// A seção **Database selection queries** (§4.6): lista + formulário de um por vez.
    ///
    /// Devolve o pedido que a tela precisa do núcleo (`Test`, `⟳`) e se a **lista mudou** — quem
    /// chamou é que grava, via `decision`.
    fn selections_ui(
        &mut self,
        ui: &mut egui::Ui,
        servers: &[ServerConfigEntry],
    ) -> (Option<Request>, bool) {
        ui.add_space(8.0);
        ui.separator();
        ui.heading("Database selection queries");
        ui.label(
            "Saved queries that mark databases in bulk. The rows are matched against the servers \
             registered in `settings db-servers`: a host that is not registered is an error.",
        );
        ui.add_space(4.0);

        let mut request = None;
        let mut list_changed = false;

        // Lista, como na aba Servers: um botão por item.
        for index in 0..self.selections.len() {
            let query = self.selections[index].clone();
            ui.horizontal(|ui| {
                ui.label(format!("{}. {}", index + 1, query.name));
                ui.weak(format!("{} / {}", query.server, query.database));

                if ui.button("Edit").clicked() {
                    self.selection_form = Some(SelectionForm::edit(&query));
                    self.confirming_remove = None;
                    self.test_result = None;
                    // Editar também carrega as bases do servidor da query.
                    if let Some(discover) = self.ensure_discovered(&query.server) {
                        request = Some(discover);
                    }
                }

                // Remover em **dois toques**: o primeiro arma, o segundo remove.
                let armed = self
                    .confirming_remove
                    .as_deref()
                    .is_some_and(|armed| armed.eq_ignore_ascii_case(&query.name));
                let label = if armed { "Confirm remove" } else { "Remove" };
                if ui.button(label).clicked() {
                    list_changed |= self.tap_remove(&query.name);
                    ui.ctx().request_repaint();
                }
            });
        }

        if self.selections.is_empty() {
            ui.weak("No query yet. Add one to mark databases from a saved query.");
        }

        if self.selection_form.is_none() && ui.button("＋ Add query").clicked() {
            self.selection_form = Some(SelectionForm::new(servers));
            self.confirming_remove = None;
            self.test_result = None;
            // O formulário novo já aponta para o primeiro servidor: as bases dele começam a
            // carregar sem o dono precisar clicar em nada.
            if let Some(first) = servers.first()
                && let Some(discover) = self.ensure_discovered(&first.name)
            {
                request = Some(discover);
            }
        }

        if let Some(message) = &self.selection_message {
            ui.weak(message);
        }

        if let Some(mut form) = self.selection_form.clone() {
            ui.add_space(6.0);
            let mut save = false;
            let mut cancel = false;

            egui::Grid::new("selection_query_form")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.name);
                    ui.end_row();

                    ui.label("Server");
                    let current = if form.server.is_empty() {
                        "— pick a server —".to_string()
                    } else {
                        form.server.clone()
                    };
                    let mut picked: Option<String> = None;
                    egui::ComboBox::from_id_salt("selection_query_server")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for server in servers {
                                if ui
                                    .selectable_value(
                                        &mut form.server,
                                        server.name.clone(),
                                        &server.name,
                                    )
                                    .changed()
                                {
                                    picked = Some(server.name.clone());
                                }
                            }
                        });
                    // Escolher o servidor **já carrega** as bases: o `⟳` deixa de ser obrigatório.
                    if let Some(name) = picked
                        && let Some(discover) = self.ensure_discovered(&name)
                    {
                        request = Some(discover);
                    }
                    ui.end_row();

                    ui.label("Database");
                    ui.horizontal(|ui| {
                        // O campo é **só** um dropdown, no mesmo estilo dos combos de fonte: mostra
                        // o nome escolhido (ou `-` quando nada foi escolhido) e abre a lista com
                        // busca. Digitar à mão saiu do formulário; o nome livre continua possível
                        // pela entrada `Use "…"` dentro do popup.
                        //
                        // Escolher só mexe no formulário; quem grava é o `Save`.
                        let databases = self.ready_databases(&form.server).unwrap_or_default();
                        let _ = database_combo(
                            ui,
                            "selection_query_database",
                            &mut form.database,
                            &databases,
                            &mut form.db_filter,
                            &mut form.db_open,
                            !form.server.is_empty(),
                        );

                        let can_discover = !form.server.is_empty();
                        let loading = self.is_loading(&form.server);
                        if ui
                            .add_enabled(can_discover && !loading, egui::Button::new("⟳"))
                            .on_hover_text("List the databases of this server")
                            .clicked()
                        {
                            self.discovered
                                .insert(form.server.clone(), Discovery::Loading);
                            request = Some(Request::DiscoverDatabases {
                                server: Some(form.server.clone()),
                            });
                        }

                        // Enquanto carrega, a tela diz que está carregando: sem isso o botão
                        // desabilitado parece quebrado.
                        if loading {
                            ui.spinner();
                            ui.weak("Loading databases…");
                        }
                    });
                    ui.end_row();

                    // Desfecho da descoberta logo **abaixo do campo**, e não no fim do formulário:
                    // era o que fazia o `⟳` parecer não ter feito nada.
                    ui.label("");
                    ui.vertical(|ui| match self.discovered.get(&form.server) {
                        Some(Discovery::Failed(error)) => {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("Could not list the databases: {error}"),
                            );
                        }
                        Some(Discovery::Ready(databases)) if !databases.is_empty() => {
                            ui.weak(format!(
                                "{} database(s) on '{}'. Use ▾ to pick one.",
                                databases.len(),
                                form.server
                            ));
                        }
                        _ => {}
                    });
                    ui.end_row();

                    ui.label("SQL");
                    ui.vertical(|ui| {
                        // O realce é o mesmo do editor da aba Query: um `layouter` com os tokens
                        // de `sql_editor`, e a altura crescendo com o texto.
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
                                .id_salt("selection_query_sql")
                                .desired_rows(rows)
                                .desired_width(f32::INFINITY)
                                .layouter(&mut layouter),
                        );
                    });
                    ui.end_row();
                });

            if let Some(error) = &form.error {
                ui.colored_label(egui::Color32::YELLOW, error);
            }

            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Test").clicked() {
                    match form.to_query() {
                        Ok(query) => request = Some(Request::TestSelection(query)),
                        Err(error) => {
                            form.error = Some(error);
                            request = None;
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });

            if let Some(result) = &self.test_result {
                ui.weak(result);
            }

            if save {
                // `commit_form` fecha o formulário quando a query entra e o reabre com o erro
                // quando não entra.
                self.selection_form = Some(form.clone());
                list_changed |= self.commit_form(&form);
            } else if cancel {
                self.selection_form = None;
                self.test_result = None;
                self.selection_message = None;
            } else {
                self.selection_form = Some(form);
            }
        }

        (request, list_changed)
    }

    /// Um combo de família com a opção "embutida" no topo.
    ///
    /// Os parâmetros de apresentação vêm em [`Combo`] para a assinatura não crescer a cada opção nova
    /// (a de monoespaçada e a proporcional só diferem nisso).
    fn family_combo(
        &mut self,
        ui: &mut egui::Ui,
        combo: Combo<'_>,
        families: &[String],
        apply_now: &mut bool,
    ) -> bool {
        let Combo {
            label,
            id,
            embedded,
            which,
        } = combo;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(label);
            let current = match which {
                Family::Interface => self.chosen.interface.clone(),
                Family::Editor => self.chosen.editor.clone(),
            };
            let selected = current.clone().unwrap_or_else(|| embedded.to_string());

            egui::ComboBox::from_id_salt(id)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    // "Embutida" limpa a escolha.
                    if ui.selectable_label(current.is_none(), embedded).clicked() {
                        match which {
                            Family::Interface => self.chosen.interface = None,
                            Family::Editor => self.chosen.editor = None,
                        }
                        *apply_now = true;
                        changed = true;
                    }

                    // A Fira Code aparece mesmo quando já está instalada: é a opção garantida.
                    if !families.iter().any(|name| name == "Fira Code")
                        && ui
                            .selectable_label(current.as_deref() == Some("Fira Code"), "Fira Code")
                            .clicked()
                    {
                        match which {
                            Family::Interface => {
                                self.chosen.interface = Some("Fira Code".to_string())
                            }
                            Family::Editor => self.chosen.editor = Some("Fira Code".to_string()),
                        }
                        *apply_now = true;
                        changed = true;
                    }

                    for family in families {
                        if ui
                            .selectable_label(current.as_deref() == Some(family.as_str()), family)
                            .clicked()
                        {
                            match which {
                                Family::Interface => self.chosen.interface = Some(family.clone()),
                                Family::Editor => self.chosen.editor = Some(family.clone()),
                            }
                            *apply_now = true;
                            changed = true;
                        }
                    }
                });
        });

        changed
    }
}

/// O campo `Database` como **dropdown**: um botão no estilo dos combos de fonte (`valor ▼`) que
/// abre a lista das bases com **campo de busca**.
///
/// Nada escolhido mostra `-`. Escolher de uma lista longa é o uso real: o servidor do dono tem
/// dezenas de bases e o nome exato ninguém lembra. A busca filtra sem diferenciar maiúsculas e,
/// quando o texto digitado não casa com nenhuma base, oferece o próprio texto (`Use "…"`) — é a
/// porta para a base que a descoberta ainda não viu, sem devolver um campo de texto ao formulário.
///
/// Devolve `true` quando o valor mudou.
fn database_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    options: &[String],
    filter: &mut String,
    open: &mut bool,
    enabled: bool,
) -> bool {
    let mut changed = false;

    // Mesmo formato dos combos de fonte: um botão com o valor à esquerda e o triângulo à direita.
    // Nada escolhido mostra `-`, que é o estado inicial de uma query nova.
    let label = if value.trim().is_empty() {
        "-".to_string()
    } else {
        value.clone()
    };
    let button = ui
        .add_enabled(
            enabled,
            egui::Button::new(format!("{label}   ▼")).min_size(egui::vec2(200.0, 0.0)),
        )
        .on_hover_text("Pick the database where the query runs")
        .on_disabled_hover_text("Pick a server first");
    if button.clicked() {
        *open = !*open;
        // Abrir limpa a busca anterior: o dono quer ver a lista, não a última procura.
        filter.clear();
    }

    if *open {
        // `IgnoreClicks` + fechamento decidido aqui: com `CloseOnClickOutside` o popup fecharia no
        // mesmo quadro em que abre (o clique do botão fica fora do popup).
        let inner = egui::Popup::from_response(&button)
            .open(true)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .width(260.0)
            .show(|ui| {
                ui.add(
                    egui::TextEdit::singleline(filter)
                        .id_salt(id)
                        .hint_text("Search…")
                        .desired_width(f32::INFINITY),
                );

                let needle = filter.trim().to_lowercase();

                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        // Texto livre: sem isso, a base fora da lista (ou com a descoberta incompleta)
                        // não teria como ser escolhida pelo dropdown.
                        let typed = filter.trim();
                        if !typed.is_empty()
                            && !options.iter().any(|option| option == typed)
                            && ui.button(format!("Use '{typed}'")).clicked()
                        {
                            *value = typed.to_string();
                            changed = true;
                            *open = false;
                        }

                        let mut any = false;
                        for option in options {
                            if !needle.is_empty() && !option.to_lowercase().contains(&needle) {
                                continue;
                            }
                            any = true;
                            let selected = option.as_str() == value.as_str();
                            if ui.selectable_label(selected, option.as_str()).clicked() {
                                *value = option.to_string();
                                changed = true;
                                *open = false;
                            }
                        }

                        if !any && typed.is_empty() {
                            ui.weak(if options.is_empty() {
                                "No database listed for this server yet — press ⟳ or type a name."
                            } else {
                                "No database matches the search."
                            });
                        }
                    });
            });

        // Fecha com `Esc` ou com um clique fora do popup — o clique que **abriu** não conta.
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let clicked_outside = !button.clicked()
            && ui.input(|input| input.pointer.any_pressed())
            && ui
                .input(|input| input.pointer.interact_pos())
                .is_some_and(|position| {
                    inner
                        .as_ref()
                        .is_some_and(|inner| !inner.response.rect.contains(position))
                        && !button.rect.contains(position)
                });

        if escape || clicked_outside {
            *open = false;
        }
    }

    changed
}

/// Qual pedido a aba emite com o que o usuário fez neste quadro.
///
/// A regra é pequena mas já errou uma vez: quando a fonte muda, `changed` **e** `apply_now` são
/// verdadeiros, e emitir os dois com `if` separados fazia o segundo sobrescrever o primeiro — a fonte
/// só era aplicada no quadro em que *outro* controle mexesse. Aqui o commit tem precedência.
///
/// `prefs` é a seção `gui` inteira (aparência + queries salvas), não só a fonte: gravar mexe no
/// arquivo todo.
/// CONTRACT: IF-222
fn decision(chosen: Chosen, prefs: GuiPrefs, changed: bool, apply_now: bool) -> Option<Request> {
    if changed {
        // Commit: persistir **e** aplicar (o app aplica antes de gravar).
        Some(Request::SaveGuiPrefs(prefs))
    } else if apply_now {
        // Prévia ao vivo (arrastar o slider): aplica sem gravar a cada movimento.
        Some(Request::ApplyGuiPrefs(chosen))
    } else {
        None
    }
}

/// Qual família o combo está editando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Interface,
    Editor,
}

/// Apresentação de um combo de família.
#[derive(Debug, Clone, Copy)]
struct Combo<'a> {
    /// Rótulo à esquerda.
    label: &'a str,
    /// Id estável do combo no egui.
    id: &'a str,
    /// Rótulo da opção "embutida" (também é o que aparece quando nada foi escolhido).
    embedded: &'a str,
    /// Qual família este combo edita.
    which: Family,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trocar a fonte emite **um** pedido, e é o de commit (grava + aplica).
    ///
    /// Era o bug relatado: `changed` e `apply_now` verdadeiros juntos, dois `if` separados e o
    /// segundo vencia — a fonte só aparecia quando o usuário mexia no tamanho depois.
    /// CONTRACT: IF-222
    #[test]
    fn choosing_a_font_emits_the_commit_request() {
        let chosen = Chosen {
            interface: Some("Arial".to_string()),
            editor: None,
            scale: 1.0,
        };

        // Escolher fonte: mudou (para gravar) e pede aplicação imediata.
        let mut prefs = GuiPrefs::default();
        chosen.apply_to(&mut prefs);
        match decision(chosen.clone(), prefs.clone(), true, true) {
            Some(Request::SaveGuiPrefs(saved)) => {
                assert_eq!(saved.interface_font.as_deref(), Some("Arial"));
            }
            other => panic!("esperava o pedido de commit, veio {other:?}"),
        }

        // Arrastar o slider: só prévia, sem gravar a cada pixel.
        match decision(chosen.clone(), prefs.clone(), false, true) {
            Some(Request::ApplyGuiPrefs(applied)) => assert_eq!(applied, chosen),
            other => panic!("esperava prévia, veio {other:?}"),
        }

        // Nada mudou: nenhum pedido.
        assert_eq!(decision(chosen, prefs, false, false), None);
    }

    /// Trocar a fonte **preserva** as queries de seleção gravadas.
    ///
    /// A seção `gui` é gravada inteira: montar só a aparência apagaria as queries de quem só mexeu
    /// no tamanho do texto.
    /// CONTRACT: IF-229
    #[test]
    fn saving_the_appearance_keeps_the_selection_queries() {
        let saved = SelectionQuery {
            name: "Produção".to_string(),
            server: "configurador".to_string(),
            database: "configurador".to_string(),
            sql: "select host, port, database from tenants".to_string(),
        };
        let prefs = GuiPrefs {
            selections: vec![saved.clone()],
            ..GuiPrefs::default()
        };
        let mut state = State::with_prefs(Some(&prefs));

        // Restaurar padrões mexe **só** na aparência.
        state.chosen = Chosen::default();

        let merged = state.prefs();
        assert_eq!(merged.selections, vec![saved], "as queries continuam lá");
        assert_eq!(
            merged.font_scale,
            mustel_core::config::model::DEFAULT_FONT_SCALE
        );
    }

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: "127.0.0.1".to_string(),
            username: "postgres".to_string(),
            ..ServerConfigEntry::default()
        }
    }

    /// O formulário novo já aponta para o primeiro servidor do cadastro, e o servidor escolhido
    /// entra na query gravada.
    /// CONTRACT: IF-230
    #[test]
    fn a_new_form_starts_on_the_first_registered_server() {
        let registry = vec![server("configurador"), server("Local")];
        let mut form = SelectionForm::new(&registry);
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
        let mut form = SelectionForm::new(&[]);
        form.name = "Produção".to_string();
        form.database = "configurador".to_string();
        form.sql = "select 1".to_string();

        assert!(form.to_query().unwrap_err().contains("server"));
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
            databases: Ok(vec!["dev".to_string(), "test2".to_string()]),
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
            databases: Ok(vec!["dev".to_string()]),
            has_password: true,
        });

        assert!(state.prefs().selections.is_empty(), "nada foi gravado");
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

    /// O combo mostra `-` sem escolha e o nome depois de escolher (formato dos combos de fonte).
    /// CONTRACT: IF-232
    #[test]
    fn the_database_combo_shows_a_dash_until_something_is_picked() {
        let ctx = egui::Context::default();
        let options = vec!["configurador".to_string(), "dev".to_string()];

        let painted = |value: &str, open: bool| {
            let mut value = value.to_string();
            let mut filter = String::new();
            let mut open = open;
            let mut texts: Vec<String> = Vec::new();
            for _ in 0..2 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let _ = database_combo(
                            ui,
                            "selection_query_database",
                            &mut value,
                            &options,
                            &mut filter,
                            &mut open,
                            true,
                        );
                    });
                });
                texts.clear();
                collect_text(&output.shapes, &mut texts);
                output.textures_delta.clear();
            }
            texts
        };

        // Sem escolha: o botão traz `-`, e nao um campo vazio esperando digitação.
        let texts = painted("", false);
        assert!(
            texts.iter().any(|text| text.contains('-')),
            "o botão mostra `-`: {texts:?}"
        );
        assert!(
            !texts
                .iter()
                .any(|text| text.contains("where the query runs")),
            "não há mais campo de digitação manual: {texts:?}"
        );

        // Com escolha: o nome aparece no próprio botão.
        let texts = painted("dev", false);
        assert!(
            texts.iter().any(|text| text.contains("dev")),
            "o botão mostra o nome escolhido: {texts:?}"
        );
    }

    /// O dropdown de base **renderiza** a busca e as opções quando aberto.
    ///
    /// O teste desenha o picker com o popup já aberto na memória do `egui` — o caminho que, na
    /// janela, parecia não fazer nada — e confere que a busca e as bases foram pintadas.
    /// CONTRACT: IF-232
    #[test]
    fn the_database_picker_shows_a_search_box_and_the_options() {
        let ctx = egui::Context::default();
        let popup_id = egui::Id::unique(("selection_query_database", "popup"));
        let mut value = String::new();
        let mut filter = String::new();
        let mut open = true;
        let options = vec![
            "configurador".to_string(),
            "dev".to_string(),
            "test2".to_string(),
        ];

        let mut texts: Vec<String> = Vec::new();
        let mut saw_open = false;
        // Dois quadros: no primeiro o `Area` do popup roda em *sizing pass* e ainda não pinta.
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = database_combo(
                        ui,
                        "selection_query_database",
                        &mut value,
                        &options,
                        &mut filter,
                        &mut open,
                        true,
                    );
                    egui::Popup::open_id(ui.ctx(), popup_id);
                    saw_open = egui::Popup::is_id_open(ui.ctx(), popup_id);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(saw_open, "a memória registra o popup aberto");
        assert!(
            texts.iter().any(|text| text == "Search…"),
            "o popup tem campo de busca: {texts:?}"
        );
        for option in &options {
            assert!(
                texts.iter().any(|text| text == option),
                "a opção {option} aparece: {texts:?}"
            );
        }
    }

    /// A busca do dropdown filtra sem diferenciar maiúsculas e oferece o texto livre.
    /// CONTRACT: IF-232
    #[test]
    fn the_database_picker_filters_and_offers_the_typed_name() {
        let ctx = egui::Context::default();
        let mut value = String::new();
        let mut filter = "CONF".to_string();
        let mut open = true;
        let options = vec!["configurador".to_string(), "dev".to_string()];

        let mut texts: Vec<String> = Vec::new();
        // Dois quadros: no primeiro o `Area` do popup roda em *sizing pass* e ainda não pinta.
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    database_combo(
                        ui,
                        "selection_query_database",
                        &mut value,
                        &options,
                        &mut filter,
                        &mut open,
                        true,
                    );
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(
            texts.iter().any(|text| text == "configurador"),
            "a base que casa com a busca aparece: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "dev"),
            "a que não casa fica fora: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text.contains("Use 'CONF'")),
            "o texto digitado pode ser usado como nome: {texts:?}"
        );
    }

    fn query(name: &str) -> SelectionQuery {
        SelectionQuery {
            name: name.to_string(),
            server: "configurador".to_string(),
            database: "configurador".to_string(),
            sql: "select host, port, database from tenants".to_string(),
        }
    }

    fn form_for(query: &SelectionQuery) -> SelectionForm {
        SelectionForm::edit(query)
    }

    /// Nome vazio é recusado, e a lista não muda.
    /// CONTRACT: IF-230
    #[test]
    fn an_empty_name_is_refused() {
        let mut state = State::with_prefs(None);
        state.selection_form = Some(SelectionForm::default());

        assert!(!state.commit_form(&SelectionForm::default()));
        assert!(state.selections.is_empty(), "nada entrou");
        assert!(
            state
                .selection_form
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
        state.selection_form = Some(form_for(&query("Produção")));
        assert!(state.commit_form(&form_for(&query("Produção"))));

        // Mesma query, nome com maiúsculas e espaços diferentes: ainda é o mesmo nome.
        let mut other = query(" produção ");
        other.sql = "select 1".to_string();
        state.selection_form = Some(form_for(&other));
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
        state.selection_form = Some(form_for(&original));
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
        state.selection_form = Some(form_for(&saved));
        assert!(state.commit_form(&form_for(&saved)));

        assert!(!state.tap_remove("Produção"), "o primeiro toque só arma");
        assert_eq!(state.selections.len(), 1, "a query continua lá");
        assert_eq!(state.confirming_remove.as_deref(), Some("Produção"));

        assert!(state.tap_remove("Produção"), "o segundo remove");
        assert!(state.selections.is_empty());
        assert_eq!(state.confirming_remove, None, "o alarme desarma");

        // Tocar em **outra** query não aproveita a confirmação da primeira.
        state.selection_form = Some(form_for(&query("A")));
        assert!(state.commit_form(&form_for(&query("A"))));
        state.selection_form = Some(form_for(&query("B")));
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
