//! Tela de servidores cadastrados (T42): CRUD no `mustel.jsonc`, senha no cofre e teste de conexão.
//!
//! O formulário edita uma **cópia** do servidor; quem grava é o núcleo (`db_servers::{add, update}`),
//! então as validações e as mensagens são as mesmas do CLI. Editar um servidor existente usa
//! `update` (novo no T42), que preserva a senha do cofre — o caminho do CLI (`rm` + `add`) apagaria.

use mustel_core::config::ServerConfigEntry;

use crate::request::Request;

/// O que o formulário está fazendo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Form {
    /// Servidor sendo editado (`None` = cadastro novo).
    original: Option<String>,
    /// Campos em edição.
    draft: ServerConfigEntry,
    /// Bancos como texto (a lista vira `Vec` ao gravar).
    databases: String,
    /// Padrões de exclusão como texto.
    exclude_patterns: String,
    /// Senha a guardar junto (opcional).
    password: String,
    /// De qual servidor os valores foram copiados (só informativo, para o combo mostrar a origem).
    template: Option<String>,
}

impl Form {
    /// Formulário vazio para um cadastro novo.
    fn new() -> Self {
        Self {
            original: None,
            draft: ServerConfigEntry::default(),
            databases: String::new(),
            exclude_patterns: "template*,postgres".to_string(),
            password: String::new(),
            template: None,
        }
    }

    /// Formulário preenchido a partir de um servidor existente.
    fn edit(server: &ServerConfigEntry) -> Self {
        Self {
            original: Some(server.name.clone()),
            draft: server.clone(),
            databases: server.databases.join(", "),
            exclude_patterns: server.exclude_patterns.join(","),
            password: String::new(),
            template: None,
        }
    }

    /// Copia os valores de `server` para o formulário, como ponto de partida de um cadastro novo.
    ///
    /// Ficam de fora, de propósito:
    ///
    /// - o **nome**, que é a identidade do servidor novo — copiá-lo só criaria um conflito;
    /// - a **senha**, que mora no cofre com a chave do nome; copiar daria ao servidor novo a senha do
    ///   antigo sem ninguém perceber. Quem quiser a mesma senha digita no campo (ou usa o CLI).
    ///
    /// Todo o resto é copiado: host, porta, usuário, bancos, descoberta automática, padrões de
    /// exclusão, SSL, timeout e paralelismo. Depois é só editar o que quiser.
    fn copy_from(&mut self, server: &ServerConfigEntry) {
        self.draft.host = server.host.clone();
        self.draft.port = server.port;
        self.draft.username = server.username.clone();
        self.draft.databases = server.databases.clone();
        self.draft.fetch_all_databases = server.fetch_all_databases;
        self.draft.exclude_patterns = server.exclude_patterns.clone();
        self.draft.ssl_mode = server.ssl_mode.clone();
        self.draft.timeout = server.timeout;
        self.draft.max_parallelism = server.max_parallelism;

        self.databases = server.databases.join(", ");
        self.exclude_patterns = server.exclude_patterns.join(",");
        self.template = Some(server.name.clone());
    }

    /// O servidor como o núcleo espera, já com as listas convertidas.
    fn to_entry(&self) -> ServerConfigEntry {
        ServerConfigEntry {
            databases: split(&self.databases),
            exclude_patterns: split(&self.exclude_patterns),
            ..self.draft.clone()
        }
    }
}

/// O rótulo de um segmento: `Servers (2)`, e `•` quando aquele lado tem rascunho aberto.
///
/// A contagem vai entre parênteses porque é **detalhe**, não parte do nome — a mesma leitura do
/// `All servers (208)` no combo de servidor da aba Query.
fn segment_label(name: &str, count: usize, draft: bool) -> String {
    let mut label = format!("{name} ({count})");
    if draft {
        label.push_str(" •");
    }
    label
}

