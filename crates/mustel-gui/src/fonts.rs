//! Fontes da janela: escolher, carregar e aplicar.
//!
//! A aba **Settings** guarda nomes de família em `mustel.jsonc` (seção `gui`). Aqui esses nomes viram
//! bytes e viram `FontDefinitions` — o único lugar que sabe que existem fontes embutidas, fontes do
//! sistema e uma **cadeia de fallback**.
//!
//! Regra de ouro: a fonte escolhida entra **na frente** da família; as embutidas continuam atrás. É o
//! que mantém `⟳`, `▼`, `📂`, `🗂`, `✓` e `✗` visíveis mesmo numa fonte que não tenha esses glifos, e
//! o que mantém a janela legível se a fonte escolhida sumir do sistema.

use std::sync::Arc;

use fontique::{
    Collection, CollectionOptions, QueryFamily, QueryStatus, SourceCache, SourceCacheOptions,
};
use mustel_core::config::model::{DEFAULT_FONT_SCALE, GuiPrefs, MAX_FONT_SCALE, MIN_FONT_SCALE};

/// Fonte mono embutida (editor de SQL).
const FIRA_CODE: &[u8] = include_bytes!("../assets/fonts/FiraCode-Regular.ttf");

/// Nome apresentado no combo para a fonte embutida do editor.
pub const EMBEDDED_MONO: &str = "Fira Code (embutida)";
/// Nome apresentado no combo para a fonte embutida da interface.
pub const EMBEDDED_PROPORTIONAL: &str = "Ubuntu-Light (embutida)";

/// O que aplicar na janela, já resolvido a partir das preferências.
#[derive(Debug, Clone, PartialEq)]
pub struct Chosen {
    /// Família da interface (`None` = embutida).
    pub interface: Option<String>,
    /// Família do editor (`None` = embutida).
    pub editor: Option<String>,
    /// Escala já dentro da faixa aceita.
    pub scale: f32,
}

impl Default for Chosen {
    fn default() -> Self {
        Self {
            interface: None,
            editor: None,
            scale: DEFAULT_FONT_SCALE,
        }
    }
}

impl Chosen {
    /// Lê as preferências gravadas, tratando vazio como "embutida".
    pub fn from_prefs(prefs: Option<&GuiPrefs>) -> Self {
        let Some(prefs) = prefs else {
            return Self::default();
        };
        Self {
            interface: normalise(&prefs.interface_font),
            editor: normalise(&prefs.editor_font),
            scale: prefs.clamped_scale(),
        }
    }

    /// As preferências para gravar (vazio volta a ser `None`).
    pub fn to_prefs(&self) -> GuiPrefs {
        GuiPrefs {
            interface_font: normalise(&self.interface),
            editor_font: normalise(&self.editor),
            font_scale: self.scale,
        }
    }
}

