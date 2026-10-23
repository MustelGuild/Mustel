//! Peças comuns do grupo `database` (`backup` e `restore`) no núcleo (T40).
//!
//! Nesses dois comandos o **código de saída é parte da regra** e o **texto é contrato** — e os
//! códigos vêm do *corpo* do comando legado, não da tabela do `Program.cs`: binário ausente é **3** no
//! `backup` e **1** no `restore`; a validação de uso é **2** nos dois. Por isso o núcleo não devolve
//! código: devolve a **mensagem** (o texto do contrato) junto do **tipo** da falha
//! ([`FailureKind`]), e cada frontend tem a sua tabela tipo→código (`commands/database/mod.rs` no CLI;
//! a GUI, que não tem exit code, só mostra a mensagem).
//!
//! O que **não** entra aqui: prompts (a senha vem resolvida) e qualquer texto de layout.

use std::path::{Path, PathBuf};

use crate::error::CoreError;

/// Extensão dos arquivos de dump (`FurLabConstants.DumpExtension`).
pub const DUMP_EXTENSION: &str = ".dump";

/// Nome do arquivo de dump de um banco (`<banco>.dump`).
///
/// Vale para o `-o` omitido do `backup` e para a resolução do `restore`, que busca o arquivo no
/// diretório atual.
pub fn dump_file_name(database: &str) -> String {
    format!("{database}{DUMP_EXTENSION}")
}

/// Conexão validada, sem a senha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// Host do servidor.
    pub host: String,
    /// Porta, **como texto**: o legado a carrega assim e a passa crua ao `pg_dump`/`pg_restore`.
    pub port: String,
    /// Usuário, `None` quando ausente (o legado não tem default aqui e manda string vazia ao binário).
    pub username: Option<String>,
    /// Banco alvo (vazio quando só o `--all` foi pedido).
    pub database: String,
}

/// Por que o comando falhou, para o frontend escolher o **código de saída**.
///
/// A classificação é o que basta para as tabelas do CLI: validação de uso (2 nos dois comandos),
/// binário do PostgreSQL ausente (3 no `backup`, 1 no `restore`) e o resto das falhas de operação (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Argumento inválido: o legado imprime e sai com **2**.
    Usage,
    /// O binário externo não foi encontrado.
    MissingBinary,
    /// Um passo do comando falhou (binário saiu != 0, banco inválido, diretório ausente...).
    OperationFailed,
}

/// Falha que o legado **imprime** e resolve com um código do corpo do comando.
#[derive(Debug)]
pub struct CommandFailure {
    /// Tipo da falha: o frontend mapeia para o código de saída.
    pub kind: FailureKind,
    /// Mensagem já pronta, com o prefixo `Error: ` quando o legado o usa.
    pub message: String,
}

impl CommandFailure {
    /// Monta a falha com a mensagem final.
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Mensagem **sem** o prefixo `Error: ` — o texto da exceção do legado.
    ///
    /// O legado imprime os erros de duas formas: `"Error: " + ex.Message` nos caminhos diretos e
    /// `"Failed to X database 'y': " + ex.Message` nos laços do `--all`. Guardar a mensagem final e
    /// tirar o prefixo aqui reproduz exatamente isso, sem duplicar as mensagens.
    pub fn bare_message(&self) -> &str {
        self.message
            .strip_prefix("Error: ")
            .unwrap_or(&self.message)
    }
}

/// Desfecho de um comando do grupo `database`.
///
/// Separa a falha **de comando** (mensagem + tipo, impressa pelo frontend com o código dele) do erro
/// **do núcleo** (cancelamento, prompt sem terminal, falha de listagem), que sobe como [`CoreError`].
#[derive(Debug)]
pub enum DatabaseFailure {
    /// O comando falhou como o legado falha: imprimir a mensagem e sair com o código do tipo.
    Command(CommandFailure),
    /// Sobe como erro do núcleo (130 no cancelamento, 40 no prompt sem terminal, ...).
    Fatal(CoreError),
}

