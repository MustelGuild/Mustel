//! Ícones da interface, de uma fonte embutida no binário (Heroicons, via `iconflow`).
//!
//! Por que assim: emoji vem de uma fonte do sistema (Segoe UI Emoji no Windows, Noto Color Emoji
//! no Linux, Apple Color Emoji no macOS) — mesmo glifo, desenhos e métricas diferentes, e a cor
//! presa na fonte. Aqui o desenho vem de um **TTF que viaja no binário**: é o mesmo ícone em todo
//! sistema operacional, e a cor é a do tema.
//!
//! Os ícones são **Heroicons** (MIT), variante *Outline* (24×24, traço 1,5) — escolhida por combinar
//! com o traço fino do resto da janela.

use iconflow::{Pack, Size, Style, try_icon};

/// Família de fonte registrada para os ícones (o `iconflow` a nomeia assim).
const FAMILY: &str = "Heroicons Outline";

/// Um ícone nomeado da interface. O nome é o do Heroicons (`kebab-case`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Recarregar / descobrir.
    Refresh,
    /// Fechar / dispensar.
    Close,
    /// Aviso.
    Warning,
    /// Excluir (lixeira).
    Delete,
    /// Salvar.
    Save,
    /// Salvar como.
    SaveAs,
    /// Pasta fechada.
    Folder,
    /// Pasta aberta.
    FolderOpen,
    /// Abrir fora do app (gerenciador de arquivos).
    OpenExternal,
    /// Certo.
    Check,
    /// Errado.
    Cross,
    /// Vassoura (limpar).
    Broom,
    /// Seta para baixo.
    ChevronDown,
    /// Seta para a esquerda.
    ChevronLeft,
    /// Seta para a direita.
    ChevronRight,
    /// Seta para cima.
    ArrowUp,
    /// Aba de consulta (lupa).
    Magnifier,
    /// Aba de servidores (pilha de servidores).
    Server,
    /// Aba de configurações (engrenagem).
    Settings,
    /// Tema seguindo o sistema.
    ThemeSystem,
    /// Tema claro.
    ThemeLight,
    /// Tema escuro.
    ThemeDark,
}

impl Icon {
    /// Nome do ícone no Heroicons.
    ///
    /// `Save`/`SaveAs` usam `arrow-down-tray`/`pencil-square`: o Heroicons **não tem disquete**
    /// (ícone de outra época), então "salvar" é a seta para o disco.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Refresh => "arrow-path",
            Icon::Close => "x-mark",
            Icon::Warning => "exclamation-triangle",
            Icon::Delete => "trash",
            Icon::Save => "arrow-down-tray",
            Icon::SaveAs => "pencil-square",
            Icon::Folder => "folder",
            Icon::FolderOpen => "folder-open",
            Icon::OpenExternal => "arrow-top-right-on-square",
            Icon::Check => "check",
            Icon::Cross => "x-mark",
            Icon::Broom => "arrow-path-rounded-square",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronRight => "chevron-right",
            Icon::ArrowUp => "arrow-up",
            Icon::Magnifier => "magnifying-glass",
            Icon::Server => "server-stack",
            Icon::Settings => "cog-6-tooth",
            Icon::ThemeSystem => "computer-desktop",
            Icon::ThemeLight => "sun",
            Icon::ThemeDark => "moon",
        }
    }

    /// O caractere (codepoint) na fonte dos ícones.
    pub fn glyph(self) -> char {
        try_icon(Pack::Heroicons, self.name(), Style::Outline, Size::Regular)
            .ok()
            .and_then(|icon| char::from_u32(icon.codepoint))
            .unwrap_or('?')
    }
}

/// `FontId` no tamanho pedido, na família dos ícones.
pub fn font_id(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(FAMILY.into()))
}

/// Registra a fonte dos ícones nas definições do egui.
///
/// Só a família **nomeada** (`Heroicons Outline`) é criada — os ícones nunca entram no fallback do
/// texto comum, então um emoji do sistema jamais é escolhido no lugar deles.
pub fn install(definitions: &mut egui::FontDefinitions) {
    for asset in iconflow::fonts() {
        if asset.family != FAMILY {
            continue;
        }
        definitions.font_data.insert(
            asset.family.to_string(),
            std::sync::Arc::new(egui::FontData::from_static(asset.bytes)),
        );
        definitions
            .families
            .entry(egui::FontFamily::Name(asset.family.into()))
            .or_default()
            .insert(0, asset.family.to_string());
    }
}

/// Desenha o ícone embutido no fluxo do texto, na cor do texto.
pub fn inline(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    inline_colored(ui, icon, ui.visuals().text_color())
}

/// Desenha o ícone embutido com uma cor explícita (aviso, erro).
pub fn inline_colored(ui: &mut egui::Ui, icon: Icon, color: egui::Color32) -> egui::Response {
    let size = ui.text_style_height(&egui::TextStyle::Body);
    let text = egui::RichText::new(icon.glyph().to_string())
        .font(font_id(size))
        .color(color);
    ui.label(text)
}

/// Ícone clicável, sem moldura (linhas densas).
pub fn button(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    button_enabled(ui, icon, true)
}

