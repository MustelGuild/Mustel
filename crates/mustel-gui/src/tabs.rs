//! Ícones das abas, desenhados (sem fonte).
//!
//! Emoji **não** servem aqui: no Windows eles vêm do *Segoe UI Emoji*, via fallback do sistema, e a
//! métrica dessa fonte muda de máquina para máquina — o glifo fica acima ou abaixo do texto sem que
//! haja `FontTweak` que alcance um glifo de fallback do sistema. Desenhar o ícone resolve na raiz:
//! ele é um retângulo calculado e centrado no texto por construção, não por métrica de fonte.

/// Ícone de uma aba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Aba de consulta (lupa).
    Query,
    /// Aba de servidores (monitor).
    Servers,
    /// Aba de configurações (engrenagem).
    Settings,
}

/// Desenha uma aba (ícone + rótulo) com o fundo de selecionável do egui.
///
/// Devolve a resposta para o chamador trocar a tela quando houver clique.
pub fn tab(ui: &mut egui::Ui, selected: bool, icon: Icon, label: &str) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), text_color);

    // O ícone acompanha a altura do texto: nada de tamanho fixo que desalinhe quando a fonte cresce.
    let icon_size = galley.size().y;
    let spacing = 4.0;
    let padding = egui::vec2(8.0, 3.0);
    let size = egui::vec2(
        icon_size + spacing + galley.size().x + padding.x * 2.0,
        galley.size().y + padding.y * 2.0,
    );

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);

    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.weak_bg_fill);
    }

    // Ícone e texto compartilham o **mesmo** centro vertical: é isto que o emoji não garantia.
    let center_y = rect.center().y;
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + padding.x + icon_size / 2.0, center_y),
        egui::Vec2::splat(icon_size),
    );
    paint(ui.painter(), icon, icon_rect, visuals.fg_stroke);

    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + spacing,
            center_y - galley.size().y / 2.0,
        ),
        galley,
        visuals.fg_stroke.color,
    );

    response
}

/// Desenha o ícone dentro de `rect`, com a cor e a espessura do texto.
fn paint(painter: &egui::Painter, icon: Icon, rect: egui::Rect, stroke: egui::Stroke) {
    let stroke = egui::Stroke::new(stroke.width.max(1.0), stroke.color);
    let center = rect.center();
    // ~0.72 do quadrado deixa margem para o traço não encostar na borda.
    let size = rect.width() * 0.72;
    let r = size / 2.0;

    match icon {
        Icon::Query => {
            // Lupa: aro + cabo na diagonal.
            let radius = r * 0.62;
            let ring_center = egui::pos2(center.x - r * 0.12, center.y - r * 0.12);
            painter.circle_stroke(ring_center, radius, stroke);
            painter.line_segment(
                [
                    egui::pos2(ring_center.x + radius * 0.72, ring_center.y + radius * 0.72),
                    egui::pos2(center.x + r * 0.72, center.y + r * 0.72),
                ],
                stroke,
            );
        }
        Icon::Servers => {
            // Monitor: tela + pé.
            let screen = egui::Rect::from_min_size(
                egui::pos2(center.x - size / 2.0, center.y - size * 0.46),
                egui::vec2(size, size * 0.62),
            );
            painter.rect_stroke(screen, 1.5, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(center.x, screen.bottom()),
                    egui::pos2(center.x, center.y + r * 0.52),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - r * 0.36, center.y + r * 0.52),
                    egui::pos2(center.x + r * 0.36, center.y + r * 0.52),
                ],
                stroke,
            );
        }
        Icon::Settings => {
            // Engrenagem: aro + seis dentes curtos.
            let radius = r * 0.46;
            painter.circle_stroke(center, radius, stroke);
            for tooth in 0..6 {
                let angle = std::f32::consts::TAU * tooth as f32 / 6.0;
                let (sin, cos) = angle.sin_cos();
                painter.line_segment(
                    [
                        egui::pos2(center.x + cos * radius, center.y + sin * radius),
                        egui::pos2(center.x + cos * r * 0.78, center.y + sin * r * 0.78),
                    ],
                    stroke,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O centro do ícone e o centro do texto ficam na **mesma** linha.
    ///
    /// É a correção do bug relatado: com emoji (fallback do sistema) os glifos saíam acima do texto,
    /// e nenhum `FontTweak` alcança glifo de fonte de sistema.
    /// CONTRACT: IF-221
    #[test]
    fn the_icon_and_the_label_share_the_same_center_line() {
        let ctx = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;

        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = tab(ui, true, Icon::Servers, "Servers");
                rect = response.rect;
            });
        });

        fn find(shape: &egui::Shape, out: &mut Option<f32>) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == "Servers" => {
                    *out = Some(text.pos.y + text.galley.size().y / 2.0);
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        find(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut found = None;
        for clipped in &output.shapes {
            find(&clipped.shape, &mut found);
        }
        output.textures_delta.clear();
        let label_center = found.expect("o rótulo foi desenhado");

        let tab_center = rect.center().y;
        assert!(
            (label_center - tab_center).abs() < 0.5,
            "o texto é centrado na aba (texto {label_center}, aba {tab_center})"
        );

        // O ícone é desenhado com `rect.center().y` por construção — este teste guarda a metade que
        // pode variar (o texto, que vem de um galley com métrica de fonte).
    }
}