/// Qual dos dois cadastros a aba **Connections** está mostrando.
///
/// O seletor é global (pode ser aplicado a qualquer servidor), então ele não mora dentro de um
/// servidor: mora ao lado, na segunda sub-aba. As duas listas trocam juntas — sidebar e área
/// principal —, e o contexto **sempre** volta em `Servers` ao entrar na aba (o rascunho de um
/// formulário, esse, sobrevive à troca de abas).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Context {
    /// O cadastro de servidores.
    #[default]
    Servers,
    /// O cadastro de seletores (queries que marcam bases em massa).
    QuerySelector,
}

/// Quanto tempo o check/X do `Test` fica no lugar do botão antes de voltar a `Test`.
const TEST_FEEDBACK: std::time::Duration = std::time::Duration::from_secs(2);

/// Largura mínima do botão `Test`, a mesma nos três estados (ocioso, verificando, resultado) —
/// assim a linha `Edit · Test · Remove` não dança a cada mudança de estado.
///
/// É **largura mínima** (`Button::min_size`), não tamanho fixo: a altura continua sendo a natural
/// do botão. Forçar a altura com `add_sized` deixava o `Test` mais alto que `Edit`/`Remove`.
fn test_button_width(ui: &egui::Ui) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap("Test".to_owned(), font, ui.visuals().text_color());
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// Desenha um botão **com spinner**: é o `Test` enquanto a conexão é verificada.
///
/// O spinner vai dentro de uma moldura do tamanho do botão ocioso, para a linha não mudar de
/// largura entre “Test”, “verificando” e o resultado.
fn draw_spinner_button(ui: &mut egui::Ui, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let visuals = ui.style().visuals.widgets.inactive;
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let inner = egui::Rect::from_center_size(
        rect.center(),
        egui::Vec2::splat((rect.height() * 0.55).min(rect.width() * 0.8)),
    );
    ui.put(inner, egui::Spinner::new().size(inner.height()));
}

/// Estado da tela de servidores.
#[derive(Debug, Default)]
pub struct State {
    /// Servidores do cadastro, como o núcleo devolveu.
    servers: Vec<ServerConfigEntry>,
    /// Formulário aberto, quando houver.
    form: Option<Form>,
    /// Servidor marcado para remoção (confirmação em dois toques).
    confirming_remove: Option<String>,
    /// Último recado da tela.
    message: Option<String>,
    /// Erro do último pedido.
    error: Option<String>,
    /// Servidores com o `Test` de conexão **em andamento** (o botão vira spinner).
    testing: Vec<String>,
    /// Último resultado do `Test` por servidor: (servidor, deu certo?, quando chegou).
    ///
    /// O botão mostra check ou X por alguns segundos e volta a `Test` — é o retorno visual do
    /// teste, no lugar do texto com as bases encontradas.
    tested: Vec<(String, bool, std::time::Instant)>,
    /// Senha a guardar depois que o servidor for gravado (o `Save` manda os dois pedidos).
    pending_password: Option<String>,
    /// Qual cadastro a aba está mostrando.
    context: Context,
    /// O cadastro de seletores (a segunda sub-aba).
    selectors: crate::screens::selectors::State,
}

impl State {
    /// Estado com a lista já carregada.
    pub fn new(servers: Vec<ServerConfigEntry>) -> Self {
        Self {
            servers,
            ..Self::default()
        }
    }

    /// Os servidores do cadastro (a tela de consulta também escolhe entre eles).
    pub fn servers(&self) -> &[ServerConfigEntry] {
        &self.servers
    }

    /// O cadastro de seletores (quem lê os seletores inteiros).
    pub fn selectors(&self) -> &crate::screens::selectors::State {
        &self.selectors
    }

    /// Estado com as queries de seleção que estavam gravadas.
    pub fn with_selectors(mut self, prefs: Option<&mustel_core::config::model::GuiPrefs>) -> Self {
        self.selectors = crate::screens::selectors::State::with_prefs(prefs);
        self
    }

    /// Entrar na aba volta para o cadastro de servidores (o rascunho do formulário fica).
    pub fn reset_context(&mut self) {
        self.context = Context::Servers;
    }

