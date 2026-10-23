//! Editor de SQL: realce de sintaxe e altura que acompanha o texto.
//!
//! Duas peças, ambas testáveis sem janela:
//!
//! - [`tokens`] classifica o texto em trechos (palavra-chave, string, comentário, número, ...);
//! - [`desired_rows`] decide quantas linhas a caixa precisa para o texto atual.
//!
//! O desenho em si ([`layout_job`]) é a camada fina que pinta os trechos com as cores do tema.
//!
//! **Por que um realçador próprio e não `syntect`**: o `egui_extras` traria o conjunto inteiro de
//! gramáticas (megabytes de dados e custo de compilação) para destacar **uma** linguagem. Um
//! classificador focado em SQL cabe em poucas linhas, não adiciona dependência e é testável.

use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};

/// Classe de um trecho do SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Palavra reservada (`SELECT`, `FROM`, ...).
    Keyword,
    /// Literal de texto entre aspas simples.
    String,
    /// Comentário de linha ou de bloco.
    Comment,
    /// Literal numérico.
    Number,
    /// Identificador (tabela, coluna, alias) ou identificador entre aspas duplas.
    Identifier,
    /// Qualquer outro caractere (operadores, vírgulas, parênteses).
    Punctuation,
}

/// Palavras reservadas destacadas.
///
/// Lista curta e de propósito geral: cobre o que aparece em consulta de dia a dia. Não é a lista
/// completa do Postgres — palavra reservada desconhecida só não recebe cor, o texto continua certo.
const KEYWORDS: &[&str] = &[
    "all",
    "alter",
    "and",
    "any",
    "as",
    "asc",
    "begin",
    "between",
    "by",
    "case",
    "cast",
    "check",
    "column",
    "commit",
    "constraint",
    "create",
    "cross",
    "current",
    "database",
    "default",
    "delete",
    "desc",
    "distinct",
    "do",
    "drop",
    "else",
    "end",
    "except",
    "exists",
    "false",
    "fetch",
    "for",
    "foreign",
    "from",
    "full",
    "grant",
    "group",
    "having",
    "if",
    "in",
    "index",
    "inner",
    "insert",
    "intersect",
    "into",
    "is",
    "join",
    "key",
    "left",
    "like",
    "limit",
    "not",
    "null",
    "offset",
    "on",
    "or",
    "order",
    "outer",
    "primary",
    "references",
    "returning",
    "revoke",
    "right",
    "rollback",
    "schema",
    "select",
    "set",
    "table",
    "then",
    "to",
    "transaction",
    "trigger",
    "true",
    "truncate",
    "union",
    "unique",
    "update",
    "using",
    "values",
    "view",
    "when",
    "where",
    "with",
];

/// Classifica o SQL em trechos contíguos.
///
/// Reconhece comentário de linha (`--`), comentário de bloco (`/* */`), string com aspas simples
/// (com `''` como escape), identificador entre aspas duplas, número e palavra. Qualquer outro
/// caractere vira pontuação.
///
/// String ou comentário não terminados vão até o fim do texto: o editor não pode perder o resto do
/// conteúdo por causa de uma aspa aberta.
pub fn tokens(sql: &str) -> Vec<(std::ops::Range<usize>, Kind)> {
    let bytes = sql.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let start = index;
        let byte = bytes[index];

        // Comentário de linha.
        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            spans.push((start..index, Kind::Comment));
            continue;
        }

        // Comentário de bloco (não aninhado, como o do Postgres em uso comum).
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            while index < bytes.len() {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 2;
                    break;
                }
                index += 1;
            }
            spans.push((start..index, Kind::Comment));
            continue;
        }

        // String de aspas simples, com `''` como aspa escapada.
        if byte == b'\'' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        index += 2;
                        continue;
                    }
                    index += 1;
                    break;
                }
                index += 1;
            }
            spans.push((start..index, Kind::String));
            continue;
        }

        // Identificador entre aspas duplas.
        if byte == b'"' {
            index += 1;
            while index < bytes.len() {
                index += 1;
                if bytes[index - 1] == b'"' {
                    break;
                }
            }
            spans.push((start..index, Kind::Identifier));
            continue;
        }

        // Número: dígitos, com parte decimal opcional.
        if byte.is_ascii_digit() {
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
                index += 1;
            }
            spans.push((start..index, Kind::Number));
            continue;
        }

        // Palavra: letra, dígito, `_` ou `$`.
        if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' {
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'_'
                    || bytes[index] == b'$')
            {
                index += 1;
            }
            let word = sql[start..index].to_ascii_lowercase();
            let kind = if KEYWORDS.contains(&word.as_str()) {
                Kind::Keyword
            } else {
                Kind::Identifier
            };
            spans.push((start..index, kind));
            continue;
        }

        // Qualquer outra coisa: um caractere por vez.
        index += utf8_len(byte);
        spans.push((start..index, Kind::Punctuation));
    }

    spans
}

