//! Estilo de texto (cores quando há terminal, texto puro quando não há).
//!
//! A saída que compõe contrato de paridade (mensagens de erro, CSV) **não** usa cor: o oráculo não
//! emite ANSI quando o stdout é redirecionado (ver T9). Estas funções são para o texto de apoio
//! (tabelas, avisos de progresso).

use console::Style;

/// Texto em negrito (só coloriza em terminal).
pub fn bold(text: &str) -> String {
    Style::new().bold().for_stdout().apply_to(text).to_string()
}

/// Texto esmaecido (só coloriza em terminal).
pub fn dim(text: &str) -> String {
    Style::new().dim().for_stdout().apply_to(text).to_string()
}

/// Texto de sucesso (só coloriza em terminal).
pub fn success(text: &str) -> String {
    Style::new().green().for_stdout().apply_to(text).to_string()
}

/// Texto de aviso (só coloriza em terminal).
pub fn warning(text: &str) -> String {
    Style::new()
        .yellow()
        .for_stdout()
        .apply_to(text)
        .to_string()
}

/// Texto de perigo (só coloriza em terminal).
pub fn danger(text: &str) -> String {
    Style::new()
        .red()
        .bold()
        .for_stdout()
        .apply_to(text)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_outside_a_terminal() {
        for styled in [bold("x"), dim("x"), success("x"), warning("x"), danger("x")] {
            assert_eq!(styled, "x", "sem terminal não deve haver ANSI");
        }
    }
}
