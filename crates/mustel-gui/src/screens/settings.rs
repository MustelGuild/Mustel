//! Tela **Settings**: preferências de aparência da janela (tema e fonte).
//!
//! A tela não grava nada: devolve [`Request::SaveGuiPrefs`] e quem persiste é o app, através de
//! `usecases::gui_prefs` — a mesma separação das outras telas (`Request` → `App` → núcleo).
//!
//! A lista de famílias do sistema é cara de montar (o `fontique` varre o sistema), então é carregada
//! **uma vez**, na primeira vez que a aba abre, e guardada na tela.

use mustel_core::config::model::{
    GuiPrefs, GuiTheme, MAX_FONT_SCALE, MAX_PAGE_SIZE, MIN_FONT_SCALE, MIN_PAGE_SIZE,
    SelectionListPrefs,
};

use crate::fonts::{self, Chosen, EMBEDDED_MONO, EMBEDDED_PROPORTIONAL};
use crate::request::Request;

/// As três opções de tema, na ordem em que aparecem no controle segmentado.
///
/// Ícone **e** texto: o ícone dá a leitura instantânea, o texto dispensa adivinhar o símbolo. Os
/// ícones vêm da fonte embutida (Heroicons) — não são emoji, então o desenho é o mesmo em todo SO.
const THEME_OPTIONS: [(crate::icons::Icon, GuiTheme, &str); 3] = [
    (crate::icons::Icon::ThemeSystem, GuiTheme::System, "System"),
    (crate::icons::Icon::ThemeLight, GuiTheme::Light, "Light"),
    (crate::icons::Icon::ThemeDark, GuiTheme::Dark, "Dark"),
];

/// Largura do menu de categorias da aba Settings.
///
/// **Fixa**, como a sidebar de servidores: o menu é o mapa da aba (não rola junto com o conteúdo) e a
/// largura não muda de categoria para categoria — mudar seria a tela "pulando" a cada clique.
const CATEGORY_WIDTH: f32 = 216.0;

/// Altura de uma entrada do menu de categorias.
const CATEGORY_ROW: f32 = 26.0;

/// Respiro entre o menu de categorias e o painel da direita.
///
/// O painel já desenha a própria divisória; sem o respiro o primeiro texto do painel encosta
/// nela e a tela parece colada. Vinte pontos (a `Margin` do egui soma o respiro próprio do
/// painel, então a distância que se vê entre a divisória e o texto fica em ~24) — é a folga do
/// desenho aprovado, e é o que separa de fato as duas colunas nesta aba.
const CONTENT_GAP: i8 = 20;

/// Uma categoria da aba Settings (o que o menu da esquerda escolhe).
///
/// A ordem de [`Category::ALL`] é a do menu. A categoria escolhida **não** é gravada: a aba abre
/// sempre na primeira, e o arquivo guarda preferências, não onde o dono parou de olhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Category {
    /// Tema, fontes e tamanho do texto.
    #[default]
    Appearance,
    /// Paginação da lista de bases (aba Query).
    DatabaseList,
    /// Perfis de IA (modelo, endpoint, chave).
    Ai,
}

impl Category {
    /// Todas, na ordem do menu.
    const ALL: [Category; 3] = [Category::Appearance, Category::DatabaseList, Category::Ai];

    /// Rótulo do menu (só texto: o ícone vem da fonte embutida, nunca emoji).
    fn label(self) -> &'static str {
        match self {
            Category::Appearance => "Appearance",
            Category::DatabaseList => "Database list",
            Category::Ai => "AI profiles",
        }
    }

    /// Ícone do menu. Cada categoria com o **seu** (três iguais não se distinguem de relance).
    fn icon(self) -> crate::icons::Icon {
        match self {
            // O sol é o ícone do tema claro: a categoria é a aparência da janela.
            Category::Appearance => crate::icons::Icon::ThemeLight,
            Category::DatabaseList => crate::icons::Icon::List,
            Category::Ai => crate::icons::Icon::Sparkles,
        }
    }

    /// Subtítulo do painel: uma linha dizendo do que a categoria trata.
    fn hint(self) -> &'static str {
        match self {
            Category::Appearance => "Theme, fonts and text size of the window.",
            Category::DatabaseList => "How the database list in the Query tab is drawn.",
            Category::Ai => {
                "Models for the AI that writes SQL in the Query tab. The key goes to the system vault."
            }
        }
    }
}

/// O que a categoria desenhada pediu neste quadro.
#[derive(Debug, Default)]
struct Outcome {
    /// Pedido que não é da seção `gui` (chave no cofre, `Test` do perfil, gravar perfis).
    request: Option<Request>,
    /// Mudou algo que se grava (a seção `gui` inteira).
    changed: bool,
    /// Aplicar já (prévia ao vivo de tema/fonte).
    apply_now: bool,
}

/// Uma entrada do menu de categorias: fundo de **largura inteira** na ativa (e no hover), ícone da
/// fonte embutida e rótulo alinhados à esquerda.
///
/// Desenhada à mão pelo mesmo motivo do [`crate::tabs::tab`]: o `selectable_label` do egui encolhe
/// até o texto, e o realce de um menu tem de ir de ponta a ponta.
fn category_item(ui: &mut egui::Ui, category: Category, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, CATEGORY_ROW), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);

    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }

    let job =
        crate::icons::icon_and_text(ui, category.icon(), category.label(), egui::TextStyle::Body);
    let galley = ui.painter().layout_job(job);
    let padding = egui::vec2(10.0, 0.0);
    ui.painter().galley(
        egui::pos2(
            rect.left() + padding.x,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        visuals.fg_stroke.color,
    );

    response
}

/// Posição da escolha no controle (o desconhecido cai em `System`, que é o default).
fn theme_index(theme: GuiTheme) -> usize {
    THEME_OPTIONS
        .iter()
        .position(|(_, option, _)| *option == theme)
        .unwrap_or(0)
}

