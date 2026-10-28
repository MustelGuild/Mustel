//! Classificação da consulta em segura ou destrutiva.
//!
//! O `sqlparser` é a fonte de verdade: ele entende CTE, `EXPLAIN ANALYZE` e a sintaxe do
//! PostgreSQL que o heurístico de regex do legado não entendia. A classificação continua sendo a
//! política do produto (as dez palavras destrutivas do legado), mas agora percorre a AST inteira.
//!
//! Quando o parse falha, o tokenizador procura palavras destrutivas fora de strings e comentários.
//! Se até o tokenizador falhar, o heurístico antigo é o último recurso: nunca classificamos como
//! segura uma consulta que não conseguimos ler.

use std::sync::LazyLock;

use regex::Regex;
use sqlparser::ast::{Query, Set, SetExpr, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer};

/// Tipo da consulta, pela classificação do produto.
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

    match Parser::parse_sql(&PostgreSqlDialect {}, query) {
        Ok(statements) => {
            if statements
                .iter()
                .any(|statement| destructive_operation(statement).is_some())
            {
                QueryType::Destructive
            } else {
                QueryType::Safe
            }
        }
        Err(_) => fallback_analysis(query),
    }
}

/// Descrição da consulta, no texto que a confirmação mostra (`SELECT`, `DELETE`, ...).
///
/// Nos 21 casos do legado, o texto continua o mesmo. Nos casos novos, a operação real é mostrada:
/// um `DELETE` escondido num CTE ou em `EXPLAIN ANALYZE` aparece como `DELETE`, não como
/// `SELECT`/`WITH`/`EXPLAIN`.
pub fn description(query: &str) -> String {
    if query.trim().is_empty() {
        return "UNKNOWN".to_string();
    }

    if let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, query)
        && let Some(operation) = statements
            .iter()
            .find_map(|statement| destructive_operation(statement))
    {
        return operation.to_string();
    }

    legacy_description(query)
}

/// Operação destrutiva que a AST contém, se houver.
fn destructive_operation(statement: &Statement) -> Option<&'static str> {
    match statement {
        Statement::Query(query) => query_operation(query),
        Statement::Explain {
            analyze, statement, ..
        } => {
            if *analyze {
                destructive_operation(statement)
            } else {
                None
            }
        }
        Statement::Set(Set::SetRole { .. }) => Some("SET"),
        _ => destructive_keyword(&statement_keyword(statement)),
    }
}

/// Operação destrutiva dentro de uma `Query`, incluindo todos os CTEs e o corpo.
fn query_operation(query: &Query) -> Option<&'static str> {
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            if let Some(operation) = query_operation(&cte.query) {
                return Some(operation);
            }
        }
    }

    set_expr_operation(&query.body)
}

/// Operação destrutiva dentro de um corpo de `Query`.
fn set_expr_operation(body: &SetExpr) -> Option<&'static str> {
    match body {
        SetExpr::Insert(_) => Some("INSERT"),
        SetExpr::Update(_) => Some("UPDATE"),
        SetExpr::Delete(_) => Some("DELETE"),
        SetExpr::Merge(_) => Some("MERGE"),
        SetExpr::Query(query) => query_operation(query),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_operation(left).or_else(|| set_expr_operation(right))
        }
        SetExpr::Select(_) | SetExpr::Values(_) | SetExpr::Table(_) => None,
    }
}

