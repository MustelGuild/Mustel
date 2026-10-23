//! Tela **Settings**: preferências de aparência da janela (hoje, fonte).
//!
//! A tela não grava nada: devolve [`Request::SaveGuiPrefs`] e quem persiste é o app, através de
//! `usecases::gui_prefs` — a mesma separação das outras telas (`Request` → `App` → núcleo).
//!
//! A lista de famílias do sistema é cara de montar (o `fontique` varre o sistema), então é carregada
//! **uma vez**, na primeira vez que a aba abre, e guardada na tela.

use mustel_core::config::model::{MAX_FONT_SCALE, MIN_FONT_SCALE};

use crate::fonts::{self, Chosen, EMBEDDED_MONO, EMBEDDED_PROPORTIONAL};
use crate::request::Request;

/// Estado da aba Settings.
#[derive(Debug, Default)]
pub struct State {
    /// Escolha em edição (aplicada ao vivo e gravada ao soltar os controles).
    chosen: Chosen,
    /// Famílias instaladas no sistema (`None` = ainda não carregadas).
    families: Option<Vec<String>>,
    /// Aviso do que não pôde ser aplicado (fonte ausente).
    warning: Option<String>,
}

impl State {
    /// Estado com as preferências que estavam gravadas.
    pub fn with_prefs(prefs: Option<&mustel_core::config::model::GuiPrefs>) -> Self {
        Self {
            chosen: Chosen::from_prefs(prefs),
            ..Self::default()
        }
    }

    /// A escolha atual (o app usa para aplicar no contexto).
    pub fn chosen(&self) -> &Chosen {
        &self.chosen
    }

    /// Guarda um aviso vindo da aplicação (fonte ausente, por exemplo).
    pub fn set_warning(&mut self, warning: Option<String>) {
        self.warning = warning;
    }

    /// Desenha a aba e devolve o pedido de gravação quando algo mudou.
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
        decision(self.chosen.clone(), changed, apply_now)
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

/// Qual pedido a aba emite com o que o usuário fez neste quadro.
///
/// A regra é pequena mas já errou uma vez: quando a fonte muda, `changed` **e** `apply_now` são
/// verdadeiros, e emitir os dois com `if` separados fazia o segundo sobrescrever o primeiro — a fonte
/// só era aplicada no quadro em que *outro* controle mexesse. Aqui o commit tem precedência.
/// CONTRACT: IF-222
fn decision(chosen: Chosen, changed: bool, apply_now: bool) -> Option<Request> {
    if changed {
        // Commit: persistir **e** aplicar (o app aplica antes de gravar).
        Some(Request::SaveGuiPrefs(chosen.to_prefs()))
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
        match decision(chosen.clone(), true, true) {
            Some(Request::SaveGuiPrefs(prefs)) => {
                assert_eq!(prefs.interface_font.as_deref(), Some("Arial"));
            }
            other => panic!("esperava o pedido de commit, veio {other:?}"),
        }

        // Arrastar o slider: só prévia, sem gravar a cada pixel.
        match decision(chosen.clone(), false, true) {
            Some(Request::ApplyGuiPrefs(applied)) => assert_eq!(applied, chosen),
            other => panic!("esperava prévia, veio {other:?}"),
        }

        // Nada mudou: nenhum pedido.
        assert_eq!(decision(chosen, false, false), None);
    }
}