/// Quantos bytes o caractere que começa em `byte` ocupa.
fn utf8_len(byte: u8) -> usize {
    match byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Quantas linhas a caixa deve mostrar para o texto atual.
///
/// Conta as quebras explícitas **e** estima a quebra automática de linhas longas (a 80 colunas):
/// quem escreve um `SELECT` numa linha só também vê a caixa crescer. O resultado fica entre
/// [`MIN_ROWS`] e [`MAX_ROWS`].
pub fn desired_rows(sql: &str) -> usize {
    const COLUMNS: usize = 80;

    let mut rows = 0;
    for line in sql.lines() {
        let chars = line.chars().count();
        rows += chars.div_ceil(COLUMNS).max(1);
    }

    (rows + 1).clamp(MIN_ROWS, MAX_ROWS)
}

/// Altura mínima da caixa de SQL, em linhas.
pub const MIN_ROWS: usize = 3;
/// Altura máxima antes de a caixa passar a rolar.
pub const MAX_ROWS: usize = 18;

/// Monta o `LayoutJob` pintado para o `TextEdit`.
///
/// As cores vêm do tema atual, então funciona no claro e no escuro sem paleta fixa.
pub fn layout_job(ui: &egui::Ui, sql: &str, wrap_width: f32) -> LayoutJob {
    let font = FontId::monospace(ui.style().text_styles[&egui::TextStyle::Monospace].size);
    let visuals = ui.visuals();
    let dark = visuals.dark_mode;

    let palette = Palette {
        keyword: visuals.hyperlink_color,
        string: if dark {
            Color32::from_rgb(140, 200, 130)
        } else {
            Color32::from_rgb(20, 115, 60)
        },
        comment: visuals.weak_text_color(),
        number: if dark {
            Color32::from_rgb(220, 170, 100)
        } else {
            Color32::from_rgb(150, 80, 10)
        },
        identifier: visuals.text_color(),
        punctuation: visuals.text_color(),
    };

    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    for (range, kind) in tokens(sql) {
        job.append(
            &sql[range],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: palette.color(kind),
                ..Default::default()
            },
        );
    }

    job
}

/// Cores por classe de trecho.
struct Palette {
    keyword: Color32,
    string: Color32,
    comment: Color32,
    number: Color32,
    identifier: Color32,
    punctuation: Color32,
}

impl Palette {
    fn color(&self, kind: Kind) -> Color32 {
        match kind {
            Kind::Keyword => self.keyword,
            Kind::String => self.string,
            Kind::Comment => self.comment,
            Kind::Number => self.number,
            Kind::Identifier => self.identifier,
            Kind::Punctuation => self.punctuation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Devolve `(texto, classe)` para cada trecho — legível nos asserts.
    fn classified(sql: &str) -> Vec<(String, Kind)> {
        tokens(sql)
            .into_iter()
            .map(|(range, kind)| (sql[range].to_string(), kind))
            .collect()
    }

    #[test]
    fn keywords_and_identifiers_are_told_apart() {
        let got = classified("select nome from clientes");
        assert_eq!(got[0], ("select".to_string(), Kind::Keyword));
        assert_eq!(got[1].1, Kind::Punctuation, "espaço é pontuação");
        assert_eq!(got[2], ("nome".to_string(), Kind::Identifier));
        assert_eq!(got[4], ("from".to_string(), Kind::Keyword));
        assert_eq!(got[6], ("clientes".to_string(), Kind::Identifier));
    }

    #[test]
    fn keywords_are_case_insensitive() {
        let got = classified("SeLeCt 1");
        assert_eq!(got[0].1, Kind::Keyword);
        assert_eq!(got[2].1, Kind::Number);
    }

    #[test]
    fn a_string_with_escaped_quote_stays_one_token() {
        let got = classified("select 'it''s'");
        assert_eq!(got[2], ("'it''s'".to_string(), Kind::String));
    }

    #[test]
    fn an_unterminated_string_goes_to_the_end() {
        let got = classified("select 'abc");
        assert_eq!(got.last().unwrap().1, Kind::String);
        assert_eq!(got.last().unwrap().0, "'abc");
    }

    #[test]
    fn line_and_block_comments_are_recognised() {
        let got = classified("select 1 -- nota\nselect /* bloco */ 2");
        assert!(
            got.iter()
                .any(|(text, kind)| text == "-- nota" && *kind == Kind::Comment)
        );
        assert!(
            got.iter()
                .any(|(text, kind)| text == "/* bloco */" && *kind == Kind::Comment)
        );
    }

    #[test]
    fn an_unterminated_block_comment_goes_to_the_end() {
        let got = classified("select /* aberto");
        assert_eq!(got.last().unwrap().1, Kind::Comment);
    }

    #[test]
    fn a_quoted_identifier_is_one_token() {
        let got = classified("select \"Nome Estranho\" from t");
        assert_eq!(got[2], ("\"Nome Estranho\"".to_string(), Kind::Identifier));
    }

    #[test]
    fn numbers_include_the_decimal_part() {
        let got = classified("select 10.5, 2");
        assert_eq!(got[2], ("10.5".to_string(), Kind::Number));
        assert_eq!(got[5], ("2".to_string(), Kind::Number));
    }

    /// O texto inteiro é coberto, sem buraco nem sobreposição — propriedade que o `LayoutJob` exige.
    #[test]
    fn the_spans_cover_the_whole_text_in_order() {
        let sql = "-- c\nSELECT a, 'x' FROM t WHERE b = 1.5; /* fim */";
        let mut position = 0;
        for (range, _) in tokens(sql) {
            assert_eq!(range.start, position, "buraco ou sobreposição em {range:?}");
            position = range.end;
        }
        assert_eq!(
            position,
            sql.len(),
            "o último trecho precisa terminar no fim"
        );
    }

    /// Quebras explícitas e linhas longas fazem a caixa crescer, dentro dos limites.
    #[test]
    fn desired_rows_grows_with_text_up_to_a_cap() {
        assert_eq!(desired_rows(""), MIN_ROWS, "vazia fica no mínimo");
        assert_eq!(desired_rows("select 1"), MIN_ROWS);

        let five_lines = "1\n2\n3\n4\n5";
        assert_eq!(desired_rows(five_lines), 6, "5 linhas + folga");

        let long_line = "x".repeat(400);
        assert!(
            desired_rows(&long_line) >= 5,
            "linha longa quebra em várias"
        );

        let too_many = "1\n".repeat(100);
        assert_eq!(desired_rows(&too_many), MAX_ROWS, "para no teto");
    }
}