/// Estado da aba Settings.
#[derive(Debug, Default)]
pub struct State {
    /// Escolha em edição (aplicada ao vivo e gravada ao solver os controles).
    chosen: Chosen,
    /// Tema da janela em edição.
    theme: GuiTheme,
    /// Paginação da lista de bases (aba Query).
    selection_list: SelectionListPrefs,
    /// Famílias instaladas no sistema (`None` = ainda não carregadas).
    families: Option<Vec<String>>,
    /// Aviso do que não pôde ser aplicado (fonte ausente).
    warning: Option<String>,
    /// Perfis de IA (aba Settings › AI). Desta tela: é ela que edita a lista.
    ai_profiles: Vec<mustel_core::config::model::AiProfile>,
    /// `id` do perfil ativo.
    ai_active: String,
    /// Formulário de perfil aberto (`None` = fechado).
    ai_form: Option<AiForm>,
    /// Perfil esperando o segundo toque para remover.
    ai_confirming_remove: Option<String>,
    /// Último recado da seção de IA ("Profile saved", erro do `Test`).
    ai_message: Option<String>,
    /// Categoria aberta no menu da esquerda.
    category: Category,
    /// Recado do atalho do arquivo de configuração (arquivo ausente, editor que não abriu).
    config_hint: Option<String>,
}

/// O formulário de um perfil de IA (nome, endpoint, modelo, temperatura e chave).
#[derive(Debug, Clone, Default)]
struct AiForm {
    /// `id` do perfil em edição (`None` = perfil novo).
    original: Option<String>,
    /// Rótulo mostrado no combo.
    name: String,
    /// Base do endpoint.
    base_url: String,
    /// Modelo.
    model: String,
    /// Temperatura.
    temperature: f32,
    /// Chave digitada (vazia = manter a que está no cofre).
    key: String,
    /// Já existe chave guardada para este perfil?
    key_present: bool,
}

