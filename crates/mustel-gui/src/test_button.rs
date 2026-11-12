//! O botão `Test` de uma linha: ocioso, verificando (spinner) e resultado (check/X por 2 s).
//!
//! O mesmo desenho serve ao cartão de servidor (Connections) e ao de perfil de IA (Settings): o
//! retorno do teste é o **próprio botão**, sem texto extra na tela. A falha vira toast (IF-263); o
//! sucesso, só o check.

use std::time::Duration;

use crate::icons::{self, Icon};
use crate::theme;

/// Quanto tempo o check/X fica no lugar do botão antes de voltar a `Test`.
pub(crate) const FEEDBACK: Duration = Duration::from_secs(2);

/// O que o botão mostra agora.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Phase {
    /// Ocioso: o rótulo (`Test`).
    Idle,
    /// Verificando: spinner, sem clique.
    Running,
    /// Resultado: check (deu certo) ou X (falhou). Continua clicável para repetir o teste.
    Done { ok: bool },
}

/// Largura mínima do botão, a mesma nos três estados — assim a linha de ações não dança.
///
/// É **largura mínima** (`Button::min_size`), não tamanho fixo: a altura continua sendo a natural
/// do botão. Forçar a altura com `add_sized` deixava o `Test` mais alto que `Edit`/`Remove`.
pub(crate) fn width(ui: &egui::Ui, label: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, ui.visuals().text_color());
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// Desenha o botão no estado pedido.
///
/// `height` é a altura dos botões vizinhos (a natural do `Button` não é `interact_size.y`); o
/// spinner usa a mesma, para a linha não mudar de tamanho ao começar o teste.
pub(crate) fn show(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    height: f32,
    phase: Phase,
) -> egui::Response {
    match phase {
        Phase::Idle => ui.add(egui::Button::new(label).min_size(egui::vec2(width, 0.0))),
        Phase::Running => {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let visuals = ui.style().visuals.widgets.inactive;
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
            // O spinner fica centrado na moldura do botão ocioso: nada muda de lugar.
            let inner = egui::Rect::from_center_size(
                rect.center(),
                egui::Vec2::splat((rect.height() * 0.55).min(rect.width() * 0.8)),
            );
            ui.put(inner, egui::Spinner::new().size(inner.height()));
            response
        }
        Phase::Done { ok } => {
            let icon = if ok { Icon::Check } else { Icon::Cross };
            let color = if ok {
                ui.visuals().text_color()
            } else {
                theme::danger(ui)
            };
            let glyph = egui::RichText::new(icon.glyph().to_string())
                .font(icons::font_id(
                    ui.text_style_height(&egui::TextStyle::Button),
                ))
                .color(color);
            ui.add(egui::Button::new(glyph).min_size(egui::vec2(width, 0.0)))
        }
    }
}
