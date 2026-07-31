use std::path::Path;
use crate::error::{MustelError, Result};

pub struct SecurityUtils;

impl SecurityUtils {
    /// Validates a file path to prevent directory traversal attacks (e.g., ../ or ..\).
    pub fn validate_path<P: AsRef<Path>>(path: P) -> Result<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();

        if path_str.contains("..") {
            return Err(MustelError::Security(format!(
                "Invalid path '{}': Path traversal is not allowed",
                path_str
            )));
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
    fn test_validate_identifier() {
        assert!(SecurityUtils::validate_identifier("my_db").is_ok());
        assert!(SecurityUtils::validate_identifier("app-dev").is_ok());
        assert!(SecurityUtils::validate_identifier("123bad").is_err());
        assert!(SecurityUtils::validate_identifier("db; DROP TABLE users;").is_err());
    }
}
