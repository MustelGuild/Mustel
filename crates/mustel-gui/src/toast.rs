//! Toasts: recados efêmeros de ação, no rodapé da janela.
//!
//! A mecânica vem do `egui-notify` e do `egui-toast` (ambos MIT), mas sem dependência nova: o toast
//! é desenhado com os widgets, a fonte de ícones e as cores de estado do próprio projeto.
//!
//! O ciclo é o mesmo das duas: **entra** (fade + deslize de baixo), fica **ocioso** contando o tempo
//! e **sai** — por tempo, por clique no corpo ou no `X`. O tempo **pausa no hover**, para dar tempo
//! de ler, e a barra fina embaixo mostra quanto falta (4 s; **10 s** no erro). A barra pede os
//! próprios quadros, então anda mesmo com o mouse parado.
//!
//! Escolhas que diferem das bibliotecas, para caber aqui:
//! - ícone e cor saem de [`crate::icons`]/[`crate::theme`] (Heroicons e o par claro/escuro), não de
//!   emoji;
//! - a mensagem **repete** reinicia o relógio em vez de empilhar outra igual (o mesmo "salvo" duas
//!   vezes não vira dois balões);
//! - o estado mora em [`Toasts`], no `App`, e não na memória do `egui` — é testável sem janela.

use std::time::Duration;

use crate::icons::{self, Icon};
use crate::theme;

/// Largura reservada para o `X`: o texto quebra linha sabendo que ele existe.
const CLOSE_WIDTH: f32 = 26.0;

/// Tempo que o toast fica na tela, sem contar o hover.
const DURATION: f32 = 4.0;
/// Erro merece mais tempo de leitura que um "salvo".
const ERROR_DURATION: f32 = 10.0;
/// Intervalo entre quadros enquanto há balão na tela: é o que faz a barra de tempo andar sem
/// depender do mouse (a janela dorme quando ninguém pede repintura).
const REFRESH: f32 = 1.0 / 30.0;
/// Duração do fade/deslize de entrada e de saída.
const FADE: f32 = 0.18;
/// Distância da borda inferior da janela.
const MARGIN: f32 = 16.0;
/// Espaço entre toasts empilhados.
const SPACING: f32 = 8.0;
/// Largura máxima: mensagem longa quebra linha em vez de esticar o balão.
const MAX_WIDTH: f32 = 460.0;
/// Quantos ficam na tela; passando disso, o mais antigo começa a sair.
const MAX_VISIBLE: usize = 4;
/// Deslocamento vertical durante a entrada/saída (px).
const SLIDE: f32 = 10.0;

/// O tom do recado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Ação concluída.
    Success,
    /// Algo impede ou degrada a ação.
    Warning,
    /// A ação falhou.
    Error,
}

impl Kind {
    /// Quanto tempo este tom fica na tela.
    fn duration(self) -> f32 {
        match self {
            Kind::Error => ERROR_DURATION,
            Kind::Success | Kind::Warning => DURATION,
        }
    }

    /// Ícone da fonte embutida.
    fn icon(self) -> Icon {
        match self {
            Kind::Success => Icon::Check,
            Kind::Warning => Icon::Warning,
            Kind::Error => Icon::Cross,
        }
    }

    /// Cor do ícone e da barra de tempo, no tema ativo.
    fn color(self, ui: &egui::Ui) -> egui::Color32 {
        match self {
            Kind::Success => theme::success(ui),
            Kind::Warning => theme::warning(ui),
            Kind::Error => theme::danger(ui),
        }
    }
}

/// Um recado na tela.
#[derive(Debug)]
struct Toast {
    /// Identificador estável (animação e `Id` da área).
    id: u64,
    message: String,
    kind: Kind,
    /// Segundos que ainda faltam.
    remaining: f32,
    /// Segundos totais, para a barra.
    total: f32,
    /// Instante do quadro anterior (relógio do `egui`); `None` no primeiro quadro.
    last_time: Option<f64>,
    /// Em saída: a animação leva a opacidade a zero e o toast sai da lista.
    closing: bool,
}

impl Toast {
    /// Avança o relógio. `hovered` **pausa** a contagem (dá tempo de ler).
    fn tick(&mut self, dt: f32, hovered: bool) {
        if self.closing || hovered {
            return;
        }
        self.remaining -= dt;
        if self.remaining <= 0.0 {
            self.remaining = 0.0;
            self.closing = true;
        }
    }

