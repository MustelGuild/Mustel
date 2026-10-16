//! Indicador de progresso (adaptador sobre `indicatif`).
//!
//! Fora de terminal o indicador é **oculto** em vez de escrever lixo no stdout redirecionado —
//! o legado misturava progresso e resultado no mesmo stream, o que atrapalha pipe/CSV.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use super::is_interactive;

/// Indicador de progresso do CLI.
#[derive(Debug)]
pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    /// Cria um spinner com mensagem (oculto fora de terminal).
    pub fn spinner(message: &str) -> Self {
        let bar = if is_interactive() {
            let bar = ProgressBar::new_spinner();
            bar.set_style(
                ProgressStyle::with_template("{spinner} {msg}").expect("static template is valid"),
            );
            bar.enable_steady_tick(Duration::from_millis(120));
            bar
        } else {
            ProgressBar::hidden()
        };

        bar.set_message(message.to_string());
        Self { bar }
    }

    /// Cria uma barra com total conhecido (oculta fora de terminal).
    pub fn bar(total: u64) -> Self {
        let bar = if is_interactive() {
            ProgressBar::new(total)
        } else {
            ProgressBar::hidden()
        };
        Self { bar }
    }

    /// Atualiza a mensagem exibida.
    pub fn set_message(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    /// Avança o progresso.
    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    /// Encerra e limpa o indicador.
    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_silent_outside_a_terminal() {
        let progress = Progress::spinner("trabalhando");
        progress.set_message("ainda");
        progress.inc(1);
        progress.finish();
        assert!(
            progress.bar.is_hidden(),
            "fora de terminal deve ficar oculto"
        );
    }
}