impl State {
    /// Estado com as preferências que estavam gravadas.
    pub fn with_prefs(prefs: Option<&GuiPrefs>) -> Self {
        Self {
            chosen: Chosen::from_prefs(prefs),
            theme: prefs.map(|prefs| prefs.theme).unwrap_or_default(),
            selection_list: prefs
                .map(|prefs| prefs.selection_list.clone())
                .unwrap_or_default(),
            ai_profiles: prefs
                .map(|prefs| prefs.ai.profiles.clone())
                .unwrap_or_default(),
            ai_active: prefs
                .map(|prefs| prefs.ai.active_profile.clone())
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// A escolha atual (o app usa para aplicar no contexto).
    pub fn chosen(&self) -> &Chosen {
        &self.chosen
    }

    /// O tema em edição (aplicado na abertura e a cada commit).
    pub fn theme(&self) -> GuiTheme {
        self.theme
    }

    /// Aplica o que voltou de um job.
    ///
    /// Nada aqui é da aparência: descoberta de bases e `Test` do seletor vivem em
    /// `screens::selectors`. O método fica porque o app chama `apply` em todas as telas.
    pub fn apply(&mut self, result: &crate::request::JobResult) {
        // O `Test` de perfil devolve o recado para a seção de IA (a lista continua na tela).
        if let crate::request::JobResult::AiTested { result, .. } = result {
            self.ai_message = Some(match result {
                Ok(message) => message.clone(),
                Err(error) => error.clone(),
            });
        }
    }

    /// A seção `gui` completa do estado: aparência **e** queries salvas.
    ///
    /// Gravar é escrever a seção inteira, então montar só a aparência apagaria as queries.
    pub fn prefs(&self) -> GuiPrefs {
        let mut prefs = GuiPrefs::default();
        self.chosen.apply_to(&mut prefs);
        prefs.theme = self.theme;
        // `selections` **não** é desta tela: quem monta a seção `gui` inteira é o app, juntando a
        // aparência (daqui) com os seletores (do cadastro em Connections) e a paginação (daqui).
        prefs.selection_list = self.selection_list.clone();
        // A seção `ai` é desta tela: sem isto, gravar a aparência apagaria os perfis.
        prefs.ai = mustel_core::config::model::AiPrefs {
            profiles: self.ai_profiles.clone(),
            active_profile: self.ai_active.clone(),
        };
        prefs
    }

    /// Guarda um aviso vindo da aplicação (fonte ausente, por exemplo).
    pub fn set_warning(&mut self, warning: Option<String>) {
        self.warning = warning;
    }

    /// A lista de perfis mudou no app (gravada): a tela passa a mostrar a mesma.
    pub fn set_ai(&mut self, profiles: Vec<mustel_core::config::model::AiProfile>, active: String) {
        self.ai_profiles = profiles;
        self.ai_active = active;
    }

    /// Desenha a aba e devolve o pedido de gravação quando algo mudou.
    ///
    /// `servers` é o **cadastro**: a seção de queries de seleção só escolhe entre servidores já
    /// cadastrados (nunca inventa um).
    ///
    /// A aba é **menu fixo + painel**: o menu ([`Category`]) não rola com o conteúdo e não depende
    /// da categoria para existir. Só o painel da direita rola, e só quando não cabe.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
        // Menu de categorias: painel próprio (desenha o fundo e a divisória), como a sidebar de
        // servidores da aba Query.
        egui::Panel::left("settings_categories")
            .exact_size(CATEGORY_WIDTH)
            .show(ui, |ui| self.categories_ui(ui));

        // Painel da categoria escolhida. Barra **sólida**: a flutuante só aparece enquanto se rola e
        // some depois; com a sólida, quem tem conteúdo abaixo vê que tem.
        let outcome = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: CONTENT_GAP,
                ..egui::Margin::ZERO
            })
            .show(ui, |ui| {
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                egui::ScrollArea::vertical()
                    .id_salt("settings_content")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.category_ui(ui))
            })
            .inner
            .inner;

        // O pedido da categoria (chave no cofre, `Test`, gravar perfis) tem precedência: ele é de
        // outra seção e não pode esperar o commit da aparência.
        if let Some(request) = outcome.request {
            return Some(request);
        }
        // Gravar é o "commit": persiste **e** aplica. Sem isso, trocar a fonte mandava só
        // `SaveGuiPrefs` (porque `changed` também era verdadeiro) e a janela só mudava quando o
        // tamanho era mexido — era o bug relatado.
        decision(
            self.chosen.clone(),
            self.prefs(),
            outcome.changed,
            outcome.apply_now,
        )
    }

    /// Abre o `mustel.jsonc` no editor do sistema.
    ///
    /// O arquivo guarda as preferências e o cadastro; editar à mão é gesto de quem quer o arquivo,
    /// não uma explicação de onde ele está. A decisão do que dizer fica em [`config_open_hint`]
    /// (pura); aqui só se dispara o processo.
    fn open_config_file(&mut self) {
        let Some(path) = mustel_core::config::paths::config_file_path() else {
            self.config_hint = Some("The system has no config directory.".to_string());
            return;
        };

        match config_open_hint(&path) {
            // Arquivo ausente (nada foi gravado ainda): explica em vez de abrir um editor vazio —
            // `notepad` abriria uma janela perguntando se pode criar, e `xdg-open` falharia calado.
            Some(hint) => self.config_hint = Some(hint),
            None => {
                self.config_hint = crate::desktop::open_file(&path)
                    .err()
                    .map(|error| format!("Could not open {}: {error}", path.display()));
            }
        }
    }

    /// O **menu** de categorias: uma entrada por categoria, com o fundo de seleção ocupando a
    /// largura toda.
    ///
    /// O fundo é desenhado à mão em vez de usar `selectable_label` porque o rótulo selecionável do
    /// egui encolhe até o texto — numa coluna de menu, o realce precisa ir de ponta a ponta para ler
    /// como "esta é a seção aberta".
    fn categories_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("SETTINGS").small().weak());
        ui.add_space(6.0);

        for category in Category::ALL {
            if category_item(ui, category, self.category == category).clicked() {
                self.category = category;
            }
        }

        // Rodapé: **atalho para o arquivo**, e não uma frase dizendo onde ele fica (a frase saiu:
        // quem quer editar à mão quer o arquivo aberto, não o caminho escrito). Vale para as três
        // categorias, então mora aqui.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(2.0);
            if let Some(hint) = &self.config_hint {
                ui.weak(hint);
            }
            if ui
                .link("Edit in mustel.jsonc")
                .on_hover_text("Open the configuration file in the system editor")
                .clicked()
            {
                self.open_config_file();
            }
        });
    }

    /// O painel da categoria aberta.
    fn category_ui(&mut self, ui: &mut egui::Ui) -> Outcome {
        let mut outcome = Outcome::default();

        // Cabeçalho do painel: o nome da categoria e do que ela trata — é o que confirma que o
        // clique no menu trocou de assunto. O respiro do topo evita o título encostado na
        // divisória das abas.
        ui.add_space(8.0);
        ui.heading(self.category.label());
        ui.label(self.category.hint());
        ui.separator();

        match self.category {
            Category::Appearance => self.appearance_ui(ui, &mut outcome),
            Category::DatabaseList => self.database_list_ui(ui, &mut outcome),
            Category::Ai => outcome.request = self.ai_section_ui(ui),
        }

        outcome
    }

    /// Categoria **Appearance**: tema, fontes e tamanho do texto.
    fn appearance_ui(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        // Carrega as famílias na primeira renderização: enumerar o sistema custa centenas de ms.
        if self.families.is_none() {
            self.families = Some(fonts::system_families());
            // Pede um quadro novo para o combo já aparecer completo.
            ui.ctx().request_repaint();
        }
        let families = self.families.clone().unwrap_or_default();

        let changed = &mut outcome.changed;
        let apply_now = &mut outcome.apply_now;

        // Primeiro item da categoria: o tema da **janela** (claro, escuro ou seguindo o SO).
        ui.label("Theme");
        let selected = theme_index(self.theme);
        let items: Vec<(Option<crate::icons::Icon>, &str)> = THEME_OPTIONS
            .iter()
            .map(|(icon, _, label)| (Some(*icon), *label))
            .collect();
        if let Some(index) = crate::tabs::segmented_with_icons(ui, &items, selected) {
            let theme = THEME_OPTIONS[index].1;
            if theme != self.theme {
                self.theme = theme;
                // Clicar é escolha concluída: aplica **e** grava (o commit tem precedência).
                *apply_now = true;
                *changed = true;
            }
        }
        ui.weak("Follow system uses the operating system's light or dark setting.");
        ui.separator();

        *changed |= self.family_combo(
            ui,
            Combo {
                label: "Interface font",
                id: "settings_interface_font",
                embedded: EMBEDDED_PROPORTIONAL,
                which: Family::Interface,
            },
            &families,
            apply_now,
        );
        *changed |= self.family_combo(
            ui,
            Combo {
                label: "Editor font",
                id: "settings_editor_font",
                embedded: EMBEDDED_MONO,
                which: Family::Editor,
            },
            &families,
            apply_now,
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
            *apply_now = true;
        }
        // Grava quando o usuário **solta** o controle: arrastar não pode gerar uma escrita por pixel.
        if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
            *changed = true;
        }
        ui.weak(format!(
            "Between {MIN_FONT_SCALE:.2}x and {MAX_FONT_SCALE:.2}x. The window's minimum size grows with the font."
        ));

        ui.separator();
        if ui.button("Restore defaults").clicked() {
            self.restore_defaults();
            *apply_now = true;
            *changed = true;
        }

        if let Some(warning) = &self.warning {
            ui.colored_label(crate::theme::warning(ui), warning);
        }
    }

    /// Categoria **Database list**: liga/desliga a paginação e define o tamanho da página.
    ///
    /// Desligada por padrão: com o filtro de servidor, a lista completa é utilizável, e paginar sem
    /// necessidade esconde informação. Quem tiver muitos milhares liga aqui.
    fn database_list_ui(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        outcome.changed |= self.selection_list_ui(ui);
    }

    /// A seção **AI**: lista de perfis, formulário e `Test`.
    ///
    /// Devolve pedidos que **não** são da seção `gui` (gravar a chave no cofre, testar o perfil);
    /// mexer na lista em si apenas marca `changed`, e o commit grava a seção inteira.
    fn ai_section_ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
        let mut request = None;

        if let Some(message) = &self.ai_message {
            ui.weak(message);
        }

        // Lista.
        let mut activate: Option<String> = None;
        let mut edit: Option<usize> = None;
        let mut remove: Option<String> = None;
        // Cada perfil é um **cartão**: rádio (o ativo), nome, o resumo logo abaixo e as ações na
        // ponta. Antes era uma linha corrida com o resumo colado no nome — com dois perfis de nomes
        // parecidos, era difícil dizer qual linha era qual.
        for (index, profile) in self.ai_profiles.iter().enumerate() {
            let active = profile.id == self.ai_active;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .radio(active, "")
                        .on_hover_text("Use this profile")
                        .clicked()
                    {
                        activate = Some(profile.id.clone());
                    }
                    ui.vertical(|ui| {
                        ui.strong(&profile.name);
                        ui.weak(format!("{} · {}", profile.model, profile.base_url));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let confirming = self.ai_confirming_remove.as_deref() == Some(&profile.id);
                        let label = if confirming {
                            "Confirm remove"
                        } else {
                            "Remove"
                        };
                        if ui.button(label).clicked() {
                            remove = Some(profile.id.clone());
                        }
                        if ui.button("Edit").clicked() {
                            edit = Some(index);
                        }
                        if ui
                            .button("Test")
                            .on_hover_text("Send a tiny request")
                            .clicked()
                        {
                            request = Some(Request::TestAiProfile {
                                profile: profile.id.clone(),
                            });
                        }
                    });
                });
            });
        }

        if let Some(id) = activate
            && id != self.ai_active
        {
            self.ai_active = id;
            request = Some(Request::SaveAiProfiles {
                profiles: self.ai_profiles.clone(),
                active: self.ai_active.clone(),
                key: None,
            });
        }
        if let Some(index) = edit {
            let profile = &self.ai_profiles[index];
            self.ai_form = Some(AiForm {
                original: Some(profile.id.clone()),
                name: profile.name.clone(),
                base_url: profile.base_url.clone(),
                model: profile.model.clone(),
                temperature: profile.clamped_temperature(),
                key: String::new(),
                key_present: ai_has_key(&profile.id),
            });
        }
        if let Some(id) = remove {
            if self.ai_confirming_remove.as_deref() == Some(&id) {
                self.ai_profiles.retain(|profile| profile.id != id);
                self.ai_confirming_remove = None;
                if self.ai_active == id {
                    self.ai_active = self
                        .ai_profiles
                        .first()
                        .map(|p| p.id.clone())
                        .unwrap_or_default();
                }
                self.ai_message = Some("Profile removed.".to_string());
                // A chave do perfil sai junto: perfil removido não deixa segredo órfão.
                request = Some(Request::RemoveAiKey { profile: id });
            } else {
                self.ai_confirming_remove = Some(id);
            }
        }

        if ui.button("Add profile").clicked() {
            self.ai_form = Some(AiForm {
                base_url: mustel_core::config::model::AiProfile::default().base_url,
                temperature: mustel_core::config::model::AiProfile::default().temperature,
                ..AiForm::default()
            });
        }

        // Formulário.
        let mut saved = false;
        let mut cancelled = false;
        if let Some(form) = &mut self.ai_form {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(if form.original.is_some() {
                    "Edit profile"
                } else {
                    "New profile"
                });
                egui::Grid::new("ai_profile_form")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut form.name);
                        ui.end_row();

                        ui.label("Base URL");
                        ui.text_edit_singleline(&mut form.base_url);
                        ui.end_row();

                        ui.label("Model");
                        ui.text_edit_singleline(&mut form.model);
                        ui.end_row();

                        ui.label("Temperature");
                        ui.add(
                            egui::DragValue::new(&mut form.temperature)
                                .range(
                                    *mustel_core::config::model::AI_TEMPERATURE_RANGE.start()
                                        ..=*mustel_core::config::model::AI_TEMPERATURE_RANGE.end(),
                                )
                                .speed(0.05)
                                .fixed_decimals(2),
                        );
                        ui.end_row();

                        ui.label("API key");
                        ui.add(
                            egui::TextEdit::singleline(&mut form.key)
                                .password(true)
                                .hint_text(if form.key_present {
                                    "stored in the vault — type to replace"
                                } else {
                                    "paste the key"
                                }),
                        );
                        ui.end_row();
                    });

                ui.horizontal(|ui| {
                    if ui.button("Save profile").clicked() {
                        saved = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
        }

        if cancelled {
            self.ai_form = None;
        }
        if saved {
            self.apply_ai_form(&mut request);
        }

        request
    }

    /// Aplica o formulário de perfil (cria ou edita) e pede a gravação.
    fn apply_ai_form(&mut self, request: &mut Option<Request>) {
        let Some(form) = self.ai_form.take() else {
            return;
        };

        let name = form.name.trim().to_string();
        if name.is_empty() {
            self.ai_message = Some("The profile needs a name.".to_string());
            self.ai_form = Some(form);
            return;
        }
        if form.model.trim().is_empty() || form.base_url.trim().is_empty() {
            self.ai_message = Some("The profile needs a base URL and a model.".to_string());
            self.ai_form = Some(form);
            return;
        }

        // O `id` é a conta no cofre: editar mantém o original; criar deriva do nome.
        let id = form
            .original
            .clone()
            .unwrap_or_else(|| slug(&name, &self.ai_profiles));
        let profile = mustel_core::config::model::AiProfile {
            id: id.clone(),
            name,
            base_url: form.base_url.trim().to_string(),
            model: form.model.trim().to_string(),
            temperature: form.temperature,
        };

        match self.ai_profiles.iter_mut().find(|other| other.id == id) {
            Some(existing) => *existing = profile,
            None => self.ai_profiles.push(profile),
        }
        if self.ai_active.is_empty() {
            self.ai_active = id.clone();
        }

        // Perfil **e** chave no mesmo pedido: mandar só a chave deixava o perfil fora do arquivo,
        // e o `Test` (que procura na lista do app) dizia que ele não existia.
        let key = if form.key.trim().is_empty() {
            None
        } else {
            Some((id, form.key.trim().to_string()))
        };
        request.replace(Request::SaveAiProfiles {
            profiles: self.ai_profiles.clone(),
            active: self.ai_active.clone(),
            key,
        });

        self.ai_message = Some("Profile saved.".to_string());
    }

    /// Repõe aparência **e** tema no de fábrica (`System`).
    fn restore_defaults(&mut self) {
        self.chosen = Chosen::default();
        self.theme = GuiTheme::System;
    }

    /// A seção **Database list**: liga/desliga a paginação e define o tamanho da página.
    ///
    /// Devolve `true` quando mudou (o app grava a seção `gui` inteira, como no resto da aba).
    fn selection_list_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        if ui
            .checkbox(
                &mut self.selection_list.paginate,
                "Paginate the database list",
            )
            .changed()
        {
            changed = true;
        }

        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.selection_list.paginate, |ui| {
                ui.label("Page size");
                let mut size = self.selection_list.clamped_page_size();
                if ui
                    .add(
                        egui::DragValue::new(&mut size)
                            .range(MIN_PAGE_SIZE..=MAX_PAGE_SIZE)
                            .speed(1.0),
                    )
                    .changed()
                {
                    self.selection_list.page_size = size;
                    changed = true;
                }
            });
        });

        ui.weak(if self.selection_list.paginate {
            "The list shows one page at a time, with a button to load the next."
        } else {
            "Off: every database in the filter is drawn at once."
        });

        changed
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

        let current = match which {
            Family::Interface => self.chosen.interface.clone(),
            Family::Editor => self.chosen.editor.clone(),
        };
        // O botão mostra a escolha; sem escolha mostra o nome da embutida (é o que vale).
        let shown = current.clone().unwrap_or_else(|| embedded.to_string());

        // A lista: a embutida, a Fira Code garantida e as instaladas. A **mesma** busca vale para
        // as três listas (fontes, bases e queries salvas).
        let mut texts: Vec<String> = vec![embedded.to_string()];
        if !families.iter().any(|name| name == "Fira Code") {
            texts.push("Fira Code".to_string());
        }
        texts.extend(families.iter().cloned());

        let items: Vec<crate::search_combo::Item<'_>> = texts
            .iter()
            .map(|text| crate::search_combo::Item::new(text.as_str(), text == &shown))
            .collect();

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(label);
            if let Some(index) = crate::search_combo::search_combo(ui, id, &shown, &items, true) {
                let picked = &texts[index];
                let value = if picked == embedded {
                    None
                } else {
                    Some(picked.clone())
                };
                match which {
                    Family::Interface => self.chosen.interface = value,
                    Family::Editor => self.chosen.editor = value,
                }
                *apply_now = true;
                changed = true;
            }
        });

        changed
    }
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