    /// Desenha o balão — moldura, ícone, mensagem que quebra linha, `X` e barra de tempo — e
    /// devolve a resposta do **corpo**, usada para hover e clique.
    fn paint(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let frame = egui::Frame::window(ui.style())
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let size = ui.text_style_height(&egui::TextStyle::Body);
                    ui.label(
                        egui::RichText::new(self.kind.icon().glyph().to_string())
                            .font(icons::font_id(size))
                            .color(self.kind.color(ui)),
                    );
                    // `wrap()` explícito: em layout horizontal o padrão do `Label` é **estender**,
                    // e a mensagem longa esticaria o balão sem limite. O espaço do `X` entra na
                    // conta **antes** de medir o texto; senão o botão empurra a moldura além de
                    // `MAX_WIDTH` (o clique também fica reservado, sem o texto por baixo).
                    let wrap =
                        (ui.available_width() - CLOSE_WIDTH - ui.spacing().item_spacing.x).max(1.0);
                    ui.scope(|ui| {
                        ui.set_max_width(wrap);
                        ui.add(egui::Label::new(&self.message).wrap());
                    });
                    if icons::button_sized(ui, Icon::Close, CLOSE_WIDTH)
                        .on_hover_text("Dismiss")
                        .clicked()
                    {
                        self.closing = true;
                    }
                });
            });

        // Barra de tempo: encolhe da direita para a esquerda e some na saída.
        let fraction = if self.total > 0.0 {
            (self.remaining / self.total).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if !self.closing && fraction > 0.0 {
            let rect = frame.response.rect;
            let mut clip = rect;
            clip.set_top(rect.bottom() - 2.0);
            clip.set_right(rect.left() + rect.width() * fraction);
            let color = self.kind.color(ui).gamma_multiply(0.65);
            ui.painter()
                .with_clip_rect(clip)
                .rect_filled(rect, 0, color);
        }

        frame.response
    }
}

/// A fila de toasts da janela.
#[derive(Debug, Default)]
pub(crate) struct Toasts {
    items: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    /// Ação concluída.
    pub(crate) fn success(&mut self, message: impl Into<String>) {
        self.notify(Kind::Success, message);
    }

    /// Põe um recado na tela, com o tom de quem chama.
    pub(crate) fn notify(&mut self, kind: Kind, message: impl Into<String>) {
        let message = message.into();

        // A mesma mensagem não empilha: reinicia o relógio da que já está na tela.
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|toast| !toast.closing && toast.message == message)
        {
            existing.kind = kind;
            existing.remaining = kind.duration();
            existing.total = kind.duration();
            return;
        }

        self.next_id += 1;
        let duration = kind.duration();
        // O mais **novo** entra na frente da lista e é desenhado no rodapé; os antigos sobem.
        self.items.insert(
            0,
            Toast {
                id: self.next_id,
                message,
                kind,
                remaining: duration,
                total: duration,
                last_time: None,
                closing: false,
            },
        );