    /// Aplica o que voltou de um job.
    pub fn apply(&mut self, result: &crate::request::JobResult) {
        use crate::request::JobResult;

        // O cadastro de seletores tem os próprios jobs (descoberta do `Database` e `Test`).
        self.selectors.apply(result);

        match result {
            // Resultados de outras telas não são desta:
            // - painel da IA (`Ddl`, `SqlGenerated`, perfis e teste);
            // - esquema/carga da aba Query (`Schema`, `Databases`, seleção e `QueryDone`);
            // - ensaio da query de seleção (`SelectionTested`), que é da aba Settings.
            JobResult::Ddl { .. }
            | JobResult::SqlGenerated { .. }
            | JobResult::AiProfilesSaved
            | JobResult::AiTested { .. }
            | JobResult::Schema { .. }
            | JobResult::Databases { .. }
            | JobResult::SelectionTested { .. }
            | JobResult::Selection { .. }
            | JobResult::SelectionFailed { .. }
            | JobResult::QueryDone { .. }
            | JobResult::QueryWithoutDatabases => {}
            JobResult::Servers(servers) => {
                self.servers = servers.clone();
                self.error = None;
            }
            JobResult::Saved(name) => {
                self.form = None;
                self.message = Some(format!("Server '{name}' saved."));
                self.error = None;
            }
            JobResult::Removed(name) => {
                self.message = Some(format!("Server '{name}' removed."));
                self.error = None;
                self.confirming_remove = None;
            }
            JobResult::PasswordSaved(name) => {
                self.form = None;
                self.message = Some(format!("Password saved for '{name}'."));
                self.error = None;
            }
            JobResult::Tested { server, outcome } => {
                self.testing.retain(|name| name != server);
                self.tested.retain(|(name, _, _)| name != server);
                self.tested
                    .push((server.clone(), outcome.is_ok(), std::time::Instant::now()));
                // O resultado é do **cartão** (check ou X); o motivo da falha vai para o toast, e
                // a área principal não repete o texto vermelho. Nada de listar as bases aqui
                // (era o texto antigo).
                self.error = None;
            }
            JobResult::Failed(error) => {
                self.error = Some(error.clone());
            }
        }
    }

    /// A senha que ficou esperando o servidor ser gravado, se houver.
    pub fn take_pending_password(&mut self) -> Option<String> {
        self.pending_password.take()
    }

    /// A **sidebar**: a lista do cadastro (é ela que escolhe o servidor a editar).
    ///
    /// O formulário fica na área principal — a lista é o que se percorre, o formulário é o que se
    /// preenche.
    pub fn sidebar_ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
        let mut request = None;

        // As **duas sub-abas** deste cadastro, como um controle segmentado: o contêiner diz "aqui se
        // troca de visão" e as metades iguais dizem que as duas são pares. O contador vive dentro
        // do segmento (a linha solta `N server(s)` saiu) e um `•` marca o lado que tem rascunho
        // aberto — o rascunho sobrevive à troca, mas nada avisava que ele estava lá.
        let labels = [
            segment_label("Servers", self.servers.len(), self.form.is_some()),
            segment_label(
                "Query Selector",
                self.selectors.selections().len(),
                self.selectors.has_form(),
            ),
        ];
        let selected = if self.context == Context::Servers {
            0
        } else {
            1
        };
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(index) = crate::tabs::segmented(ui, &refs, selected) {
            self.context = if index == 0 {
                Context::Servers
            } else {
                Context::QuerySelector
            };
        }

        // A sub-aba do seletor tem lista própria (e a área principal também troca).
        if self.context == Context::QuerySelector {
            let servers = self.servers.clone();
            return self.selectors.sidebar_ui(ui, &servers);
        }

