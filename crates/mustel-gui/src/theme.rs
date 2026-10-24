//! Tema da janela (claro/escuro/sistema) e cores semânticas.
//!
//! O tema vive em `mustel_core::config::model::GuiTheme` (dado), não aqui: esta crate só traduz a
//! escolha para o `egui` e oferece as cores de **estado** que a interface usa.
//!
//! Por que as cores de estado passam por aqui: erro, aviso e alerta eram `Color32::RED`,
//! `Color32::YELLOW` e um âmbar fixo, todos escolhidos para fundo claro. No tema escuro eles perdem
//! contraste. `danger`/`warning` delegam ao próprio tema (`error_fg_color`/`warn_fg_color`, que o
//! `egui` já troca entre claro e escuro) e `alert` tem um par de âmbares com contraste verificado nos
//! dois fundos.

use egui::{Color32, Context, ThemePreference, Ui};
use mustel_core::config::model::GuiTheme;

/// Âmbar de alerta sobre fundo claro. Escurecido em relação ao antigo `(214, 158, 46)`, que ficava em
/// ~2,4:1 sobre painel claro — abaixo do mínimo de leitura.
const ALERT_LIGHT: Color32 = Color32::from_rgb(146, 105, 10);
/// Âmbar de alerta sobre fundo escuro.
const ALERT_DARK: Color32 = Color32::from_rgb(235, 190, 100);

/// Traduz a escolha do produto para a preferência do `egui`.
pub fn preference(theme: GuiTheme) -> ThemePreference {
    match theme {
        GuiTheme::System => ThemePreference::System,
        GuiTheme::Light => ThemePreference::Light,
        GuiTheme::Dark => ThemePreference::Dark,
    }
}

/// Aplica o tema no contexto da janela.
///
/// `System` **não** fixa claro nem escuro: só devolve a decisão ao sistema operacional, que é o
/// comportamento padrão do produto.
pub fn apply(ctx: &Context, theme: GuiTheme) {
    ctx.set_theme(preference(theme));
}

/// Cor de **erro** (mensagens de falha), vinda do tema ativo.
pub fn danger(ui: &Ui) -> Color32 {
    danger_for(ui.visuals())
}

/// Cor de **aviso** (algo que impede ou degrada uma ação), vinda do tema ativo.
pub fn warning(ui: &Ui) -> Color32 {
    warning_for(ui.visuals())
}

/// Cor de **alerta** de status (ex.: lista de bases barrada por `excludePatterns`).
///
/// É estado próprio, não erro nem aviso, então não vem do `Visuals`: usa o par com contraste medido.
pub fn alert(ui: &Ui) -> Color32 {
    alert_for(ui.visuals())
}

/// Núcleo de [`danger`], sem `Ui` — é o que os testes usam para os dois temas.
pub fn danger_for(visuals: &egui::Visuals) -> Color32 {
    visuals.error_fg_color
}

/// Núcleo de [`warning`], sem `Ui`.
pub fn warning_for(visuals: &egui::Visuals) -> Color32 {
    visuals.warn_fg_color
}

/// Núcleo de [`alert`], sem `Ui`.
pub fn alert_for(visuals: &egui::Visuals) -> Color32 {
    if visuals.dark_mode {
        ALERT_DARK
    } else {
        ALERT_LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CONTRACT: IF-243
    #[test]
    fn each_product_theme_maps_to_the_egui_preference() {
        assert_eq!(preference(GuiTheme::System), ThemePreference::System);
        assert_eq!(preference(GuiTheme::Light), ThemePreference::Light);
        assert_eq!(preference(GuiTheme::Dark), ThemePreference::Dark);
    }

    /// CONTRACT: IF-243
    #[test]
    fn applying_light_and_dark_flips_the_visuals() {
        let ctx = Context::default();

        apply(&ctx, GuiTheme::Dark);
        assert_eq!(ctx.theme(), egui::Theme::Dark, "dark liga o modo escuro");

        apply(&ctx, GuiTheme::Light);
        assert_eq!(
            ctx.theme(),
            egui::Theme::Light,
            "light desliga o modo escuro"
        );
    }

    /// `System` devolve a decisão ao SO: a preferência continua `System`, sem fixar claro/escuro.
    /// CONTRACT: IF-243
    #[test]
    fn system_does_not_pin_light_or_dark() {
        let ctx = Context::default();

        apply(&ctx, GuiTheme::Dark);
        apply(&ctx, GuiTheme::System);

        let preference = ctx.options(|options| options.theme_preference);
        assert_eq!(
            preference,
            ThemePreference::System,
            "System não pode virar Light nem Dark"
        );
    }

    /// Erro e aviso vêm do tema — não há cor fixa para perder contraste no escuro.
    /// CONTRACT: IF-243
    #[test]
    fn danger_and_warning_follow_the_theme() {
        let light = egui::Visuals::light();
        let dark = egui::Visuals::dark();

        assert_eq!(danger_for(&light), light.error_fg_color);
        assert_eq!(danger_for(&dark), dark.error_fg_color);
        assert_eq!(warning_for(&light), light.warn_fg_color);
        assert_eq!(warning_for(&dark), dark.warn_fg_color);
        assert_ne!(
            warning_for(&light),
            warning_for(&dark),
            "o aviso precisa mudar com o tema (era `Color32::YELLOW` fixo)"
        );
    }

    /// O âmbar de alerta é legível nos dois fundos (WCAG AA para texto normal: 4.5:1).
    /// CONTRACT: IF-243
    #[test]
    fn the_alert_color_keeps_contrast_in_both_themes() {
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            let ratio = contrast_against_background(alert_for(&visuals), visuals.panel_fill);
            assert!(
                ratio >= 4.5,
                "tema escuro={}: contraste {ratio:.2}:1 abaixo de 4.5:1",
                visuals.dark_mode
            );
        }
        assert_ne!(
            alert_for(&egui::Visuals::light()),
            alert_for(&egui::Visuals::dark()),
            "o âmbar tem par por tema"
        );
    }

    /// Contraste WCAG entre duas cores opacas (luminância relativa).
    fn contrast_against_background(foreground: Color32, background: Color32) -> f32 {
        let (lighter, darker) = {
            let a = luminance(foreground);
            let b = luminance(background);
            if a > b { (a, b) } else { (b, a) }
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Luminância relativa da WCAG (sRGB linearizado).
    fn luminance(color: Color32) -> f32 {
        let [r, g, b, _] = color.to_normalized_gamma_f32();
        let channel = |value: f32| {
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }
}
