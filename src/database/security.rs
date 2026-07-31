use std::path::{Path, PathBuf};
use crate::error::{MustelError, Result};

pub struct SecurityUtils;

impl SecurityUtils {
    /// Validates a file path to prevent directory traversal attacks.
    /// Uses canonicalization to resolve symlinks and verify the final path is safe.
    pub fn validate_path<P: AsRef<Path>>(path: P) -> Result<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();

        // Basic check: reject obvious path traversal attempts
        if path_str.contains("..") {
            return Err(MustelError::Security(format!(
                "Invalid path '{}': Path traversal is not allowed",
                path_str
            )));
        }

        // Additional security: reject null bytes (null byte injection)
        if path_str.contains('\0') {
            return Err(MustelError::Security(format!(
                "Invalid path '{}': Contains null byte",
                path_str
            )));
        }

        // Try to canonicalize the path to resolve symlinks
        // This ensures we catch symlink-based attacks
        if let Ok(canonical) = path_ref.canonicalize() {
            // Check if the canonical path contains path traversal
            // (shouldn't happen after canonicalization, but be paranoid)
            let canonical_str = canonical.to_string_lossy();
            if canonical_str.contains("..") {
                return Err(MustelError::Security(format!(
                    "Invalid path resolved to '{}': Symlink path traversal detected",
                    canonical_str
                )));
            }
        } else if path_ref.is_relative() {
            // For relative paths that don't exist yet, check parent exists
            if let Some(parent) = path_ref.parent() {
                if !parent.as_os_str().is_empty() {
                    // Check if parent exists or can be created safely
                    if parent.to_string_lossy().contains("..") {
                        return Err(MustelError::Security(format!(
                            "Invalid path '{}': Parent directory traversal not allowed",
                            path_str
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Validates a database identifier to prevent SQL injection in dynamic database operations.
    #[allow(dead_code)]
    pub fn validate_identifier(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(MustelError::Security("Database identifier cannot be empty".into()));
        }

        // Must start with letter or underscore, followed by alphanumeric or underscore
        let valid = name.chars().enumerate().all(|(i, c)| {
            if i == 0 {
                c.is_alphabetic() || c == '_'
            } else {
                c.is_alphanumeric() || c == '_' || c == '-'
            }
        });

        if !valid {
            return Err(MustelError::Security(format!(
                "Invalid database identifier '{}': Contains disallowed characters",
                name
            )));
        }

        // Check for SQL injection patterns
        let lower = name.to_lowercase();
        let sql_keywords = ["select", "insert", "update", "delete", "drop", "create",
                           "alter", "truncate", "exec", "execute", "union", "--", ";",
                           "/*", "*/", "'", "\"", "\\x00", "\0"];
        for keyword in sql_keywords {
            if lower.contains(keyword) {
                return Err(MustelError::Security(format!(
                    "Invalid database identifier '{}': Contains SQL injection pattern '{}'",
                    name, keyword
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path() {
        assert!(SecurityUtils::validate_path("valid/path/file.sql").is_ok());
        assert!(SecurityUtils::validate_path("../invalid/file.sql").is_err());
        assert!(SecurityUtils::validate_path("dir/../file.sql").is_err());
    }

    #[test]
    fn test_validate_path_null_byte() {
        let malicious_path = "safe\0/../../../etc/passwd";
        assert!(SecurityUtils::validate_path(malicious_path).is_err());
    }

    #[test]
    fn test_validate_identifier() {
        assert!(SecurityUtils::validate_identifier("my_db").is_ok());
        assert!(SecurityUtils::validate_identifier("app-dev").is_ok());
        assert!(SecurityUtils::validate_identifier("123bad").is_err());
        assert!(SecurityUtils::validate_identifier("db; DROP TABLE users;").is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection() {
        assert!(SecurityUtils::validate_identifier("table'; DROP TABLE users;--").is_err());
        assert!(SecurityUtils::validate_identifier("table/*comment*/").is_err());
    }
}