/// Primeira palavra de um `Statement`, para os casos que a AST não precisa detalhar.
fn statement_keyword(statement: &Statement) -> String {
    statement
        .to_string()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

/// Se o parse falhou, o tokenizador ainda separa strings, comentários e palavras.
fn fallback_analysis(query: &str) -> QueryType {
    let dialect = PostgreSqlDialect {};
    let Ok(tokens) = Tokenizer::new(&dialect, query).tokenize() else {
        return legacy_analyze(query);
    };

    let words: Vec<String> = tokens
        .iter()
        .filter_map(|token| match token {
            Token::Word(word) if word.quote_style.is_none() => {
                Some(word.value.to_ascii_uppercase())
            }
            _ => None,
        })
        .collect();

    if words
        .windows(2)
        .any(|pair| pair[0] == "SET" && pair[1] == "ROLE")
        || words.iter().any(|word| is_destructive(word))
    {
        QueryType::Destructive
    } else {
        QueryType::Safe
    }
}

/// Heurístico antigo, mantido como último recurso quando o tokenizador também falha.
fn legacy_analyze(query: &str) -> QueryType {
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

/// Descrição do legado, usada quando a AST não encontra operação destrutiva.
fn legacy_description(query: &str) -> String {
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

/// Índice do primeiro `AS`, sem diferenciar maiúsculas.
fn find_as(query: &str) -> Option<usize> {
    let bytes = query.as_bytes();

    (0..bytes.len().saturating_sub(1)).find(|index| {
        matches!(bytes[*index], b'a' | b'A') && matches!(bytes[*index + 1], b's' | b'S')
    })
}

/// A palavra é uma das dez operações destrutivas do legado?
fn is_destructive(keyword: &str) -> bool {
    destructive_keyword(keyword).is_some()
}

/// Versão canônica, em maiúsculas, de uma palavra destrutiva.
fn destructive_keyword(keyword: &str) -> Option<&'static str> {
    DESTRUCTIVE_KEYWORDS
        .iter()
        .copied()
        .find(|candidate| keyword.eq_ignore_ascii_case(candidate))
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
    /// CONTRACT: IF-254
    fn every_cte_is_inspected() {
        assert_eq!(
            analyze("WITH a AS (SELECT 1), b AS (DELETE FROM users) SELECT * FROM a"),
            QueryType::Destructive
        );
        assert_eq!(
            analyze("WITH a AS (DELETE FROM users WHERE id = 1), b AS (SELECT 1) SELECT * FROM b"),
            QueryType::Destructive
        );
    }

    #[test]
    /// CONTRACT: IF-254
    fn explain_analyze_of_dml_is_destructive_but_plain_explain_is_not() {
        assert_eq!(analyze("EXPLAIN DELETE FROM users"), QueryType::Safe);
        assert_eq!(
            analyze("EXPLAIN ANALYZE DELETE FROM users"),
            QueryType::Destructive
        );
        assert_eq!(
            analyze("EXPLAIN ANALYZE UPDATE users SET name = 'x'"),
            QueryType::Destructive
        );
    }

    #[test]
    fn a_parse_failure_with_a_destructive_word_is_not_safe() {
        assert_eq!(analyze("SELEC DELETE FROM users"), QueryType::Destructive);
        assert_eq!(
            analyze("WITH broken AS ( DELETE FROM users"),
            QueryType::Destructive
        );
        assert_eq!(
            analyze("SELEC 'delete' FROM users"),
            QueryType::Safe,
            "string literal não é operação"
        );
    }

    #[test]
    fn quoted_identifiers_that_look_destructive_are_not_operations() {
        assert_eq!(analyze("SELECT \"delete\" FROM users"), QueryType::Safe);
        assert_eq!(analyze("SELECT 'delete' FROM users"), QueryType::Safe);
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
    /// CONTRACT: IF-255
    fn the_description_shows_the_real_operation_in_the_new_cases() {
        assert_eq!(
            description("WITH a AS (SELECT 1), b AS (DELETE FROM users) SELECT * FROM a"),
            "DELETE"
        );
        assert_eq!(description("EXPLAIN ANALYZE DELETE FROM users"), "DELETE");
        assert_eq!(
            description("EXPLAIN DELETE FROM users"),
            "EXPLAIN",
            "sem ANALYZE, a operação não é executada"
        );
    }

    #[test]
    fn a_comment_that_swallows_the_query_leaves_nothing_to_classify() {
        assert_eq!(analyze("-- SELECT 1"), QueryType::Safe);
        assert_eq!(description("-- SELECT 1"), "UNKNOWN");
    }
}
