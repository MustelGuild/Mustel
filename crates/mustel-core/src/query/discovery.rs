//! Descoberta de bancos do `query run` (`--all`, `fetchAllDatabases`, `excludePatterns`).
//!
//! Porte de `GetDatabasesForServerAsync` + `ListDatabasesAsync` + `MatchesPattern` do legado. São
//! **dois filtros diferentes**, ambos medidos no código:
//!
//! 1. `excludePatterns` do **cadastro do servidor** — curinga com `*`, sem diferenciar maiúsculas,
//!    aplicado **durante a descoberta** (só quando o servidor descobre bancos);
//! 2. `--exclude` da linha de comando — lista de **nomes exatos** separada por vírgula, sem
//!    diferenciar maiúsculas, aplicada **no fim**, valendo também para o caminho sem descoberta.
//!
//! Quando a descoberta falha, o legado **não** derruba o comando: avisa e cai para os bancos do
//! cadastro (ou para nenhum, se o cadastro não tiver bancos).

use std::collections::HashSet;

use regex::Regex;

use crate::error::CoreError;
use crate::postgres::settings::ConnectionSettings;

/// Nomes exatos do `--exclude` (lista separada por vírgula).
///
/// O legado usa `Split(',', RemoveEmptyEntries | TrimEntries)` com comparador
/// `OrdinalIgnoreCase` ✓ — daí o `HashSet` de minúsculas.
pub fn exclude_names(exclude: Option<&str>) -> HashSet<String> {
    let Some(exclude) = exclude.filter(|value| !value.trim().is_empty()) else {
        return HashSet::new();
    };

    exclude
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// O banco casa o padrão do cadastro?
///
/// Reproduz o `MatchesPattern`: `Regex.Escape` do padrão, `\*` vira `.*`, âncoras nas pontas e
/// comparação sem diferenciar maiúsculas. Escapar antes é o que faz os outros metacaracteres
/// (`?`, `[`, `.`) valerem como texto.
pub fn matches_pattern(database: &str, pattern: &str) -> bool {
    let expression = format!("^{}$", regex::escape(pattern).replace("\\*", ".*"));
    let Ok(regex) = Regex::new(&format!("(?i){expression}")) else {
        return false;
    };

    regex.is_match(database)
}

/// Filtra a lista pelos dois critérios (padrões do cadastro e nomes do `--exclude`).
pub fn filter_databases(
    databases: Vec<String>,
    exclude_patterns: &[String],
    exclude_names: &HashSet<String>,
) -> Vec<String> {
    databases
        .into_iter()
        .filter(|database| {
            !exclude_patterns
                .iter()
                .any(|pattern| matches_pattern(database, pattern))
        })
        .filter(|database| !exclude_names.contains(&database.to_lowercase()))
        .collect()
}

/// Lista os bancos do servidor com a query de descoberta do legado.
///
/// A query é **diferente** da do `settings db-servers test`: aqui filtra `datallowconn` e **não**
/// ordena.
pub async fn discover_databases(settings: &ConnectionSettings) -> Result<Vec<String>, CoreError> {
    crate::postgres::client::list_databases_for_discovery(settings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_names_are_trimmed_case_insensitive_and_ignore_blanks() {
        let names = exclude_names(Some(" Dev , PROD ,, "));

        assert!(names.contains("dev"));
        assert!(names.contains("prod"));
        assert_eq!(names.len(), 2);
        assert!(exclude_names(None).is_empty());
        assert!(exclude_names(Some("   ")).is_empty());
    }

    #[test]
    fn patterns_follow_the_legacy_regex() {
        // Curinga `*` em qualquer posição, sem diferenciar maiúsculas.
        assert!(matches_pattern("log_2026", "log*"));
        assert!(matches_pattern("LOG_2026", "log*"));
        assert!(matches_pattern("app_dev", "*_dev"));
        assert!(matches_pattern("qualquer", "*"));
        assert!(matches_pattern("exato", "exato"));

        assert!(!matches_pattern("app_prod", "*_dev"));
        assert!(!matches_pattern("xlog", "log*"));

        // Metacaracteres que o `Regex.Escape` transforma em texto literal.
        assert!(matches_pattern("a.b", "a.b"));
        assert!(!matches_pattern("aXb", "a.b"), "o ponto não é curinga");
        assert!(matches_pattern("a?b", "a?b"), "o `?` é literal");
        assert!(
            !matches_pattern("aXb", "a?b"),
            "e não um curinga de um caractere"
        );
    }

    #[test]
    fn filtering_applies_patterns_and_names() {
        let databases = vec![
            "mustel_test".to_string(),
            "postgres".to_string(),
            "template_extra".to_string(),
            "dev".to_string(),
        ];
        let patterns = vec!["template_*".to_string()];
        let names = exclude_names(Some("POSTGRES"));

        assert_eq!(
            filter_databases(databases, &patterns, &names),
            vec!["mustel_test".to_string(), "dev".to_string()]
        );
    }

    #[test]
    fn without_filters_every_database_stays() {
        let databases = vec!["a".to_string(), "b".to_string()];

        assert_eq!(filter_databases(databases, &[], &HashSet::new()).len(), 2);
    }
}