fn normalise(name: &Option<String>) -> Option<String> {
    name.as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Famílias instaladas no sistema, em ordem alfabética.
///
/// Enumerar custa centenas de milissegundos na primeira vez (o `fontique` varre o sistema), por isso
/// isto é chamado **uma vez**, ao montar a aba Settings, e não a cada quadro.
pub fn system_families() -> Vec<String> {
    let mut collection = Collection::new(CollectionOptions::default());
    let mut names: Vec<String> = collection.family_names().map(str::to_string).collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

/// Bytes de uma família instalada, se ela existir e o arquivo puder ser lido.
///
/// Devolve o primeiro casamento do estilo padrão (weight normal, largura normal); se a família não
/// tiver esse estilo, o `fontique` sintetiza ou cai no mais próximo.
fn system_font_bytes(family: &str) -> Option<Vec<u8>> {
    let mut collection = Collection::new(CollectionOptions::default());
    let mut source_cache = SourceCache::new(SourceCacheOptions::default());
    let mut query = collection.query(&mut source_cache);
    query.set_families([QueryFamily::Named(family)]);

    let mut bytes = None;
    query.matches_with(|font| {
        bytes = Some(font.blob.data().to_vec());
        QueryStatus::Stop
    });
    bytes
}

/// Bytes da família pedida, quando ela é uma das embutidas.
fn embedded_bytes(family: &str) -> Option<&'static [u8]> {
    if family == EMBEDDED_MONO || family.eq_ignore_ascii_case("fira code") {
        return Some(FIRA_CODE);
    }
    None
}

/// Ajuste vertical dos glifos da fonte de ícones do egui (`egui-icons`).
///
/// A fonte de ícones tem métrica própria: no texto de 13 px os glifos saem ~2 px **acima** do centro
/// do texto (medido na janela: centro 89,5 contra 99,5 em escala 4x). `y_offset_factor` desloca por
/// fração do tamanho da fonte, então ~0,19 é o que devolve os ícones para a linha do texto.
const ICON_Y_OFFSET_FACTOR: f32 = 0.19;

/// Monta as `FontDefinitions` com a escolha do usuário **na frente** e as embutidas atrás.
///
/// `loaded` devolve o que foi de fato aplicado — um nome que não resolve (fonte desinstalada) não
/// derruba a janela: cai para a embutida e o chamador pode avisar.
pub fn definitions(chosen: &Chosen) -> (egui::FontDefinitions, Vec<String>) {
    let mut definitions = egui::FontDefinitions::default();
    let mut problems = Vec::new();

    // Alinha os ícones (🔍 🖥 ⚙ 📂 🗂 ⟳ ▼) com a linha do texto: a fonte de ícones do egui tem
    // métrica própria e sem isto os glifos ficam acima do texto nas abas e nos botões.
    if let Some(icons) = definitions.font_data.get_mut("egui-icons") {
        // `FontData` é imutável dentro do `Arc`: recria com o **mesmo** blob (sem copiar o arquivo)
        // e o novo tweak.
        let mut tweak = icons.tweak.clone();
        tweak.y_offset_factor = ICON_Y_OFFSET_FACTOR;
        *icons = std::sync::Arc::new(egui::FontData {
            font: icons.font.clone(),
            index: icons.index,
            tweak,
        });
    }

    let prepend = |definitions: &mut egui::FontDefinitions,
                   family: egui::FontFamily,
                   name: &Option<String>,
                   problems: &mut Vec<String>| {
        let Some(name) = name else { return };

        let bytes = embedded_bytes(name)
            .map(|bytes| bytes.to_vec())
            .or_else(|| system_font_bytes(name));

        let Some(bytes) = bytes else {
            problems.push(format!("fonte '{name}' não encontrada; usando a embutida"));
            return;
        };

        let key = format!("user:{name}");
        definitions
            .font_data
            .insert(key.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, key);
    };

    prepend(
        &mut definitions,
        egui::FontFamily::Proportional,
        &chosen.interface,
        &mut problems,
    );
    prepend(
        &mut definitions,
        egui::FontFamily::Monospace,
        &chosen.editor,
        &mut problems,
    );

    (definitions, problems)
}

/// Escala presa na faixa aceita (a tela usa antes de gravar).
pub fn clamp_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
    } else {
        DEFAULT_FONT_SCALE
    }
}

/// Escala os tamanhos de texto do estilo pela escolha do usuário.
///
/// Mexe nos `text_styles` (não em `pixels_per_point`): assim o **layout** acompanha a fonte, em vez
/// de a janela inteira ser reescalada — inclusive bordas e ícones.
pub fn scaled_style(mut style: egui::Style, scale: f32) -> egui::Style {
    let scale = if scale.is_finite() {
        scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
    } else {
        DEFAULT_FONT_SCALE
    };

    // Os tamanhos de referência são os do estilo padrão do egui.
    let base = egui::Style::default().text_styles.clone();
    for (text_style, font_id) in &base {
        if let Some(slot) = style.text_styles.get_mut(text_style) {
            slot.size = font_id.size * scale;
        }
    }

    style
}

/// Aplica a escolha no contexto: fontes e escala.
///
/// Devolve os avisos do que não pôde ser aplicado (fonte ausente), para a tela mostrar.
pub fn apply(ctx: &egui::Context, chosen: &Chosen) -> Vec<String> {
    let (definitions, problems) = definitions(chosen);
    ctx.set_fonts(definitions);

    // `all_styles_mut` cobre os dois temas (claro e escuro): a escala vale para ambos.
    let scale = chosen.scale;
    ctx.all_styles_mut(|style| {
        let scaled = scaled_style(style.clone(), scale);
        *style = scaled;
    });

    problems
}

