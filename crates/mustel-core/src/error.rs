//! Erros do núcleo e a tradução para exit codes.
//!
//! Os códigos foram **observados no oráculo** (`legacy/FurLab.CLI/Program.cs`), não copiados da
//! documentação — o `AGENTS.md` antigo dizia `2` para argumentos inválidos, e o binário devolve `41`.

use std::path::PathBuf;

use crate::config::ConfigError;

/// Exit codes do contrato de paridade.
pub mod exit_code {
    /// Sucesso.
    pub const SUCCESS: i32 = 0;
    /// Erro geral não classificado.
    pub const GENERAL: i32 = 1;
    /// Erro de Postgres com SQLSTATE.
    pub const POSTGRES_WITH_SQLSTATE: i32 = 10;
    /// Erro de Postgres sem SQLSTATE.
    pub const POSTGRES: i32 = 11;
    /// Outro erro de I/O.
    pub const IO: i32 = 20;
    /// Diretório não encontrado.
    pub const DIRECTORY_NOT_FOUND: i32 = 21;
    /// Arquivo não encontrado.
    pub const FILE_NOT_FOUND: i32 = 22;
    /// Acesso negado.
    pub const ACCESS_DENIED: i32 = 30;
    /// Estado inválido para a operação.
    pub const INVALID_STATE: i32 = 40;
    /// Argumento inválido.
    pub const INVALID_ARGUMENT: i32 = 41;
    /// Tempo esgotado.
    pub const TIMEOUT: i32 = 50;
    /// Cancelado pelo usuário (Ctrl+C).
    pub const CANCELLED: i32 = 130;
}

/// Que tipo de caminho faltou — o oráculo diferencia diretório (21) de arquivo (22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// Arquivo esperado.
    File,
    /// Diretório esperado.
    Directory,
}

/// Erro produzido pelos casos de uso do núcleo.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// Falha de conexão/execução no PostgreSQL.
    #[error("{message}")]
    Postgres {
        /// SQLSTATE quando o servidor informou um.
        sqlstate: Option<String>,
        /// Mensagem exibida ao usuário.
        message: String,
    },
    /// Caminho esperado que não existe.
    #[error("{path} not found")]
    PathNotFound {
        /// Caminho ausente.
        path: PathBuf,
        /// Se o esperado era arquivo ou diretório.
        kind: PathKind,
    },
    /// Erro de I/O genérico.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Erro de configuração.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Argumento inválido informado pelo usuário.
    #[error("{0}")]
    InvalidArgument(String),
    /// Estado inválido para a operação pedida.
    #[error("{0}")]
    InvalidState(String),
    /// Tempo esgotado.
    #[error("{0}")]
    Timeout(String),
    /// Operação cancelada pelo usuário.
    #[error("operation cancelled")]
    Cancelled,
    /// Qualquer outro erro.
    #[error("{0}")]
    Other(String),
}

impl CoreError {
    /// Cria um erro de Postgres a partir de uma mensagem sem SQLSTATE.
    pub fn postgres(message: impl Into<String>) -> Self {
        Self::Postgres {
            sqlstate: None,
            message: message.into(),
        }
    }

    /// Código de saída correspondente, para paridade com o binário legado.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Postgres {
                sqlstate: Some(_), ..
            } => exit_code::POSTGRES_WITH_SQLSTATE,
            Self::Postgres { sqlstate: None, .. } => exit_code::POSTGRES,
            Self::PathNotFound { kind, .. } => match kind {
                PathKind::Directory => exit_code::DIRECTORY_NOT_FOUND,
                PathKind::File => exit_code::FILE_NOT_FOUND,
            },
            Self::Io(error) => match error.kind() {
                std::io::ErrorKind::NotFound => exit_code::FILE_NOT_FOUND,
                std::io::ErrorKind::PermissionDenied => exit_code::ACCESS_DENIED,
                _ => exit_code::IO,
            },
            Self::Config(config_error) => match config_error {
                ConfigError::EmptyServerName => exit_code::INVALID_ARGUMENT,
                ConfigError::ServerNotFound(_) => exit_code::INVALID_ARGUMENT,
                ConfigError::Read { source, .. } | ConfigError::Write { source, .. } => {
                    match source.kind() {
                        std::io::ErrorKind::NotFound => exit_code::FILE_NOT_FOUND,
                        std::io::ErrorKind::PermissionDenied => exit_code::ACCESS_DENIED,
                        _ => exit_code::IO,
                    }
                }
                ConfigError::NoConfigDir | ConfigError::Parse { .. } => exit_code::INVALID_STATE,
            },
            Self::InvalidArgument(_) => exit_code::INVALID_ARGUMENT,
            Self::InvalidState(_) => exit_code::INVALID_STATE,
            Self::Timeout(_) => exit_code::TIMEOUT,
            Self::Cancelled => exit_code::CANCELLED,
            Self::Other(_) => exit_code::GENERAL,
        }
    }

    /// Indica se o erro é fruto de cancelamento pelo usuário.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-150
    fn maps_every_variant_to_the_observed_exit_code() {
        let cases: Vec<(CoreError, i32)> = vec![
            (
                CoreError::Postgres {
                    sqlstate: Some("42P01".to_string()),
                    message: "missing table".to_string(),
                },
                exit_code::POSTGRES_WITH_SQLSTATE,
            ),
            (
                CoreError::postgres("connection refused"),
                exit_code::POSTGRES,
            ),
            (
                CoreError::PathNotFound {
                    path: PathBuf::from("backup.dump"),
                    kind: PathKind::File,
                },
                exit_code::FILE_NOT_FOUND,
            ),
            (
                CoreError::PathNotFound {
                    path: PathBuf::from("backups"),
                    kind: PathKind::Directory,
                },
                exit_code::DIRECTORY_NOT_FOUND,
            ),
            (
                CoreError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing")),
                exit_code::FILE_NOT_FOUND,
            ),
            (
                CoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "denied",
                )),
                exit_code::ACCESS_DENIED,
            ),
            (
                CoreError::Io(std::io::Error::other("disk on fire")),
                exit_code::IO,
            ),
            (
                CoreError::Config(ConfigError::EmptyServerName),
                exit_code::INVALID_ARGUMENT,
            ),
            (
                CoreError::Config(ConfigError::ServerNotFound("dev".to_string())),
                exit_code::INVALID_ARGUMENT,
            ),
            (
                CoreError::Config(ConfigError::Parse {
                    message: "expected value".to_string(),
                }),
                exit_code::INVALID_STATE,
            ),
            (
                CoreError::Config(ConfigError::NoConfigDir),
                exit_code::INVALID_STATE,
            ),
            (
                CoreError::InvalidArgument("bad port".to_string()),
                exit_code::INVALID_ARGUMENT,
            ),
            (
                CoreError::InvalidState("nothing to restore".to_string()),
                exit_code::INVALID_STATE,
            ),
            (
                CoreError::Timeout("timed out".to_string()),
                exit_code::TIMEOUT,
            ),
            (CoreError::Cancelled, exit_code::CANCELLED),
            (CoreError::Other("boom".to_string()), exit_code::GENERAL),
        ];

        for (error, expected) in cases {
            assert_eq!(error.exit_code(), expected, "wrong code for {error:?}");
        }
    }

    #[test]
    /// CONTRACT: IF-153
    fn cancellation_is_identifiable() {
        assert!(CoreError::Cancelled.is_cancelled());
        assert!(!CoreError::Other("boom".to_string()).is_cancelled());
    }

    #[test]
    fn success_code_is_zero() {
        assert_eq!(exit_code::SUCCESS, 0);
    }
}
