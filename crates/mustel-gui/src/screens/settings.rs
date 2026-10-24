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
/// Ícone **e** texto: o ícone dá a leitura instantânea, o texto dispensa adivinhar o símbolo. São os
/// mesmos ícones que o `egui` usa em `ThemePreference::icon`.
const THEME_OPTIONS: [(&str, GuiTheme); 3] = [
    ("💻 System", GuiTheme::System),
    ("☀️ Light", GuiTheme::Light),
    ("🌙 Dark", GuiTheme::Dark),
];

/// Posição da escolha no controle (o desconhecido cai em `System`, que é o default).
fn theme_index(theme: GuiTheme) -> usize {
    THEME_OPTIONS
        .iter()
        .position(|(_, option)| *option == theme)
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
    pub fn apply(&mut self, _result: &crate::request::JobResult) {}

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
        prefs
    }

    /// Guarda um aviso vindo da aplicação (fonte ausente, por exemplo).
    pub fn set_warning(&mut self, warning: Option<String>) {
        self.warning = warning;
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
        let labels: Vec<&str> = THEME_OPTIONS.iter().map(|(label, _)| *label).collect();
        if let Some(index) = crate::tabs::segmented(ui, &labels, selected) {
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

        decision(self.chosen.clone(), self.prefs(), changed, apply_now)
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

        for (index, (label, theme)) in THEME_OPTIONS.iter().enumerate() {
            assert!(!label.trim().is_empty(), "rótulo vazio na posição {index}");
            assert!(
                label
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !c.is_whitespace()),
                "o rótulo precisa do ícone: {label}"
            );
            assert!(
                label.contains(char::is_alphabetic),
                "falta o texto: {label}"
            );
            assert_eq!(theme_index(*theme), index, "posição de {theme:?}");
        }
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
