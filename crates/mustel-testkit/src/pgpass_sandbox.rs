//! Fixtures do `pgpass.conf` e a regra de semeadura usada pelos testes.
//!
//! O sandbox do arquivo do **legado** (com backup/restore do `%APPDATA%` real) saiu junto com o
//! `legacy/`; do lado do Mustel o arquivo é sempre isolado por `MUSTEL_PGPASS_FILE`.

use std::path::Path;

use crate::capture::PgPassSeed;

pub fn apply_seed(path: &Path, seed: PgPassSeed) -> std::io::Result<()> {
    let content = match seed {
        PgPassSeed::Unchanged => return Ok(()),
        PgPassSeed::Absent => None,
        PgPassSeed::Fixture => Some(fixture()),
        PgPassSeed::NoTrailingNewline => Some(fixture_without_trailing_newline()),
        PgPassSeed::Wildcard => Some(wildcard_fixture()),
    };

    match content {
        Some(text) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, text)
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

/// Entradas de exemplo usadas nas capturas: um comentário e duas entradas, com a quebra do SO.
///
/// O comentário existe para pinar que ele **não** entra na listagem e que **sobrevive** a um
/// `remove` (o arquivo tem de sair byte a byte igual).
pub fn fixture() -> String {
    [
        "# exemplo",
        "localhost:5432:mydb:postgres:senha123",
        "db.interno:5433:app_dev:deploy:outra_senha",
    ]
    .join(native_line_ending())
        + native_line_ending()
}

/// Fixture de uma entrada **sem** quebra de linha no fim.
///
/// É o caso que pina o `File.AppendAllText` do legado: sem quebra final, a entrada nova entra colada
/// na última (ver `mustel-core/src/pgpass/store.rs`).
pub fn fixture_without_trailing_newline() -> String {
    "localhost:5432:mydb:postgres:senha123".to_string()
}

/// Fixture de uma entrada **curinga**, para o `remove --host "*"` da spec `pgpass-cli-setup`.
pub fn wildcard_fixture() -> String {
    format!("*:5432:mydb:postgres:senha_curinga{}", native_line_ending())
}

/// Quebra de linha do SO.
fn native_line_ending() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}
