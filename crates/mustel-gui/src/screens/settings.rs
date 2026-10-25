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
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
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

        // Primeiro item da aba: o tema da **janela** (claro, escuro ou seguindo o SO).
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
                apply_now = true;
                changed = true;
            }
        }
        ui.weak("Follow system uses the operating system's light or dark setting.");
        ui.separator();

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
            self.restore_defaults();
            apply_now = true;
            changed = true;
        }

        if let Some(warning) = &self.warning {
            ui.colored_label(crate::theme::warning(ui), warning);
        }

        // Gravar é o "commit": persiste **e** aplica. Sem isso, trocar a fonte mandava só
        // `SaveGuiPrefs` (porque `changed` também era verdadeiro) e a janela só mudava quando o
        // tamanho era mexido — era o bug relatado.
        //
        // A segunda seção entra no mesmo commit: a seção `gui` é gravada inteira.
        // --- Paginação da lista de bases (aba Query) -------------------------------------------
        //
        // Desligada por padrão: com o filtro de servidor, a lista completa é utilizável, e paginar
        // sem necessidade esconde informação. Quem tiver muitos milhares liga aqui.
        ui.separator();
        ui.heading("Database list");
        ui.label(
            "How the database list in the Query tab is drawn. Filters and search always see the \
             whole list; pagination only limits what is drawn at once.",
        );
        changed |= self.selection_list_ui(ui);

        // --- IA (OpenAI-compatible) ---------------------------------------------------------
        ui.separator();
        ui.heading("AI");
        ui.label(
            "Profiles for the AI that writes SQL in the Query tab.              The API key goes to the system vault, never to `mustel.jsonc`.",
        );
        let ai_request = self.ai_section_ui(ui);

        if let Some(request) = ai_request {
            return Some(request);
        }

        decision(self.chosen.clone(), self.prefs(), changed, apply_now)
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
        for (index, profile) in self.ai_profiles.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .radio(profile.id == self.ai_active, &profile.name)
                    .on_hover_text("Use this profile")
                    .clicked()
                {
                    activate = Some(profile.id.clone());
                }
                ui.weak(format!("{} · {}", profile.model, profile.base_url));
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
}