/// Ícone clicável, respeitando o estado habilitado.
pub fn button_enabled(ui: &mut egui::Ui, icon: Icon, enabled: bool) -> egui::Response {
    let size = ui.text_style_height(&egui::TextStyle::Body);
    let color = if enabled {
        ui.visuals().weak_text_color()
    } else {
        ui.visuals().weak_text_color().gamma_multiply(0.5)
    };
    let text = egui::RichText::new(icon.glyph().to_string())
        .font(font_id(size))
        .color(color);
    let button = egui::Button::new(text).frame(false);
    if enabled {
        ui.add(button)
    } else {
        ui.add_enabled(false, button)
    }
}

/// Botão com ícone + rótulo.
pub fn button_text(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    button_text_enabled(ui, icon, text, true)
}

/// Botão com ícone + rótulo, respeitando o estado habilitado.
pub fn button_text_enabled(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    enabled: bool,
) -> egui::Response {
    let size = ui.text_style_height(&egui::TextStyle::Button);
    // `egui::Button` aceita um `WidgetText`; juntar glifo e rótulo com fontes diferentes pede um
    // `LayoutJob`. Construído aqui para o ícone não herdar a fonte do texto.
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &icon.glyph().to_string(),
        0.0,
        egui::TextFormat {
            font_id: font_id(size),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    job.append(
        &format!(" {text}"),
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Button.resolve(ui.style()),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    ui.add_enabled(enabled, egui::Button::new(job))
}

/// Rótulo selecionável com ícone + texto (substitui `selectable_label` com emoji).
pub fn selectable(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    selectable_selected(ui, icon, text, false)
}

/// Rótulo selecionável com ícone + texto, marcado ou não.
pub fn selectable_selected(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    selected: bool,
) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &icon.glyph().to_string(),
        0.0,
        egui::TextFormat {
            font_id: font_id(ui.text_style_height(&egui::TextStyle::Body)),
            ..Default::default()
        },
    );
    job.append(
        &format!(" {text}"),
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(ui.style()),
            ..Default::default()
        },
    );
    ui.selectable_label(selected, job)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo ícone resolve na fonte embutida: nenhum cai no `?` do fallback.
    #[test]
    fn every_icon_resolves_in_the_embedded_font() {
        for icon in ALL {
            assert_ne!(icon.glyph(), '?', "{icon:?} não existe no Heroicons");
            assert!(!icon.name().is_empty(), "{icon:?} sem nome");
        }
    }

    /// A fonte é registrada como família **nomeada**: ícone não entra no fallback do texto.
    #[test]
    fn the_icon_font_is_a_named_family() {
        let mut definitions = egui::FontDefinitions::default();
        install(&mut definitions);

        let family = egui::FontFamily::Name(FAMILY.into());
        assert!(
            definitions.families.contains_key(&family),
            "família {FAMILY} registrada"
        );
        assert!(
            !definitions
                .families
                .get(&egui::FontFamily::Proportional)
                .is_some_and(|fonts| fonts.iter().any(|f| f == FAMILY)),
            "os ícones não podem entrar no fallback do texto"
        );
    }

    const ALL: [Icon; 22] = [
        Icon::Refresh,
        Icon::Close,
        Icon::Warning,
        Icon::Delete,
        Icon::Save,
        Icon::SaveAs,
        Icon::Folder,
        Icon::FolderOpen,
        Icon::Check,
        Icon::Cross,
        Icon::Broom,
        Icon::ChevronDown,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ArrowUp,
        Icon::Magnifier,
        Icon::Server,
        Icon::Settings,
        Icon::OpenExternal,
        Icon::ThemeSystem,
        Icon::ThemeLight,
        Icon::ThemeDark,
    ];

    /// **Guarda de iconografia**: nenhuma string de UI pode conter um glifo/emoji de sistema.
    ///
    /// O requisito é "o mesmo ícone em todo SO", e isso só vale se o desenho vier da fonte
    /// embutida. Emoji (✕, 🗑, 💾, ⚙, ⚠…) vem do sistema e muda de desenho e métrica por SO — este
    /// teste falha se algum voltar a aparecer no código de UI.
    /// CONTRACT: IF-245
    #[test]
    fn no_ui_string_uses_a_system_glyph() {
        const FORBIDDEN: [char; 22] = [
            '⟳', '✕', '⚠', '💾', '📝', '🗑', '📂', '🗂', '📁', '💻', '☀', '🌙', '✓', '✗', '∅', '▼',
            '▾', '◀', '▶', '⬆', '🧹', '⚙',
        ];

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        collect_rust_files(&root, &mut |path| {
            let Ok(text) = std::fs::read_to_string(path) else {
                return;
            };
            // O próprio `icons.rs` declara os codepoints de referência nos testes.
            if path.ends_with("icons.rs") {
                return;
            }
            for (line_number, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                // Comentário/doc pode citar o glifo antigo para explicar a decisão.
                if trimmed.starts_with("//") {
                    continue;
                }
                if let Some(glyph) = FORBIDDEN.iter().find(|glyph| line.contains(**glyph)) {
                    offenders.push(format!("{}:{} ({glyph})", path.display(), line_number + 1));
                }
            }
        });

        assert!(
            offenders.is_empty(),
            "glifo de sistema voltou na UI (use `crate::icons::Icon`): {offenders:#?}"
        );
    }

    /// Percorre `.rs` recursivamente.
    fn collect_rust_files(dir: &std::path::Path, visit: &mut impl FnMut(&std::path::Path)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, visit);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                visit(&path);
            }
        }
    }
}
