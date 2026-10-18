//! Comandos de `mustel database pgpass`.
//!
//! Porte do grupo `legacy/FurLab.CLI/Commands/Database/PgPass`: o `pgpass.conf` guarda credenciais de
//! PostgreSQL em texto claro (`host:porta:banco:usuário:senha`), com `*` como curinga em qualquer
//! campo menos a senha. O arquivo é um mecanismo **separado** do cofre (D6) — nada aqui usa keyring,
//! porque um `pgpass.conf` sem a senha em claro não serviria para o `psql`.

pub mod add;
pub mod list;
pub mod remove;

use mustel_core::error::CoreError;
use mustel_core::pgpass::PgPassError;

/// Traduz o erro do arquivo na mesma saída do legado.
///
/// O legado devolve `1` (erro geral) para as falhas de escrita do `add`/`remove`, com a mensagem
/// própria de cada caso — `CoreError::Other` acrescenta o prefixo `Error: ` e sai com 1. O
/// `PgPassError::NoPath` é caminho novo do Mustel (o `%APPDATA%` do Windows sempre existe) e vem
/// como estado inválido, como o `NoConfigDir` da configuração.
///
/// `EmptyPassword` é inalcançável pelo CLI (o `add` valida antes, com exit 2, como o legado); ele
/// existe para um frontend que chame o núcleo direto não gravar entrada sem senha.
fn map_error(error: PgPassError) -> CoreError {
    match error {
        PgPassError::NoPath => CoreError::InvalidState(error.to_string()),
        other => CoreError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::error::exit_code;

    #[test]
    fn write_failures_become_general_errors_like_the_legacy() {
        assert_eq!(
            map_error(PgPassError::Write).exit_code(),
            exit_code::GENERAL
        );
        assert_eq!(
            map_error(PgPassError::PermissionDenied {
                directory: "C:\\Users\\x\\AppData\\Roaming\\postgresql".to_string(),
            })
            .exit_code(),
            exit_code::GENERAL
        );
        assert_eq!(
            map_error(PgPassError::EmptyPassword).exit_code(),
            exit_code::GENERAL
        );
    }

    #[test]
    fn a_platform_without_application_data_is_an_invalid_state() {
        assert_eq!(
            map_error(PgPassError::NoPath).exit_code(),
            exit_code::INVALID_STATE
        );
    }
}
