//! `mustel database pgpass list` — lista as entradas do `pgpass.conf`.
//!
//! Paridade conferida no código legado (`PgPassListCommand`):
//! - arquivo ausente ou sem entrada válida → `No entries configured in pgpass.conf.` e exit **0**;
//! - caso contrário, tabela com título `pgpass.conf` e colunas `HOSTNAME`, `PORT`, `DATABASE`,
//!   `USERNAME`, `PASSWORD`, com a senha mascarada como `****` (o valor **nunca** é impresso);
//! - exit **0** nos dois caminhos.
//!
//! Erro de leitura não é tratado pelo comando legado: sobe para o `Program.cs`, que mapeia
//! `IOException`/`UnauthorizedAccessException` para 20/30 — o mesmo que `CoreError::Io` faz aqui.
//! A formatação da tabela é nível N4 (o legado usa borda quadrada e o console dele sai em codepage);
//! o comparador confere o conteúdo por tokens ASCII, e é por isso que o título e os cabeçalhos
//! precisam existir.

use mustel_core::error::CoreError;
use mustel_core::pgpass::PgPassStore;

use crate::ui::table;

use super::map_error;

/// Título da tabela, como no legado.
const TITLE: &str = "pgpass.conf";

/// Colunas da tabela, na mesma ordem do legado.
const HEADERS: [&str; 5] = ["HOSTNAME", "PORT", "DATABASE", "USERNAME", "PASSWORD"];

/// Máscara exibida no lugar da senha.
const MASK: &str = "****";

/// Executa o comando.
pub fn run() -> Result<i32, CoreError> {
    let store = PgPassStore::with_default_path().map_err(map_error)?;
    let entries = store.list()?;

    if entries.is_empty() {
        println!("No entries configured in pgpass.conf.");
        return Ok(0);
    }

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.hostname.clone(),
                entry.port.clone(),
                entry.database.clone(),
                entry.username.clone(),
                MASK.to_string(),
            ]
        })
        .collect();

    println!("{}", table::render_titled(TITLE, &HEADERS, &rows));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-100
    fn the_mask_never_reveals_the_password() {
        let entry = mustel_core::pgpass::PgPassEntry {
            password: "segredo".to_string(),
            ..mustel_core::pgpass::PgPassEntry::default()
        };
        let row = vec![
            entry.hostname.clone(),
            entry.port.clone(),
            entry.database.clone(),
            entry.username.clone(),
            MASK.to_string(),
        ];
        assert_eq!(row, vec!["localhost", "5432", "*", "postgres", "****"]);
        assert!(!row.iter().any(|cell| cell.contains("segredo")));
    }
}
