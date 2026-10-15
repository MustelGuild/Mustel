//! Tradução de erro → mensagem + exit code (contrato `Program.cs` do FurLab).

use mustel_core::error::CoreError;

/// Imprime o erro no mesmo formato do CLI legado (`Error: <mensagem>`) e devolve o exit code.
///
/// Sem cores e sem markup: formatação é nível N4 do contrato (ver `oracle/known-divergences.md`).
pub fn report_error(error: &CoreError) -> i32 {
    eprintln!("Error: {error}");
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
