//! Rolagem suave até um destino.
//!
//! O `ScrollArea` do egui aceita um offset por quadro; animar é calcular esse offset a cada quadro até
//! chegar ao fim. A conta fica aqui, isolada e testável sem janela:
//!
//! - **sem overshoot**: o offset nunca passa do destino;
//! - **independente de framerate**: a fração avançada usa `dt`, então 30 e 144 Hz chegam ao mesmo
//!   lugar no mesmo tempo aproximado;
//! - **termina**: quando falta menos de [`SNAP_PX`], encaixa no destino e desliga — sem isso o laço
//!   ficaria pedindo repaint para sempre.

/// Distância a partir da qual o destino é considerado alcançado.
const SNAP_PX: f32 = 0.5;

/// Quão depressa a rolagem converge (maior = mais rápida).
const SPEED: f32 = 14.0;

/// Animação de rolagem entre dois offsets.
#[derive(Debug, Default)]
pub struct SmoothScroll {
    current: f32,
    target: f32,
    active: bool,
}

impl SmoothScroll {
    /// Começa a animar de `from` até `to`.
    pub fn start(&mut self, from: f32, to: f32) {
        self.current = from;
        self.target = to;
        self.active = (to - from).abs() > SNAP_PX;
    }

    /// Avança um quadro e devolve o offset a aplicar, ou `None` quando não há animação.
    pub fn step(&mut self, dt: f32) -> Option<f32> {
        if !self.active {
            return None;
        }

        let remaining = self.target - self.current;
        if remaining.abs() <= SNAP_PX {
            self.current = self.target;
            self.active = false;
            return Some(self.current);
        }

        // Exponencial com `dt`: a mesma fração por segundo em qualquer framerate.
        // `dt` inválido (NaN) viraria offset NaN — `f32::clamp` **propaga** NaN, então a checagem
        // vem antes, não depois.
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let factor = 1.0 - (-SPEED * dt).exp();
        self.current += remaining * factor;

        // Encosta no destino sem passar dele.
        if (self.target - self.current).abs() <= SNAP_PX {
            self.current = self.target;
            self.active = false;
        }

        Some(self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_animation_is_inactive() {
        let mut scroll = SmoothScroll::default();
        assert!(
            scroll.step(1.0 / 60.0).is_none(),
            "sem animação não devolve offset"
        );
    }

    /// IF-201: o botão "ir ao fim" anima até o destino e para — sem overshoot e independente de
    /// framerate. É a decisão de rolagem suave; o desenho do botão fica na tela.
    /// CONTRACT: IF-201
    #[test]
    fn it_reaches_the_target_and_stops() {
        let mut scroll = SmoothScroll::default();
        scroll.start(0.0, 500.0);
        assert!(scroll.step(0.0).is_some(), "a animação começa");

        let mut last = 0.0;
        for _ in 0..240 {
            if let Some(offset) = scroll.step(1.0 / 60.0) {
                assert!(offset >= last, "nunca volta para trás: {offset} < {last}");
                assert!(
                    offset <= 500.0 + f32::EPSILON,
                    "nunca passa do destino: {offset}"
                );
                last = offset;
            }
        }

        assert!(scroll.step(0.0).is_none(), "a animação termina");
        assert_eq!(last, 500.0, "encaixa exatamente no destino");
    }

    /// Mesmo tempo total em framerates diferentes chega perto do mesmo ponto.
    #[test]
    fn it_is_frame_rate_independent() {
        let mut slow = SmoothScroll::default();
        slow.start(0.0, 400.0);
        for _ in 0..30 {
            slow.step(1.0 / 30.0);
        }
        let at_30hz = slow.step(0.0).unwrap_or(400.0);

        let mut fast = SmoothScroll::default();
        fast.start(0.0, 400.0);
        for _ in 0..144 {
            fast.step(1.0 / 144.0);
        }
        let at_144hz = fast.step(0.0).unwrap_or(400.0);

        assert!(
            (at_30hz - at_144hz).abs() < 60.0,
            "30 Hz chegou em {at_30hz}, 144 Hz em {at_144hz}"
        );
    }

    /// Um passo com `dt` inválido não vira `NaN` nem trava a animação.
    #[test]
    fn a_bad_dt_does_not_poison_the_offset() {
        let mut scroll = SmoothScroll::default();
        scroll.start(0.0, 100.0);
        if let Some(offset) = scroll.step(f32::NAN) {
            assert!(offset.is_finite(), "offset virou {offset}");
        }
    }

    /// Pedir para ir aonde já se está não liga a animação.
    #[test]
    fn starting_at_the_target_does_nothing() {
        let mut scroll = SmoothScroll::default();
        scroll.start(300.0, 300.0);
        assert!(scroll.step(0.0).is_none(), "nada a animar");
    }
}
