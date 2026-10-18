//! Conversão de valores do servidor para o texto que vai ao CSV.
//!
//! **Esta é a parte sensível a paridade do `query run`.** O legado escreve CSV a partir de valores
//! do Npgsql; o texto exato de cada tipo é uma decisão de formatação que só pode ser confirmada
//! comparando com o oráculo (T32) — por isso o mapeamento aqui é explícito e testável, em vez de
//! espalhado pelo código de escrita do CSV.
//!
//! O desenho separa duas coisas: [`TextValue`] descreve o que veio do servidor (construível em teste
//! sem conexão) e [`format_cell`] decide o texto. A fronteira com o driver (que produz
//! `TextValue` a partir de uma linha) fica em [`crate::postgres::client`].

/// Valor de uma célula, já reduzido ao que importa para exportar.
#[derive(Debug, Clone, PartialEq)]
pub enum TextValue<'a> {
    /// `NULL` do SQL.
    Null,
    /// Booleano.
    Bool(bool),
    /// Inteiro (int2/int4/int8).
    Int(i64),
    /// Ponto flutuante (float4/float8).
    Float(f64),
    /// Texto, `varchar`, `char`, enum, uuid, json/jsonb — o que o servidor mandou como texto.
    Text(&'a str),
    /// Binário (`bytea`).
    Bytes(&'a [u8]),
    /// Lista (arrays), na ordem do servidor.
    List(Vec<TextValue<'a>>),
}

/// Texto de uma célula para o CSV.
///
/// `NULL` vira campo vazio — o mesmo que o export do legado faz para valor nulo.
pub fn format_cell(value: &TextValue<'_>) -> String {
    match value {
        TextValue::Null => String::new(),
        TextValue::Bool(value) => value.to_string(),
        TextValue::Int(value) => value.to_string(),
        TextValue::Float(value) => format_float(*value),
        TextValue::Text(value) => (*value).to_string(),
        TextValue::Bytes(bytes) => format_bytes(bytes),
        TextValue::List(items) => {
            let inner: Vec<String> = items.iter().map(format_cell).collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// `float` no formato do .NET (`1.5`, `1E+20`), sem o `inf`/`NaN` do Rust.
fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }

    let text = value.to_string();
    if value.fract() == 0.0 && !text.contains(['e', 'E', '.']) {
        format!("{text}.0")
    } else {
        text
    }
}

/// `bytea` em hexadecimal com prefixo `0x`.
///
/// **Não verificado contra o oráculo**: o texto exato que o Npgsql produz para `bytea` precisa ser
/// confirmado no T32 (comparando CSV do legado e do Mustel com o mesmo dado). Enquanto isso, este é
/// o formato escolhido, isolado em uma função para ser corrigido num só lugar.
fn format_bytes(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(2 + bytes.len() * 2);
    text.push_str("0x");
    for byte in bytes {
        text.push_str(&format!("{byte:02X}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_becomes_an_empty_field() {
        assert_eq!(format_cell(&TextValue::Null), "");
    }

    #[test]
    fn scalars_are_rendered_as_text() {
        assert_eq!(format_cell(&TextValue::Bool(true)), "true");
        assert_eq!(format_cell(&TextValue::Int(-42)), "-42");
        assert_eq!(format_cell(&TextValue::Text("mydb")), "mydb");
    }

    #[test]
    fn floats_keep_the_dot_for_whole_numbers() {
        assert_eq!(format_cell(&TextValue::Float(1.5)), "1.5");
        assert_eq!(format_cell(&TextValue::Float(2.0)), "2.0");
        assert_eq!(format_cell(&TextValue::Float(f64::NAN)), "NaN");
        assert_eq!(format_cell(&TextValue::Float(f64::INFINITY)), "Infinity");
    }

    #[test]
    fn lists_use_postgres_braces_and_commas() {
        let list = TextValue::List(vec![
            TextValue::Int(1),
            TextValue::Int(2),
            TextValue::Text("três"),
        ]);
        assert_eq!(format_cell(&list), "{1,2,três}");
    }

    #[test]
    fn bytes_are_hex_prefixed_like_npgsql() {
        assert_eq!(format_cell(&TextValue::Bytes(&[0x0A, 0xFF])), "0x0AFF");
        assert_eq!(format_cell(&TextValue::Bytes(&[])), "0x");
    }
}