/// O perfil já tem chave no cofre? (só o **booleano**: a chave nunca volta para a tela)
///
/// A leitura acontece quando o formulário abre, não por quadro: o cofre é lento e não muda sozinho.
fn ai_has_key(profile_id: &str) -> bool {
    mustel_core::usecases::ai::ai_key(
        &mustel_core::credentials::SystemKeyring::with_service(
            mustel_core::usecases::ai::AI_SERVICE_NAME,
        ),
        profile_id,
    )
    .map(|key| key.is_some())
    .unwrap_or(false)
}

/// Um `id` estável a partir do nome, sem colidir com os existentes.
fn slug(name: &str, existing: &[mustel_core::config::model::AiProfile]) -> String {
    let base: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            // Acento vira a letra sem acento: "Rápido" → "rapido", não "r-pido".
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            _ => '-',
        })
        .collect();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() {
        "profile".to_string()
    } else {
        base
    };

    if !existing.iter().any(|profile| profile.id == base) {
        return base;
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if !existing.iter().any(|profile| profile.id == candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// O que o atalho do rodapé diz antes de abrir o arquivo, ou `None` quando é para abrir.
///
/// Pura de propósito: a decisão depende só do caminho (`is_file`), então dá para testar os dois
/// casos sem tocar no ambiente nem abrir editor nenhum.
fn config_open_hint(path: &std::path::Path) -> Option<String> {
    if path.is_file() {
        return None;
    }
    Some(format!(
        "{} does not exist yet — change a setting to save it.",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
    ))
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

    /// Toda categoria tem rótulo, ícone próprio e um subtítulo — e o ícone existe na fonte.
    ///
    /// Sem isto, uma categoria nova entra sem ícone (cai no `?` do fallback) ou repete o da vizinha
    /// (duas entradas idênticas de relance).
    /// CONTRACT: IF-248
    #[test]
    fn every_category_has_its_own_icon_and_label() {
        assert_eq!(
            Category::ALL.len(),
            3,
            "Appearance, Database list, AI profiles"
        );
        assert_eq!(
            Category::default(),
            Category::Appearance,
            "a aba abre na primeira categoria"
        );

        for (index, category) in Category::ALL.iter().enumerate() {
            assert!(
                !category.label().trim().is_empty(),
                "rótulo vazio na posição {index}"
            );
            assert!(
                !category.hint().trim().is_empty(),
                "subtítulo de {category:?}"
            );
            assert_ne!(
                category.icon().glyph(),
                '?',
                "o ícone de {category:?} não existe no Heroicons"
            );
            let same = Category::ALL
                .iter()
                .filter(|other| other.icon() == category.icon())
                .count();
            assert_eq!(same, 1, "ícone repetido em {category:?}");
        }
    }

    /// O menu fica **à esquerda** e o conteúdo da categoria **à direita** dele.
    ///
    /// É a geometria do desenho aprovado: a coluna do menu é fixa e o painel não a invade. O menu
    /// pinta ícone + rótulo num galley só, então o texto procurado é o do fim da string.
    #[test]
    fn the_category_menu_is_a_fixed_column_on_the_left() {
        let ctx = egui::Context::default();
        let mut state = State::with_prefs(None);
        state.families = Some(Vec::new());

        let mut painted: Vec<(String, egui::Pos2)> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(screen(900.0, 700.0), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = state.ui(ui);
                });
            });
            painted.clear();
            collect_text(&output.shapes, &mut painted);
            output.textures_delta.clear();
        }

        for category in Category::ALL {
            let label = category.label();
            assert!(
                painted
                    .iter()
                    .any(|(text, pos)| text.ends_with(label) && pos.x < CATEGORY_WIDTH),
                "{label:?} devia estar na coluna do menu: {painted:?}"
            );
        }
        // O painel da categoria aberta começa **depois** da coluna do menu.
        assert!(
            painted
                .iter()
                .any(|(text, pos)| text == Category::Appearance.label() && pos.x > CATEGORY_WIDTH),
            "o painel começa à direita do menu: {painted:?}"
        );
    }

    /// O atalho só manda abrir quando o arquivo existe; sem ele, o rodapé explica o que fazer.
    ///
    /// `notepad` abriria uma janela perguntando se pode criar o arquivo, e `xdg-open`/`open`
    /// falhariam em silêncio — por isso a decisão é tomada antes do `spawn`.
    #[test]
    fn the_config_link_explains_a_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("mustel.jsonc");
        let hint = config_open_hint(&missing).expect("arquivo ausente tem recado");
        assert!(
            hint.contains("mustel.jsonc") && hint.contains("does not exist"),
            "o recado nomeia o arquivo e diz o que fazer: {hint}"
        );

        std::fs::write(&missing, "{}").expect("escreve");
        assert_eq!(
            config_open_hint(&missing),
            None,
            "com o arquivo no disco, é para abrir (sem recado)"
        );
    }

    /// O rodapé do menu é um **atalho para o arquivo**, não uma frase sobre onde ele fica.
    ///
    /// A frase antiga ("Stored in the gui section of mustel.jsonc") saiu a pedido do dono: quem
    /// procura o rodapé quer abrir o arquivo. O clique **não** é exercitado aqui — ele dispara um
    /// processo (`notepad`/`xdg-open`), que teste não abre; o comando é conferido em
    /// `desktop::tests`.
    /// CONTRACT: IF-248
    #[test]
    fn the_menu_footer_links_to_the_config_file() {
        let ctx = egui::Context::default();
        let mut state = State::with_prefs(None);
        state.families = Some(Vec::new());

        let mut painted: Vec<(String, egui::Pos2)> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(screen(900.0, 700.0), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = state.ui(ui);
                });
            });
            painted.clear();
            collect_text(&output.shapes, &mut painted);
            output.textures_delta.clear();
        }

        assert!(
            painted
                .iter()
                .any(|(text, _)| text == "Edit in mustel.jsonc"),
            "o rodapé é o atalho para o arquivo: {painted:?}"
        );
        assert!(
            !painted
                .iter()
                .any(|(text, _)| text.contains("Stored in the gui section")),
            "a frase antiga saiu: {painted:?}"
        );
    }

    /// Há **respiro** entre o menu e o painel: o texto não encosta na divisória.
    ///
    /// Era o pedido do dono depois do menu entrar (as colunas ficavam coladas). O número é do
    /// `CONTENT_GAP` desta tela, e o teste mede a tinta: a primeira letra do título tem de começar
    /// depois da coluna do menu, com folga.
    /// CONTRACT: IF-248
    #[test]
    fn the_panel_keeps_a_gap_from_the_menu() {
        let ctx = egui::Context::default();
        let mut state = State::with_prefs(None);
        state.families = Some(Vec::new());

        let mut painted: Vec<(String, egui::Pos2)> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(screen(900.0, 700.0), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = state.ui(ui);
                });
            });
            painted.clear();
            collect_text(&output.shapes, &mut painted);
            output.textures_delta.clear();
        }

        let heading = painted
            .iter()
            .find(|(text, pos)| text == Category::Appearance.label() && pos.x > CATEGORY_WIDTH)
            .expect("o título da categoria foi desenhado")
            .1;

        assert!(
            heading.x >= CATEGORY_WIDTH + f32::from(CONTENT_GAP),
            "o título começa em {} e tem de ficar ao menos {} à direita do menu",
            heading.x,
            CATEGORY_WIDTH + f32::from(CONTENT_GAP)
        );
    }

    /// Clicar numa categoria troca o painel (o menu não é enfeite).
    #[test]
    fn clicking_a_category_swaps_the_panel() {
        let ctx = egui::Context::default();
        let mut state = State::with_prefs(None);
        state.families = Some(Vec::new());

        // Primeiro quadro: desenha e descobre onde está a entrada "AI profiles".
        let mut painted: Vec<(String, egui::Pos2)> = Vec::new();
        let mut output = ctx.run_ui(screen(900.0, 700.0), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.ui(ui);
            });
        });
        collect_text(&output.shapes, &mut painted);
        output.textures_delta.clear();
        let ai = painted
            .iter()
            .find(|(text, pos)| text.ends_with(Category::Ai.label()) && pos.x < CATEGORY_WIDTH)
            .expect("a entrada da IA está no menu")
            .1;
        // O galley começa no topo do texto; o meio da linha fica alguns pixels abaixo.
        let alvo = egui::pos2(ai.x + 4.0, ai.y + 6.0);

        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(alvo),
                egui::Event::PointerButton {
                    pos: alvo,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: alvo,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..screen(900.0, 700.0)
        };
        painted.clear();
        let mut clicked = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.ui(ui);
            });
        });
        collect_text(&clicked.shapes, &mut painted);
        clicked.textures_delta.clear();

        assert_eq!(state.category, Category::Ai, "o clique troca a categoria");
        assert!(
            painted
                .iter()
                .any(|(text, _)| text.contains("Models for the AI")),
            "o painel da IA está à vista: {painted:?}"
        );
        assert!(
            !painted
                .iter()
                .any(|(text, _)| text.contains("Between 0.80x and 1.25x")),
            "o painel da aparência saiu: {painted:?}"
        );
    }

    /// O estado da aba **carrega** tudo o que está gravado — inclusive a paginação.
    ///
    /// O bug que originou este teste: `with_prefs` copiava fonte e queries e esquecia a lista,
    /// então a opção ligada no arquivo não valia na tela.
    /// CONTRACT: IF-234
    #[test]
    fn the_state_keeps_every_saved_preference() {
        let prefs = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            selection_list: mustel_core::config::model::SelectionListPrefs {
                paginate: true,
                page_size: 80,
            },
            ..GuiPrefs::default()
        };

        let state = State::with_prefs(Some(&prefs));
        assert_eq!(state.chosen().interface.as_deref(), Some("Segoe UI"));
        assert!(
            state.prefs().selection_list.paginate,
            "a paginação gravada chega no estado"
        );
        assert_eq!(state.prefs().selection_list.page_size, 80);
    }

    /// Mexer na paginação grava a seção `gui` inteira (sem perder a fonte).
    /// CONTRACT: IF-234
    #[test]
    fn changing_the_pagination_keeps_the_rest() {
        let prefs = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            ..GuiPrefs::default()
        };
        let mut state = State::with_prefs(Some(&prefs));
        state.selection_list.paginate = true;
        state.selection_list.page_size = 25;

        let merged = state.prefs();
        assert!(merged.selection_list.paginate);
        assert_eq!(merged.selection_list.page_size, 25);
        assert_eq!(merged.interface_font.as_deref(), Some("Segoe UI"));
    }

    /// O tema gravado chega na tela, e a tela o devolve ao gravar.
    /// CONTRACT: IF-243
    #[test]
    fn the_state_keeps_and_returns_the_theme() {
        let prefs = GuiPrefs {
            theme: GuiTheme::Dark,
            ..GuiPrefs::default()
        };

        let state = State::with_prefs(Some(&prefs));
        assert_eq!(theme_index(GuiTheme::Dark), 2, "dark é a terceira opção");
        assert_eq!(
            state.prefs().theme,
            GuiTheme::Dark,
            "o tema volta ao gravar"
        );
    }

    /// Cada opção do controle mapeia para um tema (e volta), sem rótulo vazio.
    /// CONTRACT: IF-243
    #[test]
    fn every_theme_option_has_a_label_and_round_trips() {
        assert_eq!(THEME_OPTIONS.len(), 3, "System, Light e Dark");

        for (index, (icon, theme, label)) in THEME_OPTIONS.iter().enumerate() {
            assert!(!label.trim().is_empty(), "rótulo vazio na posição {index}");
            // O rótulo é **só texto**: o ícone vem da fonte embutida, não é emoji dentro da string.
            assert!(
                label.chars().all(|c| c.is_alphabetic()),
                "o rótulo é só texto; o ícone é separado: {label}"
            );
            assert_eq!(theme_index(*theme), index, "posição de {theme:?}");
            // Cada opção tem um ícone próprio (senão duas parecem a mesma).
            let same = THEME_OPTIONS
                .iter()
                .filter(|(other, _, _)| other == icon)
                .count();
            assert_eq!(same, 1, "ícone repetido em {icon:?}");
        }
    }

    /// A seção `ai` sai do estado da tela (senão gravar aparência apagaria os perfis).
    /// CONTRACT: IF-247
    #[test]
    fn the_ai_profiles_come_from_and_go_back_to_the_prefs() {
        let profile = mustel_core::config::model::AiProfile {
            id: "rapido".to_string(),
            name: "Rápido".to_string(),
            model: "gpt-4o-mini".to_string(),
            ..mustel_core::config::model::AiProfile::default()
        };
        let prefs = GuiPrefs {
            ai: mustel_core::config::model::AiPrefs {
                profiles: vec![profile.clone()],
                active_profile: "rapido".to_string(),
            },
            ..GuiPrefs::default()
        };

        let state = State::with_prefs(Some(&prefs));
        let back = state.prefs();

        assert_eq!(back.ai.profiles, vec![profile]);
        assert_eq!(back.ai.active_profile, "rapido");
    }

    /// Criar perfil deriva um `id` estável e não colide; editar mantém o `id` e a posição.
    #[test]
    fn saving_a_profile_creates_then_replaces_in_place() {
        let mut state = State::with_prefs(None);

        state.ai_form = Some(AiForm {
            name: "Rápido".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.2,
            ..AiForm::default()
        });
        let mut request = None;
        state.apply_ai_form(&mut request);

        assert_eq!(state.ai_profiles.len(), 1);
        assert_eq!(state.ai_profiles[0].id, "rapido");
        assert_eq!(state.ai_profiles[0].temperature, 0.2);
        assert_eq!(state.ai_active, "rapido", "o primeiro vira o ativo");
        assert!(matches!(request, Some(Request::SaveAiProfiles { .. })));

        // Editar mantém o id (é a conta do cofre) e não duplica.
        let id = state.ai_profiles[0].id.clone();
        state.ai_form = Some(AiForm {
            original: Some(id.clone()),
            name: "Rápido 2".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            ..AiForm::default()
        });
        let mut request = None;
        state.apply_ai_form(&mut request);

        assert_eq!(state.ai_profiles.len(), 1);
        assert_eq!(state.ai_profiles[0].id, id);
        assert_eq!(state.ai_profiles[0].name, "Rápido 2");
        assert_eq!(state.ai_profiles[0].model, "gpt-4o");
    }

    /// A chave digitada vai **junto** da lista (o perfil precisa existir para o `Test`).
    #[test]
    fn a_typed_key_goes_to_the_vault_and_an_empty_one_keeps_it() {
        let mut state = State::with_prefs(None);

        state.ai_form = Some(AiForm {
            name: "A".to_string(),
            base_url: "https://x/v1".to_string(),
            model: "m".to_string(),
            key: "  sk-nova  ".to_string(),
            ..AiForm::default()
        });
        let mut request = None;
        state.apply_ai_form(&mut request);

        match request {
            Some(Request::SaveAiProfiles { key, .. }) => {
                let (profile, secret) = key.expect("a chave foi junto");
                assert_eq!(profile, "a");
                assert_eq!(secret, "sk-nova", "a chave é aparada");
            }
            other => panic!("pedido errado: {other:?}"),
        }

        // Sem chave digitada: grava a lista.
        state.ai_form = Some(AiForm {
            original: Some("a".to_string()),
            name: "A".to_string(),
            base_url: "https://x/v1".to_string(),
            model: "m".to_string(),
            ..AiForm::default()
        });
        let mut request = None;
        state.apply_ai_form(&mut request);
        match request {
            Some(Request::SaveAiProfiles { key, profiles, .. }) => {
                assert!(key.is_none(), "sem chave digitada, nada vai para o cofre");
                assert_eq!(profiles.len(), 1, "e o perfil é gravado assim mesmo");
            }
            other => panic!("pedido errado: {other:?}"),
        }
    }

    /// **Regressão**: salvar com chave digitada mandava só a chave, e o perfil nunca chegava ao
    /// app nem ao arquivo — o `Test` depois respondia "That profile no longer exists.".
    #[test]
    fn saving_with_a_key_still_delivers_the_profile_list() {
        let mut state = State::with_prefs(None);
        state.ai_form = Some(AiForm {
            name: "Ollama".to_string(),
            base_url: "https://ollama.com/v1".to_string(),
            model: "deepseek-v4.1-flash".to_string(),
            temperature: 0.1,
            key: "sk-teste".to_string(),
            ..AiForm::default()
        });

        let mut request = None;
        state.apply_ai_form(&mut request);

        match request {
            Some(Request::SaveAiProfiles {
                profiles,
                active,
                key,
            }) => {
                assert_eq!(profiles.len(), 1, "o perfil vai junto com a chave");
                assert_eq!(profiles[0].id, "ollama");
                assert_eq!(active, "ollama");
                assert_eq!(key.as_ref().map(|(id, _)| id.as_str()), Some("ollama"));
            }
            other => panic!("pedido errado: {other:?}"),
        }
    }

    /// Formulário inválido não grava e explica o que falta.
    #[test]
    fn an_incomplete_profile_is_refused() {
        let mut state = State::with_prefs(None);
        state.ai_form = Some(AiForm::default());
        let mut request = None;
        state.apply_ai_form(&mut request);

        assert!(state.ai_profiles.is_empty());
        assert!(request.is_none());
        assert_eq!(
            state.ai_message.as_deref(),
            Some("The profile needs a name.")
        );
        assert!(state.ai_form.is_some(), "o formulário continua aberto");
    }

    /// O `id` derivado do nome não colide com os existentes.
    #[test]
    fn the_profile_id_does_not_collide() {
        use mustel_core::config::model::AiProfile;

        let existing = vec![
            AiProfile {
                id: "rapido".to_string(),
                ..AiProfile::default()
            },
            AiProfile {
                id: "rapido-2".to_string(),
                ..AiProfile::default()
            },
        ];

        assert_eq!(slug("Rápido", &existing), "rapido-3");
        assert_eq!(slug("Outro", &existing), "outro");
        assert_eq!(slug("!!!", &existing), "profile");
    }

    /// `Restore defaults` volta o tema para `System` (o default do produto).
    /// CONTRACT: IF-243
    #[test]
    fn restore_defaults_resets_the_theme() {
        let mut state = State::with_prefs(Some(&GuiPrefs {
            theme: GuiTheme::Dark,
            ..GuiPrefs::default()
        }));

        state.restore_defaults();

        assert_eq!(state.prefs().theme, GuiTheme::System);
        assert_eq!(
            theme_index(state.prefs().theme),
            0,
            "volta para a primeira opção"
        );
    }

    /// Entrada de teste com uma janela de tamanho conhecido.
    ///
    /// Sem `screen_rect` o egui usa uma superfície enorme (10k x 10k) e qualquer medida de largura
    /// vira ficção — foi assim que o controle segmentado apareceu com 8k de largura num teste.
    fn screen(width: f32, height: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        }
    }

    /// Junta `(texto, posição)` de todo `Shape::Text`, descendo nos grupos.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<(String, egui::Pos2)>) {
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
}
