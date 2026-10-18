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

/// Renderiza uma tabela com título centralizado acima dela (o `Table.Title` do Spectre).
///
/// O `pgpass list` do legado tem título (`pgpass.conf`) e o `db-servers ls` não; a centralização
/// exata é N4, mas o **texto** do título é conteúdo conferido pelo comparador de tokens ASCII.
pub fn render_titled(title: &str, headers: &[&str], rows: &[Vec<String>]) -> String {
    let table = render(headers, rows);
    let width = table
        .lines()
        .next()
        .map(|line| line.chars().count())
        .unwrap_or_default();
    let padding = width.saturating_sub(title.chars().count()) / 2;

    format!("{}{title}\n{table}", " ".repeat(padding))
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

    #[test]
    fn a_title_is_centered_above_the_table() {
        let table = render_titled(
            "pgpass.conf",
            &["HOSTNAME", "PASSWORD"],
            &[vec!["localhost".to_string(), "****".to_string()]],
        );

        let title_line = table.lines().next().expect("título na primeira linha");
        assert!(title_line.contains("pgpass.conf"), "{table}");
        assert!(
            title_line.starts_with(' '),
            "o título do legado vem centralizado: {title_line:?}"
        );
        assert!(table.contains("HOSTNAME"), "a tabela vem depois do título");
    }
}
