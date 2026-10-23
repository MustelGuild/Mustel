//! `mustel database pgpass remove` — remove uma entrada do `pgpass.conf`.
//!
//! Paridade conferida no código legado (`PgPassRemoveCommand`):
//! - banco ausente ou em branco → `Error: the <database> argument is required.` e exit **2**
//!   (o clap já recusa o posicional faltando; o branco chega aqui);
//! - a chave é `host:porta:banco:usuário`, com os mesmos defaults do `add` — e o `remove` **não**
//!   valida host nem porta: `--host "*"` remove literalmente a entrada curinga;
//! - removido → `Entry removed: host:porta:banco:usuário` e exit **0**;
//! - entrada (ou arquivo) inexistente → `Entry not found: ...` e exit **0** — o legado pinta de
//!   amarelo e **não** é erro; o arquivo fica intacto;
//! - falha de escrita → mensagem própria e exit **1** (via `map_error`).

use mustel_core::error::CoreError;
use mustel_core::pgpass::PgPassEntry;
use mustel_core::pgpass::entry::{DEFAULT_HOST, DEFAULT_PORT, DEFAULT_USERNAME};
use mustel_core::pgpass::store::PgPassStore;

use crate::cli::PgpassRemoveArgs;

use super::map_error;

/// Executa o comando.
pub fn run(args: PgpassRemoveArgs) -> Result<i32, CoreError> {
    let store = PgPassStore::with_default_path().map_err(map_error)?;

    if args.database.trim().is_empty() {
        println!("Error: the <database> argument is required.");
        return Ok(2);
    }

    let key = key_for(&args);
    let outcome = store.remove(&key).map_err(map_error)?;

    // `Removed` e `NotFound` saem com 0: não achar o que se queria remover é o estado desejado.
    println!("{}", outcome.message(&key));
    Ok(0)
}

/// Chave de busca: a senha do legado (`"placeholder"`) não participa da identidade.
fn key_for(args: &PgpassRemoveArgs) -> PgPassEntry {
    PgPassEntry {
        hostname: args
            .host
            .clone()
            .unwrap_or_else(|| DEFAULT_HOST.to_string()),
        port: args
            .port
            .clone()
            .unwrap_or_else(|| DEFAULT_PORT.to_string()),
        database: args.database.clone(),
        username: args
            .username
            .clone()
            .unwrap_or_else(|| DEFAULT_USERNAME.to_string()),
        password: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(database: &str) -> PgpassRemoveArgs {
        PgpassRemoveArgs {
            database: database.to_string(),
            host: None,
            port: None,
            username: None,
        }
    }

    #[test]
    /// CONTRACT: IF-101
    fn the_key_uses_the_same_defaults_as_add() {
        let key = key_for(&args("mydb"));
        assert_eq!(key.formatted_key(), "localhost:5432:mydb:postgres");
    }

    #[test]
    /// CONTRACT: IF-101
    fn the_wildcard_is_a_literal_part_of_the_key() {
        let mut command = args("mydb");
        command.host = Some("*".to_string());
        assert_eq!(key_for(&command).formatted_key(), "*:5432:mydb:postgres");
    }
}
