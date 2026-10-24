//! `mustel settings db-servers ls` — lista os servidores configurados.
//!
//! Paridade observada no oráculo (T9):
//! - colunas `Name`, `Host`, `Port`, `Username`, `Databases`, `Auto-DB`;
//! - célula de bancos: `(auto)` quando descobre todos, senão a lista separada por `, `,
//!   senão `(default)`;
//! - `Auto-DB`: `Yes`/`No`;
//! - configuração vazia **não** é erro (exit 0) e imprime uma mensagem orientando o `add`;
//! - o exit code é 0 nos dois caminhos.

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::error::CoreError;
use mustel_core::usecases::db_servers;

use crate::ui::table;

/// Colunas da tabela, na mesma ordem do legado.
const HEADERS: [&str; 6] = ["Name", "Host", "Port", "Username", "Databases", "Auto-DB"];

/// Executa o comando.
pub fn run() -> Result<i32, CoreError> {
    let store = ConfigStore::with_default_path()?;
    let servers = db_servers::list(&store)?;

    if servers.is_empty() {
        // O legado diz `'fur settings db-servers add'`; aqui o binário é o `mustel` (divergência
        // aceita e registrada em `docs/explanation/diferencas-deliberadas.md`).
        println!("No servers configured. Use 'mustel settings db-servers add' to add one.");
        return Ok(0);
    }

    let rows: Vec<Vec<String>> = servers.iter().map(row_for).collect();
    println!("{}", table::render(&HEADERS, &rows));
    Ok(0)
}

/// Monta a linha de um servidor.
fn row_for(server: &ServerConfigEntry) -> Vec<String> {
    vec![
        server.name.clone(),
        server.host.clone(),
        server.port.to_string(),
        server.username.clone(),
        databases_cell(server),
        auto_databases_cell(server),
    ]
}

/// Texto da coluna `Databases`.
fn databases_cell(server: &ServerConfigEntry) -> String {
    if server.fetch_all_databases {
        "(auto)".to_string()
    } else if !server.databases.is_empty() {
        server.databases.join(", ")
    } else {
        "(default)".to_string()
    }
}

/// Texto da coluna `Auto-DB`.
fn auto_databases_cell(server: &ServerConfigEntry) -> String {
    if server.fetch_all_databases {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-112
    fn databases_cell_follows_the_three_legacy_cases() {
        let auto = ServerConfigEntry {
            fetch_all_databases: true,
            ..ServerConfigEntry::default()
        };
        assert_eq!(databases_cell(&auto), "(auto)");

        let listed = ServerConfigEntry {
            databases: vec!["mydb".to_string(), "app_dev".to_string()],
            ..ServerConfigEntry::default()
        };
        assert_eq!(databases_cell(&listed), "mydb, app_dev");

        let none = ServerConfigEntry::default();
        assert_eq!(databases_cell(&none), "(default)");
    }

    #[test]
    /// CONTRACT: IF-112
    fn row_matches_the_legacy_columns() {
        let server = ServerConfigEntry {
            name: "dev".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            databases: vec!["mydb".to_string()],
            ..ServerConfigEntry::default()
        };
        assert_eq!(
            row_for(&server),
            vec![
                "dev".to_string(),
                "localhost".to_string(),
                "5432".to_string(),
                "postgres".to_string(),
                "mydb".to_string(),
                "No".to_string()
            ]
        );
    }
}
