//! Classificação da consulta em segura ou destrutiva, como o `SqlQueryAnalyzer` do legado.
//!
//! Porte linha a linha do FurLab, **inclusive das limitações conhecidas**: o analisador é um
//! heurístico de regex, não um parser de SQL, e o próprio legado documenta o que ele não pega.
//!
//! - só o corpo do **primeiro** CTE é inspecionado: `WITH a AS (SELECT 1), b AS (DELETE ...)` é
//!   classificado como **seguro**, apesar do `DELETE`;
//! - um `AS` sem `(` depois (CTE malformado) cai em `WITH`, que conta como seguro;
//! - os comentários saem por regex **sem olhar strings**, e o primeiro `AS` é procurado em qualquer
//!   lugar — inclusive dentro de um nome de tabela.
//!
//! Reproduzir isso é o que interessa ao produto: são estas mensagens e esta classificação que decidem
//! se o comando pede confirmação antes de rodar.

use std::sync::LazyLock;

use regex::Regex;

/// Tipo da consulta, pela mesma classificação do legado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Não modifica dados nem esquema.
    Safe,
    /// Modifica dados ou esquema: pede confirmação quando ela está ligada.
    Destructive,
}

/// Palavras que abrem uma operação destrutiva, como no `DestructiveKeywords` do legado.
const DESTRUCTIVE_KEYWORDS: [&str; 10] = [
    "INSERT", "UPDATE", "DELETE", "ALTER", "DROP", "CREATE", "TRUNCATE", "MERGE", "GRANT", "REVOKE",
];

/// Comentários de linha (`--` até o fim da linha) e de bloco (`/* ... */`).
static COMMENTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)--.*?$|/\*[\s\S]*?\*/").expect("padrão de comentário válido")
});

/// A primeira palavra da consulta.
static FIRST_KEYWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(\w+)").expect("padrão de palavra válido"));

/// `SET ROLE`, o `SET` que o legado trata como destrutivo.
static SET_ROLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*SET\s+ROLE").expect("padrão de SET ROLE válido"));

/// Classifica a consulta em [`QueryType::Safe`] ou [`QueryType::Destructive`].
pub fn analyze(query: &str) -> QueryType {
    if query.trim().is_empty() {
        return QueryType::Safe;
    }

    let cleaned = COMMENTS.replace_all(query, "");
    let Some(keyword) = first_keyword(&cleaned) else {
        return QueryType::Safe;
    };

    if is_destructive(&keyword) {
        return QueryType::Destructive;
    }

    if keyword.eq_ignore_ascii_case("SET") && SET_ROLE.is_match(&cleaned) {
        return QueryType::Destructive;
    }

    QueryType::Safe
}

/// Descrição da consulta, no texto que a confirmação mostra (`SELECT`, `DELETE`, ...).
///
/// É o `GetQueryTypeDescription` do legado: a primeira palavra em maiúsculas, ou `UNKNOWN` quando
/// não há nenhuma. Num `WITH`, é a palavra que o CTE escondeu (`SELECT`, a operação destrutiva ou
/// `WITH`).
pub fn description(query: &str) -> String {
    if query.trim().is_empty() {
        return "UNKNOWN".to_string();
    }

    let cleaned = COMMENTS.replace_all(query, "");

    match first_keyword(&cleaned) {
        Some(keyword) => keyword.to_uppercase(),
        None => "UNKNOWN".to_string(),
    }
}

/// Primeira palavra que importa — no `WITH`, a palavra do corpo do primeiro CTE.
fn first_keyword(query: &str) -> Option<String> {
    let captures = FIRST_KEYWORD.captures(query.trim())?;
    let keyword = captures.get(1)?.as_str().to_string();

    if keyword.eq_ignore_ascii_case("WITH") {
        return Some(keyword_from_cte(query));
    }

    Some(keyword)
}

/// `WITH ... AS (...)`: a palavra que abre o corpo do **primeiro** CTE.
///
/// É o `ExtractKeywordFromCte` do legado — procura o primeiro `AS` (em qualquer lugar, inclusive
/// dentro de um nome), o primeiro `(` depois dele e olha a palavra seguinte. O que não for `SELECT`
/// nem uma operação destrutiva vira `WITH`, que o chamador trata como seguro.
fn keyword_from_cte(query: &str) -> String {
    let Some(as_index) = find_as(query) else {
        return "WITH".to_string();
    };
    let Some(open_offset) = query[as_index..].find('(') else {
        return "WITH".to_string();
    };

    let body = &query[as_index + open_offset + 1..];
    let Some(captures) = FIRST_KEYWORD.captures(body.trim()) else {
        return "WITH".to_string();
    };
    let inner = captures
        .get(1)
        .map(|word| word.as_str())
        .unwrap_or_default();

    if inner.eq_ignore_ascii_case("SELECT") {
        return "SELECT".to_string();
    }
    if is_destructive(inner) {
        return inner.to_string();
    }

    "WITH".to_string()
}

