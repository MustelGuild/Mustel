//! Tabelas de terminal (adaptador sobre `tabled`).
//!
//! O FurLab usa tabela do Spectre (bordas arredondadas); usamos `Style::rounded()` para ficar próximo.
//! A formatação exata é nível N4 do contrato — a tabela do legado sai em codepage do console, o que
//! já impede comparação byte a byte (ver `oracle/README.md`).

use tabled::builder::Builder;
use tabled::settings::Style;

/// Renderiza uma tabela com cabeçalho.
pub fn render(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut builder = Builder::default();
    builder.push_record(headers.iter().copied());
    for row in rows {
        builder.push_record(row.iter().map(String::as_str));
    }

    let mut table = builder.build();
    table.with(Style::rounded());
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headers_and_rows() {
        let table = render(
            &["Name", "Host"],
            &[vec!["dev".to_string(), "localhost".to_string()]],
        );
        assert!(table.contains("Name"));
        assert!(table.contains("dev"));
        assert!(table.contains('╭'), "bordas arredondadas: {table}");
    }

    #[test]
    fn renders_header_only_when_there_are_no_rows() {
        let table = render(&["Name"], &[]);
        assert!(table.contains("Name"));
    }
}
