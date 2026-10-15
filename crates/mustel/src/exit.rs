//! Tradução de erro → mensagem + exit code (contrato `Program.cs` do FurLab).
//!
//! Detalhe descoberto na captura do oráculo (T9): o FurLab escreve tudo — inclusive erros — em
//! **stdout** (`stderr` ficou vazio em 7/7 casos de erro). Manter o stream é parte da paridade;
//! "melhorar" para stderr quebraria o contrato sem ganho para quem usa o CLI hoje.

use mustel_core::error::CoreError;

/// Imprime o erro no mesmo formato do CLI legado (`Error: <mensagem>`) e devolve o exit code.
///
/// Sem cores e sem markup: formatação é nível N4 do contrato (ver `oracle/known-divergences.md`).
pub fn report_error(error: &CoreError) -> i32 {
    println!("Error: {error}");
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
}
