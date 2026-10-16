//! Tradução de erro → mensagem + exit code (contrato `Program.cs` do FurLab).
//!
//! Detalhe descoberto na captura do oráculo (T9): o FurLab escreve tudo — inclusive erros — em
//! **stdout** (`stderr` ficou vazio em 7/7 casos de erro). Manter o stream é parte da paridade;
//! "melhorar" para stderr quebraria o contrato sem ganho para quem usa o CLI hoje.

use mustel_core::error::CoreError;

/// Formata a mensagem de erro sem imprimir (testável; a impressão fica em [`report_error`]).
pub fn format_error(error: &CoreError) -> String {
    format!("Error: {error}")
}

/// Imprime o erro no mesmo formato e **no mesmo stream** do CLI legado e devolve o exit code.
pub fn report_error(error: &CoreError) -> i32 {
    println!("{}", format_error(error));
    error.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::error::exit_code;

    #[test]
    fn returns_the_mapped_code() {
        assert_eq!(report_error(&CoreError::Cancelled), exit_code::CANCELLED);
    }

    #[test]
    fn formats_with_the_legacy_prefix() {
        let error = CoreError::InvalidState("boom".to_string());
        assert_eq!(format_error(&error), "Error: boom");
    }
}