        ui.horizontal(|ui| {
            if ui.button("Reload").clicked() {
                request = Some(Request::ReloadServers);
            }
            if ui.button("Add server").clicked() {
                self.form = Some(Form::new());
                self.confirming_remove = None;
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::danger(ui), error);
        }

        ui.separator();
        let servers = self.servers.clone();
        // Resultados antigos saem de cena sozinhos: o botão volta a ser `Test`.
        self.tested
            .retain(|(_, _, at)| at.elapsed() < TEST_FEEDBACK);
        // A lista ocupa a altura restante da sidebar: é o conteúdo principal dela.
        egui::ScrollArea::vertical()
            .id_salt("servers")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for server in servers {
                    // O card ocupa **toda a largura** da sidebar. O `group` encolhe até o conteúdo
                    // por padrão, então a largura é dada ao `Frame` já descontada a margem dele —
                    // pedir `available_width()` de dentro do grupo realimentava e o painel crescia
                    // até o máximo (420).
                    let frame = egui::Frame::group(ui.style());
                    let margins = frame.total_margin().sum().x;
                    let width = (ui.available_width() - margins).max(0.0);
                    frame.show(ui, |ui| {
                        ui.set_width(width);
                        ui.label(&server.name);
                        ui.weak(format!("{}:{}", server.host, server.port));
                        ui.horizontal(|ui| {
                            let edit = ui.button("Edit");
                            if edit.clicked() {
                                self.form = Some(Form::edit(&server));
                                self.confirming_remove = None;
                            }

                            // `Test`: spinner enquanto verifica a conexão e, por alguns segundos,
                            // check (deu certo) ou X (falhou). O texto com as bases encontradas
                            // saiu de cena: o resultado é o próprio botão.
                            let width = test_button_width(ui);
                            // A altura vem do `Edit`: forçar a altura deixava o `Test` mais alto.
                            let height = edit.rect.height();
                            let running = self.testing.iter().any(|name| name == &server.name);
                            let feedback = self
                                .tested
                                .iter()
                                .find(|(name, _, _)| name == &server.name)
                                .map(|(_, ok, at)| (*ok, *at));

                            if running {
                                draw_spinner_button(ui, egui::vec2(width, height));
                            } else if let Some((ok, at)) = feedback
                                && at.elapsed() < TEST_FEEDBACK
                            {
                                let icon = if ok {
                                    crate::icons::Icon::Check
                                } else {
                                    crate::icons::Icon::Cross
                                };
                                let color = if ok {
                                    ui.visuals().text_color()
                                } else {
                                    crate::theme::danger(ui)
                                };
                                let glyph = egui::RichText::new(icon.glyph().to_string())
                                    .font(crate::icons::font_id(
                                        ui.text_style_height(&egui::TextStyle::Button),
                                    ))
                                    .color(color);
                                let hover = if ok { "Connected" } else { "Connection failed" };
                                let button =
                                    egui::Button::new(glyph).min_size(egui::vec2(width, 0.0));
                                if ui.add(button).on_hover_text(hover).clicked() {
                                    self.testing.push(server.name.clone());
                                    request = Some(Request::TestConnection(server.clone()));
                                }
                                ui.ctx().request_repaint_after(TEST_FEEDBACK - at.elapsed());
                            } else if ui
                                .add(egui::Button::new("Test").min_size(egui::vec2(width, 0.0)))
                                .clicked()
                            {
                                self.testing.push(server.name.clone());
                                request = Some(Request::TestConnection(server.clone()));
                            }

                            // Remover pede confirmação no mesmo botão: o cadastro apaga a senha do
                            // cofre junto, então um clique só seria fácil demais.
                            let removing = self.confirming_remove.as_deref() == Some(&server.name);
                            let label = if removing { "Confirm remove" } else { "Remove" };
                            if ui.button(label).clicked() {
                                if removing {
                                    self.confirming_remove = None;
                                    request = Some(Request::RemoveServer(server.name.clone()));
                                } else {
                                    self.confirming_remove = Some(server.name.clone());
                                }
                            }
                        });
                    });
                }
            });

        request
    }

    /// A **área principal**: o formulário de cadastro/edição e o recado da última operação.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
        let mut request = None;

        // Título da seção: depois de trocar de sub-aba, a orientação não pode depender só da
        // sidebar.
        ui.heading(match self.context {
            Context::Servers => "Servers",
            Context::QuerySelector => "Query Selector",
        });
        ui.separator();

        // O cadastro de seletores desenha na área principal inteira (é onde o editor SQL cabe).
        if self.context == Context::QuerySelector {
            let servers = self.servers.clone();
            return self.selectors.ui(ui, &servers);
        }

        if let Some(message) = &self.message {
            ui.label(message);
        }

        if self.form.is_some() {
            // O formulário sai do estado enquanto é desenhado: ele precisa de `&mut self` para
            // guardar a senha pendente, e o empréstimo não pode durar o desenho inteiro.
            let mut form = self.form.take().expect("acabou de ser verificado");
            let (new_request, keep_open) = self.form_ui(ui, &mut form);
            request = new_request.or(request);
            if keep_open {
                self.form = Some(form);
            }
        } else {
            ui.weak("Pick a server in the list, or add a new one.");
        }

        request
    }

    /// O formulário de cadastro/edição. Devolve o pedido e se o formulário deve continuar aberto.
    fn form_ui(&mut self, ui: &mut egui::Ui, form: &mut Form) -> (Option<Request>, bool) {
        let mut request = None;
        let mut keep_open = true;
        let editing = form.original.is_some();

        ui.group(|ui| {
            ui.label(if editing { "Edit server" } else { "New server" });

            // Modelo: atalho para cadastrar a partir de um servidor que já existe. Só no cadastro
            // novo (editar um servidor existente é sobre aquele servidor) e só quando há de onde
            // copiar. Clicar numa linha copia **na hora**, inclusive a mesma já escolhida — o combo
            // é um botão de copiar, não um vínculo que se mantém.
            if !editing && !self.servers.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Template");
                    let selected = form.template.clone().unwrap_or_else(|| "—".to_string());
                    egui::ComboBox::from_id_salt("server_template")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for server in &self.servers {
                                if ui
                                    .selectable_label(
                                        form.template.as_deref() == Some(server.name.as_str()),
                                        &server.name,
                                    )
                                    .clicked()
                                {
                                    form.copy_from(server);
                                }
                            }
                        });
                    ui.weak("copia os valores do servidor escolhido (sem nome e sem senha)");
                });
            }

            egui::Grid::new("server_form")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.name));
                    ui.end_row();

                    ui.label("Host");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.host));
                    ui.end_row();

                    ui.label("Port");
                    let mut port = i64::from(form.draft.port);
                    if ui
                        .add(egui::DragValue::new(&mut port).range(1..=65535))
                        .changed()
                    {
                        form.draft.port = u16::try_from(port).unwrap_or(5432);
                    }
                    ui.end_row();

                    ui.label("Username");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.username));
                    ui.end_row();

                    ui.label("Databases");
                    ui.add(egui::TextEdit::singleline(&mut form.databases));
                    ui.end_row();

                    ui.label("Auto-discover");
                    ui.checkbox(&mut form.draft.fetch_all_databases, "fetch all");
                    ui.end_row();

                    ui.label("Exclude patterns");
                    ui.add(egui::TextEdit::singleline(&mut form.exclude_patterns));
                    ui.end_row();

                    ui.label("SSL mode");
                    egui::ComboBox::from_id_salt("ssl_mode")
                        .selected_text(&form.draft.ssl_mode)
                        .show_ui(ui, |ui| {
                            for mode in ["Disable", "Prefer", "Require"] {
                                ui.selectable_value(
                                    &mut form.draft.ssl_mode,
                                    mode.to_string(),
                                    mode,
                                );
                            }
                        });
                    ui.end_row();

                    ui.label("Password (keyring)");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.password)
                            .password(true)
                            .hint_text(if editing { "unchanged" } else { "optional" }),
                    );
                    ui.end_row();
                });

            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    request = Some(Request::SaveServer {
                        original: form.original.clone(),
                        server: form.to_entry(),
                    });
                    if !form.password.is_empty() {
                        // A senha vai junto: guardar depois de salvar exigiria saber que o save deu
                        // certo, e o `App` já encadeia os dois pedidos nessa ordem.
                        self.pending_password = Some(form.password.clone());
                    }
                }
                if ui.button("Cancel").clicked() {
                    keep_open = false;
                }
            });
        });

        (request, keep_open)
    }
}

