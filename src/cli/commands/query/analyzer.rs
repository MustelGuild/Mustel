use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::Statement;

pub enum QueryType {
    ReadOnly,
    Destructive(String),
}

pub struct SqlQueryAnalyzer;

impl SqlQueryAnalyzer {
    /// Analyzes an SQL string to determine if it contains destructive or state-changing statements.
    pub fn analyze(sql: &str) -> QueryType {
        let dialect = PostgreSqlDialect {};
        
        match Parser::parse_sql(&dialect, sql) {
            Ok(statements) => {
                for stmt in statements {
                    match stmt {
                        Statement::Delete { .. } => return QueryType::Destructive("DELETE".into()),
                        Statement::Drop { .. } => return QueryType::Destructive("DROP".into()),
                        Statement::Truncate { .. } => return QueryType::Destructive("TRUNCATE".into()),
                        Statement::Update { .. } => return QueryType::Destructive("UPDATE".into()),
                        Statement::AlterTable { .. } => return QueryType::Destructive("ALTER TABLE".into()),
                        Statement::Insert { .. } => return QueryType::Destructive("INSERT".into()),
                        Statement::CreateDatabase { .. } | Statement::CreateTable { .. } => {
                            return QueryType::Destructive("CREATE".into())
                        }
                        _ => {}
                    }
                }
                QueryType::ReadOnly
            }
            Err(_) => {
                // Fallback regex detection if SQL AST parser cannot parse complex vendor SQL
                let upper = sql.to_uppercase();
                if upper.contains("DELETE FROM") || upper.contains("DROP TABLE") || upper.contains("DROP DATABASE") {
                    QueryType::Destructive("DELETE/DROP".into())
                } else if upper.contains("TRUNCATE ") {
                    QueryType::Destructive("TRUNCATE".into())
                } else if upper.contains("UPDATE ") && upper.contains(" SET ") {
                    QueryType::Destructive("UPDATE".into())
                } else if upper.contains("ALTER TABLE") {
                    QueryType::Destructive("ALTER TABLE".into())
                } else {
                    QueryType::ReadOnly
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_query_is_readonly() {
        let query = "SELECT id, name FROM users WHERE active = true;";
        assert!(matches!(SqlQueryAnalyzer::analyze(query), QueryType::ReadOnly));
    }

    #[test]
    fn test_delete_query_is_destructive() {
        let query = "DELETE FROM users WHERE active = false;";
        assert!(matches!(SqlQueryAnalyzer::analyze(query), QueryType::Destructive(_)));
    }

    #[test]
    fn test_drop_query_is_destructive() {
        let query = "DROP TABLE audit_logs;";
        assert!(matches!(SqlQueryAnalyzer::analyze(query), QueryType::Destructive(_)));
    }
}