/// Tamanho mínimo da janela para a escala escolhida.
///
/// A linha mais larga da tela de consulta mede ~332 px a 1.0 com nomes típicos e ~413 px na fonte de
/// sistema mais larga testada (Lucida Console). Escalar a fonte sem escalar o mínimo deixaria o
/// layout apertado; por isso o mínimo acompanha.
pub fn min_window_size(scale: f32) -> egui::Vec2 {
    let scale = if scale.is_finite() {
        scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
    } else {
        DEFAULT_FONT_SCALE
    };
    egui::vec2(640.0 * scale, 480.0 * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A escolha aplicada põe a fonte do usuário **na frente** e mantém as embutidas atrás.
    ///
    /// É o que garante que `⟳`, `▼`, `📂`, `🗂`, `✓` e `✗` continuem aparecendo quando a fonte
    /// escolhida não tem esses glifos.
    /// CONTRACT: IF-219
    #[test]
    fn the_chosen_font_goes_first_and_the_embedded_stay_as_fallback() {
        let chosen = Chosen {
            interface: Some("Fira Code".to_string()),
            editor: Some(EMBEDDED_MONO.to_string()),
            scale: 1.0,
        };

        let (definitions, problems) = definitions(&chosen);
        assert!(problems.is_empty(), "fonte embutida resolve sempre");

        let proportional = definitions
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("família proporcional existe");
        assert_eq!(
            proportional.first().map(String::as_str),
            Some("user:Fira Code"),
            "a escolhida vem primeiro: {proportional:?}"
        );
        assert!(
            proportional.len() > 1,
            "as embutidas continuam atrás: {proportional:?}"
        );

        let mono = definitions
            .families
            .get(&egui::FontFamily::Monospace)
            .expect("família mono existe");
        assert_eq!(
            mono.first().map(String::as_str),
            Some("user:Fira Code (embutida)"),
            "o nome escolhido é o rótulo do combo"
        );
    }

    /// Fonte que não existe no sistema não derruba a janela: cai na embutida e avisa.
    ///
    /// CONTRACT: IF-219
    #[test]
    fn a_missing_font_falls_back_with_a_warning() {
        let chosen = Chosen {
            interface: Some("Fonte Que Nao Existe 123".to_string()),
            editor: None,
            scale: 1.0,
        };

        let (definitions, problems) = definitions(&chosen);

        assert_eq!(
            problems.len(),
            1,
            "um aviso por fonte ausente: {problems:?}"
        );
        assert!(problems[0].contains("Fonte Que Nao Existe 123"));
        let proportional = definitions
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("família existe");
        assert!(
            !proportional.iter().any(|name| name.starts_with("user:")),
            "nada da fonte ausente entra: {proportional:?}"
        );
        assert!(!definitions.font_data.is_empty(), "as embutidas ficam");
    }

    /// A escala é aplicada a **todos** os estilos e fica dentro da faixa.
    ///
    /// CONTRACT: IF-220
    #[test]
    fn the_scale_multiplies_every_text_style() {
        // Os tamanhos de referência saem do próprio egui: se ele mudar, o teste acompanha.
        let base = egui::Style::default().text_styles;
        let style = scaled_style(egui::Style::default(), 1.25);

        for (text_style, font_id) in &base {
            let scaled = style.text_styles[text_style].size;
            assert!(
                (scaled - font_id.size * 1.25).abs() < 0.01,
                "{text_style:?} deveria escalar de {} para {}, ficou {scaled}",
                font_id.size,
                font_id.size * 1.25
            );
        }
    }

    /// Escala absurda é presa na faixa (o arquivo é editável à mão).
    ///
    /// CONTRACT: IF-220
    #[test]
    fn an_absurd_scale_is_clamped() {
        let base = egui::Style::default().text_styles[&egui::TextStyle::Body].size;

        let style = scaled_style(egui::Style::default(), 10.0);
        let body = style.text_styles[&egui::TextStyle::Body].size;
        assert!(
            (body - base * MAX_FONT_SCALE).abs() < 0.01,
            "presa no teto: {body}"
        );

        let style = scaled_style(egui::Style::default(), 0.01);
        let body = style.text_styles[&egui::TextStyle::Body].size;
        assert!(
            (body - base * MIN_FONT_SCALE).abs() < 0.01,
            "presa no piso: {body}"
        );
    }

    /// O mínimo da janela acompanha a escala — o layout nunca fica mais apertado que antes.
    ///
    /// CONTRACT: IF-220
    #[test]
    fn the_minimum_window_grows_with_the_scale() {
        assert_eq!(min_window_size(1.0), egui::vec2(640.0, 480.0));
        assert_eq!(
            min_window_size(1.25),
            egui::vec2(640.0 * 1.25, 480.0 * 1.25)
        );
        // Valor corrompido não vira janela gigante nem invisível.
        assert_eq!(min_window_size(f32::NAN), egui::vec2(640.0, 480.0));
    }

    /// Ida e volta das preferências: o que a tela escolhe é o que vai para o arquivo.
    ///
    /// CONTRACT: IF-219
    #[test]
    fn prefs_round_trip_through_the_chosen_value() {
        let chosen = Chosen {
            interface: Some("  Segoe UI  ".to_string()),
            editor: Some(String::new()),
            scale: 1.1,
        };
        let prefs = chosen.to_prefs();

        assert_eq!(prefs.interface_font.as_deref(), Some("Segoe UI"), "apara");
        assert_eq!(prefs.editor_font, None, "vazio vira ausência");
        assert_eq!(
            Chosen::from_prefs(Some(&prefs)),
            Chosen {
                interface: Some("Segoe UI".to_string()),
                editor: None,
                scale: 1.1,
            }
        );
    }
}