        // Teto de balões: o mais antigo começa a sair (com fade), não some de repente.
        if self.items.len() > MAX_VISIBLE
            && let Some(oldest) = self.items.last_mut()
        {
            oldest.closing = true;
        }
    }

    /// `true` quando não há nada na tela (nem saindo).
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Desenha e avança os toasts. Chamado **uma vez por quadro**, depois de tudo.
    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        if self.is_empty() {
            return;
        }

        let now = ctx.input(|input| input.time);
        let mut items = std::mem::take(&mut self.items);
        let mut keep = Vec::with_capacity(items.len());
        // Distância do rodapé até o **fundo** do próximo balão.
        let mut bottom = -MARGIN;

        for mut toast in items.drain(..) {
            let id = egui::Id::unique(("mustel-toast", toast.id));
            let eased = ctx.animate_bool_with_time(id.with("anim"), !toast.closing, FADE);
            let offset = egui::vec2(0.0, bottom + (1.0 - eased) * SLIDE);

            let area = egui::Area::new(id)
                .anchor(egui::Align2::CENTER_BOTTOM, offset)
                .order(egui::Order::Foreground)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.set_opacity(eased);
                    ui.set_max_width(MAX_WIDTH);
                    toast.paint(ui)
                });

            let response = area.inner;
            let hovered = response.contains_pointer();
            // Relógio de **parede**: entre duas repinturas a janela pode dormir muito mais que um
            // quadro, e o `stable_dt` (curto por definição) deixaria o balão bem além dos 4 s.
            let dt = toast
                .last_time
                .map_or(0.0, |last| (now - last).max(0.0) as f32);
            toast.last_time = Some(now);
            toast.tick(dt, hovered);

            // Clique no corpo fecha, como nas duas bibliotecas; o `X` é a afordância visível.
            if !toast.closing && response.interact(egui::Sense::click()).clicked() {
                toast.closing = true;
            }

            if !toast.closing && !hovered {
                // Um quadro a cada `REFRESH`: sem isso a janela dorme e a barra de tempo fica
                // parada até o próximo evento (o prazo ainda vence — o relógio é de parede —,
                // mas a barra só pularia no fim). No hover não há o que animar: a pausa é
                // intencional, e o movimento do ponteiro acorda a janela quando ele sai.
                let step = toast.remaining.clamp(0.0, REFRESH);
                ctx.request_repaint_after(Duration::from_secs_f32(step));
            }
            // A saída também precisa de quadros: `closing` acabou de virar **depois** de a
            // animação ser avaliada, e o `animate_bool_with_time` só pede repintura quando ela
            // já está em curso — sem esta linha, o fade ficaria parado em opacidade 1.
            if toast.closing || eased < 1.0 {
                ctx.request_repaint();
            }

            if toast.closing && eased <= 0.0 {
                continue;
            }

            bottom -= response.rect.height() + SPACING;
            keep.push(toast);
        }

        self.items = keep;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast() -> Toast {
        Toast {
            id: 1,
            message: "Salvo".to_string(),
            kind: Kind::Success,
            remaining: DURATION,
            total: DURATION,
            last_time: None,
            closing: false,
        }
    }

    /// Erro fica **10 s** na tela: dá tempo de ler o motivo; um "salvo" continua com 4 s.
    /// CONTRACT: IF-263
    #[test]
    fn an_error_toast_stays_ten_seconds() {
        let mut toasts = Toasts::default();
        toasts.notify(Kind::Error, "Local: Connection failed");

        assert!((toasts.items[0].total - 10.0).abs() < 0.01);
        toasts.items[0].tick(4.0, false);
        assert!(!toasts.items[0].closing, "quatro segundos não bastam");
        toasts.items[0].tick(6.0, false);
        assert!(toasts.items[0].closing, "dez segundos fecham");
    }

    /// O toast visível pede quadros sozinho: é o que faz a barra de tempo andar sem o mouse.
    /// CONTRACT: IF-263
    #[test]
    fn a_visible_toast_asks_for_frames_so_the_bar_moves() {
        let ctx = egui::Context::default();
        let mut toasts = Toasts::default();
        toasts.success("Perfil salvo");

        let _ = frame(&ctx, &mut toasts, 0.0);
        // 0,5 s: o fade de entrada (0,18 s) já terminou, então não é ele quem pede repintura.
        let output = frame(&ctx, &mut toasts, 0.5);

        let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(
            delay <= Duration::from_millis(50),
            "a barra pediu repintura com atraso de {delay:?}"
        );
    }

    /// O relógio corre e o toast se fecha sozinho ao esgotar.
    /// CONTRACT: IF-263
    #[test]
    fn a_toast_closes_itself_after_its_duration() {
        let mut toast = toast();

        toast.tick(1.0, false);
        assert!((toast.remaining - 3.0).abs() < 0.01, "andou um segundo");
        assert!(!toast.closing, "ainda tem tempo");

        toast.tick(3.0, false);
        assert_eq!(toast.remaining, 0.0);
        assert!(toast.closing, "o tempo esgotou");
    }

    /// Com o ponteiro em cima, o tempo **não** anda.
    /// CONTRACT: IF-263
    #[test]
    fn hovering_pauses_the_countdown() {
        let mut toast = toast();

        toast.tick(10.0, true);

        assert!(
            (toast.remaining - DURATION).abs() < 0.01,
            "pausado no hover"
        );
        assert!(!toast.closing);
    }

    /// A mesma mensagem repetida reinicia o relógio em vez de empilhar outro balão.
    /// CONTRACT: IF-263
    #[test]
    fn repeating_a_message_restarts_instead_of_stacking() {
        let mut toasts = Toasts::default();
        toasts.success("Perfil salvo");
        toasts.items[0].remaining = 1.0;

        toasts.success("Perfil salvo");

        assert_eq!(toasts.items.len(), 1, "não virou dois");
        assert!(
            (toasts.items[0].remaining - DURATION).abs() < 0.01,
            "o relógio voltou ao começo"
        );
    }

    /// Passando do teto, o mais antigo começa a sair — não some de repente.
    /// CONTRACT: IF-263
    #[test]
    fn the_oldest_starts_leaving_when_the_screen_is_full() {
        let mut toasts = Toasts::default();
        for index in 0..=MAX_VISIBLE {
            toasts.notify(Kind::Warning, format!("recado {index}"));
        }

        assert_eq!(toasts.items.len(), MAX_VISIBLE + 1);
        assert!(
            toasts.items[MAX_VISIBLE].closing,
            "o mais antigo está saindo"
        );
    }

    /// Desenha um quadro no instante `time` e devolve as formas pintadas (sem as texturas, que
    /// fazer soltar um `FullOutput` com deltas pendentes é panic do `epaint`).
    fn frame(ctx: &egui::Context, toasts: &mut Toasts, time: f64) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            time: Some(time),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| toasts.show(ui.ctx()));
        output.textures_delta.clear();
        output
    }

    /// O quadro em que o prazo vence pede repintura **imediata** — sem isso o fade de saída não
    /// andaria (o `animate_bool_with_time` só agenda quadros quando a animação já começou, e a
    /// saída só começa neste quadro).
    /// CONTRACT: IF-263
    #[test]
    fn the_frame_that_starts_the_exit_asks_for_another_one() {
        let ctx = egui::Context::default();
        let mut toasts = Toasts::default();
        toasts.success("Perfil salvo");

        let _ = frame(&ctx, &mut toasts, 0.0);
        let output = frame(&ctx, &mut toasts, 4.5);

        assert!(
            toasts.items.iter().any(|toast| toast.closing),
            "o prazo venceu"
        );
        let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(
            delay.is_zero(),
            "a saída pediu repintura com atraso de {delay:?}"
        );
    }

    /// O toast aparece na tela com a mensagem, depois do fade de entrada.
    ///
    /// No primeiro quadro a opacidade é **zero** (o `Painter` descarta o texto), então o teste
    /// avança o relógio para além de `FADE`, como a janela faria em dois quadros.
    /// CONTRACT: IF-263
    #[test]
    fn a_toast_paints_its_message() {
        let ctx = egui::Context::default();
        let mut toasts = Toasts::default();
        toasts.success("Perfil salvo");

        let _ = frame(&ctx, &mut toasts, 0.0);
        let output = frame(&ctx, &mut toasts, 0.5);

        let mut texts = Vec::new();
        collect_text(&output.shapes, &mut texts);
        assert!(
            texts.iter().any(|text| text == "Perfil salvo"),
            "a mensagem foi pintada: {texts:?}"
        );
    }

    /// A janela pode dormir entre repinturas: o relógio do toast é o **tempo real**, não o `dt`
    /// do quadro (que o egui mantém curto), senão o balão passaria muito dos 4 s na tela.
    /// CONTRACT: IF-263
    #[test]
    fn a_long_idle_gap_still_empties_the_countdown() {
        let ctx = egui::Context::default();
        let mut toasts = Toasts::default();
        toasts.success("Perfil salvo");

        let _ = frame(&ctx, &mut toasts, 0.0);
        let _ = frame(&ctx, &mut toasts, 10.0);

        assert!(
            toasts.items.iter().all(|toast| toast.closing),
            "dez segundos de sono bastam para o prazo vencer"
        );
    }

    /// Mensagem longa **quebra linha** em vez de esticar o balão além de `MAX_WIDTH`.
    /// CONTRACT: IF-263
    #[test]
    fn a_long_message_wraps_instead_of_stretching() {
        let ctx = egui::Context::default();
        let mut toasts = Toasts::default();
        toasts.notify(
            Kind::Error,
            "Could not save the AI session because the directory is read-only \
             and the file could not be replaced",
        );

        let _ = frame(&ctx, &mut toasts, 0.0);
        let output = frame(&ctx, &mut toasts, 0.5);

        let mut rows = Vec::new();
        collect_text_rows(&output.shapes, &mut rows);
        assert!(
            rows.iter().any(|rows| *rows > 1),
            "a mensagem longa quebrou em mais de uma linha: {rows:?}"
        );
    }

    /// O balão não passa de `MAX_WIDTH`, mesmo com mensagem longa.
    /// CONTRACT: IF-263
    #[test]
    fn a_toast_never_grows_beyond_the_max_width() {
        let ctx = egui::Context::default();
        let mut toast = Toast {
            message: "A message long enough to need wrapping and then a few more words \
                      to be sure it does not fit on a single line"
                .to_string(),
            ..toast()
        };
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut width = 0.0_f32;
        let mut output = ctx.run_ui(input, |ui| {
            let area =
                egui::Area::new(egui::Id::unique("mustel-toast-width-test")).show(ui.ctx(), |ui| {
                    ui.set_max_width(MAX_WIDTH);
                    toast.paint(ui)
                });
            width = area.inner.rect.width();
        });
        output.textures_delta.clear();

        assert!(
            width <= MAX_WIDTH + 1.0,
            "o balão esticou para {width}px (teto de {MAX_WIDTH}px)"
        );
    }

    /// Junta o texto de todo `Shape::Text`, descendo nos grupos.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
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

    /// Quantas linhas cada texto pintado ocupa.
    fn collect_text_rows(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<usize>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<usize>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.rows.len()),
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
