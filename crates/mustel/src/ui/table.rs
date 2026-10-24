//! Tabelas de terminal (adaptador sobre `tabled`).
//!
//! O FurLab usa tabela do Spectre (bordas arredondadas); usamos `Style::rounded()` para ficar próximo.
//! A formatação exata é nível N4 do contrato — a tabela do legado sai em codepage do console, o que
//! já impede comparação byte a byte (ver `fixtures/README.md`).

use tabled::builder::Builder;
use tabled::settings::object::Columns;
use tabled::settings::{Modify, Style, Width};

/// Renderiza uma tabela com cabeçalho.
pub fn render(headers: &[&str], rows: &[Vec<String>]) -> String {
    build(headers, rows).to_string()
}

/// Renderiza uma tabela **ajustada à largura disponível**.
///
/// A regra é a do `Table` do Spectre, que é o que se viu no oráculo: as colunas ficam na largura
/// natural dos seus dados e **só as que não cabem** é que quebram o texto em linhas (a mensagem de
/// erro do oráculo saiu em três linhas enquanto as outras colunas ficaram inteiras).
///
/// O `Width::wrap` do `tabled` sozinho não serve: ele encolhe todas as colunas proporcionalmente e
/// chega a esvaziar as estreitas (medido no T35). Então a conta é feita aqui — natural por coluna,
/// do maior para o menor até caber — e o `wrap` é aplicado só a quem precisa.
pub fn render_fitted(headers: &[&str], rows: &[Vec<String>], width: usize) -> String {
    let natural = natural_widths(headers, rows);

    // Uma tabela do `tabled` gasta 2 de preenchimento por coluna e N+1 separadores.
    let overhead = natural.len() * 2 + natural.len() + 1;
    let mut content = natural.clone();
    let mut over = (content.iter().sum::<usize>() + overhead).saturating_sub(width);

    for index in order_by_natural_width(&natural) {
        if over == 0 {
            break;
        }

        // A coluna larga entrega o que sobra, sem desaparecer.
        let give = over.min(content[index].saturating_sub(1));
        content[index] -= give;
        over -= give;
    }

    let mut table = build(headers, rows);
    for (index, wrapping) in content.iter().enumerate() {
        if *wrapping < natural[index] {
            table.with(
                Modify::new(Columns::new(index..=index))
                    .with(Width::wrap(*wrapping).keep_words(true)),
            );
        }
    }

    table.to_string()
}

/// Índices das colunas, da mais larga para a mais estreita.
fn order_by_natural_width(natural: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..natural.len()).collect();
    order.sort_by_key(|index| std::cmp::Reverse(natural[*index]));
    order
}

/// Largura natural de cada coluna: o maior texto entre o cabeçalho e as células.
fn natural_widths(headers: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|text| text.chars().count()).collect();

    for row in rows {
        for (index, cell) in row.iter().enumerate().take(widths.len()) {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    widths
}

/// Monta a tabela com as bordas do Spectre.
fn build(headers: &[&str], rows: &[Vec<String>]) -> tabled::Table {
    let mut builder = Builder::default();
    builder.push_record(headers.iter().copied());
    for row in rows {
        builder.push_record(row.iter().map(String::as_str));
    }

    let mut table = builder.build();
    table.with(Style::rounded());
    table
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
    fn a_fitted_table_breaks_long_cells_into_more_lines() {
        let row = vec![
            "x".to_string(),
            "3D000: database \"nao_existe\" does not exist".to_string(),
        ];
        let loose = render(&["Status", "Detail"], std::slice::from_ref(&row));
        let fitted = render_fitted(&["Status", "Detail"], &[row], 46);

        assert!(
            fitted.lines().count() > loose.lines().count(),
            "o texto tem de quebrar em mais linhas: {fitted}"
        );
        assert!(
            fitted.lines().all(|line| line.chars().count() <= 46),
            "nenhuma linha pode passar da largura: {fitted}"
        );
        assert!(
            fitted.contains("3D000"),
            "e o conteúdo continua lá: {fitted}"
        );
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