/// Índice do primeiro `AS`, sem diferenciar maiúsculas — o `IndexOf("AS", OrdinalIgnoreCase)` do
/// legado. Em bytes: `a`/`A` nunca aparece como byte de continuação em UTF-8, então a varredura é
/// segura e não precisa montar `String` nova.
fn find_as(query: &str) -> Option<usize> {
    let bytes = query.as_bytes();

    (0..bytes.len().saturating_sub(1)).find(|index| {
        matches!(bytes[*index], b'a' | b'A') && matches!(bytes[*index + 1], b's' | b'S')
    })
}

/// A palavra abre uma operação destrutiva?
fn is_destructive(keyword: &str) -> bool {
    DESTRUCTIVE_KEYWORDS
        .iter()
        .any(|candidate| keyword.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os 21 casos do `SqlQueryAnalyzerTests` do legado, um a um.
    #[test]
    fn the_classification_matches_the_legacy_tests() {
        let cases: [(&str, QueryType); 21] = [
            ("SELECT * FROM users", QueryType::Safe),
            ("SHOW tables", QueryType::Safe),
            ("EXPLAIN SELECT * FROM users", QueryType::Safe),
            (
                "INSERT INTO users (name) VALUES ('test')",
                QueryType::Destructive,
            ),
            (
                "UPDATE users SET name = 'test' WHERE id = 1",
                QueryType::Destructive,
            ),
            ("DELETE FROM users WHERE id = 1", QueryType::Destructive),
            (
                "ALTER TABLE users ADD COLUMN email TEXT",
                QueryType::Destructive,
            ),
            ("DROP TABLE users", QueryType::Destructive),
            ("CREATE TABLE test (id INT)", QueryType::Destructive),
            ("TRUNCATE TABLE users", QueryType::Destructive),
            (
                "MERGE INTO target USING source ON (target.id = source.id) WHEN MATCHED THEN UPDATE SET target.val = source.val",
                QueryType::Destructive,
            ),
            ("GRANT SELECT ON users TO readonly", QueryType::Destructive),
            (
                "REVOKE SELECT ON users FROM readonly",
                QueryType::Destructive,
            ),
            ("SET ROLE admin", QueryType::Destructive),
            ("SET search_path TO public", QueryType::Safe),
            ("", QueryType::Safe),
            ("   ", QueryType::Safe),
            ("-- This is a comment\nSELECT * FROM users", QueryType::Safe),
            ("/* block comment */ SELECT * FROM users", QueryType::Safe),
            ("-- careful!\nDELETE FROM users", QueryType::Destructive),
            (
                "WITH temp AS (SELECT * FROM users) SELECT * FROM temp",
                QueryType::Safe,
            ),
        ];

        for (query, expected) in cases {
            assert_eq!(analyze(query), expected, "{query}");
        }
    }

    #[test]
    fn a_cte_that_writes_is_destructive_and_a_reading_cte_is_not() {
        assert_eq!(
            analyze("WITH temp AS (INSERT INTO users VALUES (1)) SELECT * FROM temp"),
            QueryType::Destructive
        );
    }

    #[test]
    fn the_known_limitations_are_kept() {
        // Só o corpo do **primeiro** CTE é olhado: o `DELETE` do segundo passa batido.
        assert_eq!(
            analyze("WITH a AS (SELECT 1), b AS (DELETE FROM users) SELECT * FROM a"),
            QueryType::Safe,
            "limitação conhecida do legado"
        );
        assert_eq!(
            analyze("WITH a AS (DELETE FROM users WHERE id = 1), b AS (SELECT 1) SELECT * FROM b"),
            QueryType::Destructive,
            "o primeiro CTE destrutivo é detectado (o `AS` da esquerda aponta para o `(` certo)"
        );
        // CTE sem `(` depois do `AS`: cai em `WITH`, que é seguro.
        assert_eq!(
            analyze("WITH cte SELECT * FROM cte"),
            QueryType::Safe,
            "CTE malformado"
        );
    }

    #[test]
    fn the_description_is_the_uppercase_keyword() {
        assert_eq!(description("select * from users"), "SELECT");
        assert_eq!(description("delete from users"), "DELETE");
        assert_eq!(
            description("WITH a AS (INSERT INTO t VALUES (1)) SELECT 1"),
            "INSERT"
        );
        assert_eq!(description(""), "UNKNOWN");
        assert_eq!(description("   "), "UNKNOWN");
    }

    #[test]
    fn a_comment_that_swallows_the_query_leaves_nothing_to_classify() {
        // O comentário sai por regex, **sem olhar strings**: o `--` dentro do literal corta a linha e
        // não sobra palavra nenhuma. É o comportamento do legado (o analisador é heurístico).
        assert_eq!(analyze("-- SELECT 1"), QueryType::Safe);
        assert_eq!(description("-- SELECT 1"), "UNKNOWN");
    }
}