impl From<CoreError> for DatabaseFailure {
    fn from(error: CoreError) -> Self {
        Self::Fatal(error)
    }
}

impl From<CommandFailure> for DatabaseFailure {
    fn from(failure: CommandFailure) -> Self {
        Self::Command(failure)
    }
}

/// Converte a falha do runner de processos: cancelamento sobe; o resto é saída impressa com código 1.
impl From<crate::process::ProcessError> for DatabaseFailure {
    fn from(error: crate::process::ProcessError) -> Self {
        match error {
            crate::process::ProcessError::Cancelled => Self::Fatal(CoreError::Cancelled),
            other => Self::Command(CommandFailure::new(
                FailureKind::OperationFailed,
                format!("Error: {other}"),
            )),
        }
    }
}

/// Mensagem do legado quando o binário não é encontrado.
pub fn binary_not_found(binary: &str) -> String {
    format!(
        "PostgreSQL binary '{binary}' not found. Please ensure PostgreSQL is installed and the \
         binaries are in your PATH."
    )
}

/// Caminho absoluto, como o `Path.GetFullPath` do legado.
pub fn absolute_path(path: &str) -> String {
    std::path::absolute(path)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

/// Arquivos `*.dump` do diretório (só o nível de cima), em ordem de nome.
pub fn dump_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case(&DUMP_EXTENSION[1..]))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_message_drops_the_legacy_prefix() {
        let printed = CommandFailure::new(
            FailureKind::OperationFailed,
            "Error: pg_restore failed with exit code 1. Error: boom",
        );
        assert_eq!(
            printed.bare_message(),
            "pg_restore failed with exit code 1. Error: boom"
        );

        // Mensagem sem o prefixo volta como veio.
        let sem_prefixo =
            CommandFailure::new(FailureKind::OperationFailed, "Invalid database name");
        assert_eq!(sem_prefixo.bare_message(), "Invalid database name");
    }

    #[test]
    /// CONTRACT: IF-059
    fn the_dump_file_name_matches_the_legacy() {
        assert_eq!(dump_file_name("mydb"), "mydb.dump");
    }

    #[test]
    fn dump_files_are_listed_by_name_and_only_at_the_top_level() {
        let directory = tempfile::tempdir().expect("tempdir");
        std::fs::write(directory.path().join("b.dump"), b"").expect("escreve");
        std::fs::write(directory.path().join("a.DUMP"), b"").expect("escreve");
        std::fs::write(directory.path().join("notas.txt"), b"").expect("escreve");
        std::fs::create_dir(directory.path().join("sub.dump")).expect("cria diretório");

        let names: Vec<String> = dump_files(directory.path())
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            names,
            vec!["a.DUMP".to_string(), "b.dump".to_string()],
            "a extensão não diferencia maiúsculas e diretórios ficam de fora"
        );
    }

    #[test]
    fn the_binary_not_found_message_is_the_legacy_one() {
        assert_eq!(
            binary_not_found("pg_dump"),
            "PostgreSQL binary 'pg_dump' not found. Please ensure PostgreSQL is installed and the \
             binaries are in your PATH."
        );
    }

    #[test]
    fn a_process_error_that_is_not_cancellation_is_an_operation_failure() {
        let spawn = crate::process::ProcessError::Spawn {
            program: "pg_dump".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };

        match DatabaseFailure::from(spawn) {
            DatabaseFailure::Command(failure) => {
                assert_eq!(failure.kind, FailureKind::OperationFailed);
                assert!(
                    failure.message.starts_with("Error: "),
                    "{}",
                    failure.message
                );
            }
            DatabaseFailure::Fatal(error) => panic!("não pode subir como erro do núcleo: {error}"),
        }

        // Cancelamento sobe como cancelamento (exit 130).
        match DatabaseFailure::from(crate::process::ProcessError::Cancelled) {
            DatabaseFailure::Fatal(error) => {
                assert_eq!(error.exit_code(), crate::error::exit_code::CANCELLED)
            }
            DatabaseFailure::Command(failure) => {
                panic!("cancelamento não é saída impressa: {}", failure.message)
            }
        }
    }
}