/// Separa a lista por vírgula, como os campos de bancos e padrões do CLI.
fn split(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Copiar de um modelo traz **todos** os valores do formulário, menos nome e senha.
    ///
    /// O nome é a identidade do servidor novo e a senha mora no cofre com a chave do nome: copiar a
    /// senha daria ao servidor novo a senha do antigo sem ninguém perceber.
    /// CONTRACT: IF-215
    #[test]
    fn copying_a_template_brings_every_value_except_name_and_password() {
        let template = ServerConfigEntry {
            name: "origem".to_string(),
            host: "db.interno".to_string(),
            port: 5544,
            username: "leitor".to_string(),
            databases: vec!["a".to_string(), "b".to_string()],
            fetch_all_databases: true,
            exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
            ssl_mode: "Require".to_string(),
            timeout: 45,
            max_parallelism: 8,
        };

        let mut form = Form::new();
        form.draft.name = "novo".to_string();
        form.copy_from(&template);

        assert_eq!(form.draft.name, "novo", "o nome é do servidor novo");
        assert!(form.password.is_empty(), "senha não é copiada");

        let entry = form.to_entry();
        assert_eq!(entry.host, "db.interno");
        assert_eq!(entry.port, 5544);
        assert_eq!(entry.username, "leitor");
        assert_eq!(entry.databases, vec!["a".to_string(), "b".to_string()]);
        assert!(entry.fetch_all_databases);
        assert_eq!(
            entry.exclude_patterns,
            vec!["template*".to_string(), "postgres".to_string()]
        );
        assert_eq!(entry.ssl_mode, "Require");
        assert_eq!(entry.timeout, 45);
        assert_eq!(entry.max_parallelism, 8);

        assert_eq!(
            form.template.as_deref(),
            Some("origem"),
            "o formulário lembra de onde copiou"
        );
    }

    /// Depois de copiar, os campos continuam editáveis e valem para o save.
    ///
    /// CONTRACT: IF-215
    #[test]
    fn copied_values_can_be_changed_before_saving() {
        let template = ServerConfigEntry {
            name: "origem".to_string(),
            host: "antigo".to_string(),
            port: 5432,
            ..ServerConfigEntry::default()
        };

        let mut form = Form::new();
        form.copy_from(&template);
        form.draft.host = "novo-host".to_string();
        form.databases = "so-esta".to_string();

        let entry = form.to_entry();
        assert_eq!(entry.host, "novo-host", "a edição vence o modelo");
        assert_eq!(entry.databases, vec!["so-esta".to_string()]);
    }

    #[test]
    fn the_form_splits_the_lists_like_the_cli() {
        let mut form = Form::new();
        form.draft.name = "dev".to_string();
        form.draft.host = "localhost".to_string();
        form.databases = "a, b ,".to_string();
        form.exclude_patterns = "template*, postgres".to_string();

        let entry = form.to_entry();

        assert_eq!(entry.databases, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(
            entry.exclude_patterns,
            vec!["template*".to_string(), "postgres".to_string()]
        );
    }

    #[test]
    fn editing_keeps_the_original_name_and_prefills_the_lists() {
        let server = ServerConfigEntry {
            name: "dev".to_string(),
            host: "localhost".to_string(),
            databases: vec!["a".to_string(), "b".to_string()],
            exclude_patterns: vec!["x*".to_string()],
            ..ServerConfigEntry::default()
        };

        let form = Form::edit(&server);

        assert_eq!(form.original.as_deref(), Some("dev"));
        assert_eq!(form.databases, "a, b");
        assert_eq!(form.exclude_patterns, "x*");
        assert_eq!(form.to_entry().name, "dev");
    }

    /// O rótulo do segmento traz a contagem, e `•` só quando aquele lado tem rascunho.
    /// CONTRACT: IF-242
    #[test]
    fn the_segment_label_shows_the_count_and_the_draft_dot() {
        assert_eq!(segment_label("Servers", 2, false), "Servers (2)");
        assert_eq!(
            segment_label("Query Selector", 0, false),
            "Query Selector (0)"
        );
        assert_eq!(
            segment_label("Query Selector", 3, true),
            "Query Selector (3) •",
            "o ponto avisa que há rascunho do outro lado"
        );
    }

    /// Entrar na aba volta para `Servers`, e o rascunho do formulário **não** se perde.
    /// CONTRACT: IF-242
    #[test]
    fn entering_the_tab_resets_the_context_but_keeps_the_draft() {
        let prefs = mustel_core::config::model::GuiPrefs {
            selections: vec![mustel_core::config::model::SelectionQuery {
                name: "Produção".to_string(),
                server: "Local".to_string(),
                database: "configurador".to_string(),
                sql: "select 1".to_string(),
            }],
            ..mustel_core::config::model::GuiPrefs::default()
        };
        let mut state = State::new(vec![ServerConfigEntry {
            name: "Local".to_string(),
            ..ServerConfigEntry::default()
        }])
        .with_selectors(Some(&prefs));

        // Vai para a sub-aba do seletor e abre o formulário (o rascunho).
        state.context = Context::QuerySelector;
        assert_eq!(state.selectors().selections().len(), 1);
        state.selectors.open_new_for_test(&state.servers.clone());
        assert!(state.selectors().has_form(), "o rascunho está aberto");

        // Sair e voltar (o app chama `reset_context`): contexto volta, rascunho fica.
        state.reset_context();
        assert_eq!(state.context, Context::Servers, "sempre começa em Servers");
        assert!(
            state.selectors().has_form(),
            "o rascunho sobrevive à troca de aba"
        );
        assert_eq!(
            state.selectors().selections().len(),
            1,
            "cadastro carregado"
        );
    }

    #[test]
    fn the_list_arrives_from_the_core() {
        let state = State::new(vec![ServerConfigEntry {
            name: "dev".to_string(),
            ..ServerConfigEntry::default()
        }]);

        assert_eq!(state.servers().len(), 1);
    }

    /// Os rótulos `Edit`, `Test` e `Remove` ficam na **mesma linha**.
    ///
    /// A regressão que originou isto: forçar o tamanho com `add_sized`/`interact_size.y` deixava o
    /// `Test` com a moldura mais alta que os vizinhos (o `Button` usa o layout de átomos; a altura
    /// natural não é `interact_size`). Agora só a **largura** é mínima (`Button::min_size`), e a
    /// altura vem do layout normal.
    ///
    /// Limite deste teste: o `Response` reporta a mesma altura nos dois códigos — a diferença era
    /// na moldura pintada, que só se confere na janela. Aqui fica presa a linha do texto.
    /// CONTRACT: IF-262
    #[test]
    fn the_test_label_sits_on_the_same_line_as_edit_and_remove() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.button("Edit");
                    let width = test_button_width(ui);
                    let _ = ui.add(egui::Button::new("Test").min_size(egui::vec2(width, 0.0)));
                    let _ = ui.button("Remove");
                });
            });
        });
        output.textures_delta.clear();

        let mut texts = Vec::new();
        collect_text_positions(&output.shapes, &mut texts);
        let y_of = |label: &str| {
            texts
                .iter()
                .find(|(text, _)| text == label)
                .map(|(_, position)| position.y)
                .unwrap_or_else(|| panic!("o rótulo {label} não foi pintado: {texts:?}"))
        };

        let (edit, test, remove) = (y_of("Edit"), y_of("Test"), y_of("Remove"));
        assert!(
            (edit - test).abs() < 0.5,
            "o Test desalinhou do Edit (edit={edit}, test={test})"
        );
        assert!(
            (remove - test).abs() < 0.5,
            "o Test desalinhou do Remove (remove={remove}, test={test})"
        );
    }

    /// Junta o texto e a posição de todo `Shape::Text`, descendo nos grupos.
    fn collect_text_positions(
        shapes: &[egui::epaint::ClippedShape],
        out: &mut Vec<(String, egui::Pos2)>,
    ) {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text) => {
                    out.push((text.galley.text().to_string(), text.pos));
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

    /// O que a tela pintou (textos), num quadro de 900×700.
    fn painted(state: &mut State) -> Vec<String> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // O app desenha a sidebar num painel próprio; a tela sozinha só faz a área
                // principal.
                egui::Panel::left("test_sidebar")
                    .exact_size(320.0)
                    .show(ui, |ui| {
                        let _ = state.sidebar_ui(ui);
                    });
                let _ = state.ui(ui);
            });
        });
        output.textures_delta.clear();

        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        texts
    }

    /// Junta o texto de todo `Shape::Text`, descendo nos grupos.
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

    /// `Test` que passou: o botão vira **check** e a lista de bases não aparece em texto.
    /// CONTRACT: IF-262
    #[test]
    fn a_successful_test_shows_a_check_and_no_database_list() {
        let mut state = State::new(vec![ServerConfigEntry {
            name: "Local".to_string(),
            ..ServerConfigEntry::default()
        }]);
        state.testing.push("Local".to_string());
        state.apply(&crate::request::JobResult::Tested {
            server: "Local".to_string(),
            outcome: Ok(vec!["mustel_test".to_string(), "postgres".to_string()]),
        });

        let painted = painted(&mut state);
        let everything = painted.join("\n");
        let check = crate::icons::Icon::Check.glyph().to_string();
        assert!(
            painted.iter().any(|text| text == &check),
            "o botão mostra o check: {everything}"
        );
        assert!(
            !everything.contains("mustel_test"),
            "sem lista de bases: {everything}"
        );
        assert!(
            !everything.contains("database(s)"),
            "sem a contagem de bases: {everything}"
        );
        assert!(state.testing.is_empty(), "o spinner saiu");
    }

    /// `Test` que falhou: o botão vira **X** e o motivo **não** fica na área principal — vai para
    /// o toast (é o mesmo recado, sem o texto vermelho ocupando a tela).
    /// CONTRACT: IF-262
    #[test]
    fn a_failed_test_shows_a_cross_and_leaves_the_reason_to_the_toast() {
        let mut state = State::new(vec![ServerConfigEntry {
            name: "Local".to_string(),
            ..ServerConfigEntry::default()
        }]);
        state.testing.push("Local".to_string());
        state.apply(&crate::request::JobResult::Tested {
            server: "Local".to_string(),
            outcome: Err("password authentication failed".to_string()),
        });

        let painted = painted(&mut state);
        let everything = painted.join("\n");
        let cross = crate::icons::Icon::Cross.glyph().to_string();
        assert!(
            painted.iter().any(|text| text == &cross),
            "o botão mostra o X: {everything}"
        );
        assert!(
            !everything.contains("password authentication failed"),
            "o motivo saiu da área principal (vai no toast): {everything}"
        );
        assert!(state.testing.is_empty(), "o spinner saiu");
    }

    /// Enquanto verifica, o botão vira spinner: o rótulo `Test` sai de cena.
    /// CONTRACT: IF-262
    #[test]
    fn the_test_button_shows_a_spinner_while_running() {
        let mut state = State::new(vec![ServerConfigEntry {
            name: "Local".to_string(),
            ..ServerConfigEntry::default()
        }]);
        state.testing.push("Local".to_string());

        let painted = painted(&mut state);
        assert!(
            !painted.iter().any(|text| text == "Test"),
            "sem o rótulo durante o teste: {painted:?}"
        );
    }
}
