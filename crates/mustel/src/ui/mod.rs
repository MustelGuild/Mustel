//! Primitivas da UI de terminal: prompts, tabelas, progresso e estilo.
//!
//! Fica no frontend (nunca no `mustel-core`, D9) e existe para dar **paridade de comportamento** com
//! os prompts do Spectre que o FurLab usa: seleção, multi-seleção com itens pré-selecionados,
//! confirmação, texto livre, senha com máscara, tabela e indicador de progresso.
//!
//! Regra de ouro: nada aqui pode **travar** quando não há terminal. Toda função que depende de
//! interação chama [`ensure_interactive`] antes e falha com mensagem clara — o legado, nesse cenário,
//! quebrava com erro de console (ver `docs/explanation/diferencas-deliberadas.md`).

pub mod password_input;
pub mod progress;
pub mod prompts;
pub mod style;
pub mod table;

use std::io::IsTerminal;

use mustel_core::error::CoreError;

/// Há terminal interativo nas duas pontas (entrada e saída).
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Garante modo interativo, ou devolve erro claro dizendo o que fazer.
pub fn ensure_interactive(action: &str) -> Result<(), CoreError> {
    ensure_interactive_with(is_interactive(), action)
}

/// A regra de [`ensure_interactive`] com a resposta já decidida.
///
/// A decisão é injetada para o teste não depender do TTY de quem roda a suíte: num terminal
/// interativo o processo de teste **tem** terminal, e os dois lados precisam ser cobertos.
fn ensure_interactive_with(interactive: bool, action: &str) -> Result<(), CoreError> {
    if interactive {
        Ok(())
    } else {
        Err(CoreError::InvalidState(format!(
            "{action} needs an interactive terminal; pass the value as a command-line argument instead"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_interactive_explains_what_to_do() {
        let error = ensure_interactive_with(false, "a password prompt").expect_err("must fail");
        let message = error.to_string();
        assert!(message.contains("interactive terminal"), "{message}");
        assert!(message.contains("command-line argument"), "{message}");
    }

    /// Com terminal, a mesma ação é aceita — a decisão vem do parâmetro, não do TTY da suíte.
    #[test]
    fn an_interactive_terminal_accepts_the_action() {
        ensure_interactive_with(true, "a password prompt").expect("interactive accepts");
    }
}
