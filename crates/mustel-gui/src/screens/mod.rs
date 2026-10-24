//! As telas da GUI (T42).
//!
//! Cada tela é **estado + desenho**: ela não fala com o núcleo nem com o tokio. O que ela faz é
//! desenhar e devolver [`Request`](crate::request::Request)s — quem executa, cancelar e devolver o
//! resultado é o [`App`](crate::app::App). Assim a tela é testável sem janela, e a regra continua toda
//! no núcleo.

pub mod query;
pub mod selectors;
pub mod servers;
pub mod settings;

/// A seção `gui` inteira, composta das telas donas de cada pedaço.
///
/// Nenhuma tela conhece a seção toda: Settings tem aparência e paginação, Connections tem os
/// seletores. Gravar o que **uma** delas conhece apagaria o resto — foi assim que a seção de
/// queries sobreviveu a uma troca de fonte (IF-229), e é o que esta função garante para os dois
/// lados.
pub fn compose_gui_prefs(
    appearance: mustel_core::config::model::GuiPrefs,
    selections: &[mustel_core::config::model::SelectionQuery],
) -> mustel_core::config::model::GuiPrefs {
    let mut prefs = appearance;
    prefs.selections = selections.to_vec();
    prefs
}

#[cfg(test)]
mod tests {
    use mustel_core::config::model::{GuiPrefs, SelectionQuery};

    /// Gravar a aparência **preserva** os seletores, e vice-versa.
    /// CONTRACT: IF-229
    #[test]
    fn composing_the_section_keeps_both_halves() {
        let saved = SelectionQuery {
            name: "Produção".to_string(),
            server: "configurador".to_string(),
            database: "configurador".to_string(),
            sql: "select host, port, database from tenants".to_string(),
        };
        let appearance = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            ..GuiPrefs::default()
        };

        let composed = super::compose_gui_prefs(appearance.clone(), std::slice::from_ref(&saved));
        assert_eq!(composed.interface_font.as_deref(), Some("Segoe UI"));
        assert_eq!(
            composed.selections,
            vec![saved.clone()],
            "os seletores ficam"
        );

        // E o contrário: sem seletores, a aparência continua inteira.
        let composed = super::compose_gui_prefs(appearance, &[]);
        assert_eq!(composed.interface_font.as_deref(), Some("Segoe UI"));
        assert!(
            composed.selections.is_empty(),
            "removidos continuam removidos"
        );
    }
}
