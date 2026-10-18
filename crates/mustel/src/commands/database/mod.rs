//! Comandos do grupo `database`.

pub mod backup;
pub mod pgpass;
pub mod restore;

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;

use crate::cli::{DatabaseArgs, DatabaseCommand, PgpassCommand};

/// Extensão dos arquivos de dump (`FurLabConstants.DumpExtension`).
pub(crate) const DUMP_EXTENSION: &str = ".dump";

/// Nome do arquivo de dump de um banco (`<banco>.dump`).
///
/// Vale para o `-o` omitido do `backup` e para a resolução do `restore`, que busca o arquivo no
/// diretório atual.
pub(crate) fn dump_file_name(database: &str) -> String {
    format!("{database}{DUMP_EXTENSION}")
}

/// Falha que o legado **imprime** e resolve com um código de saída próprio.
///
/// Códigos como 2 (validação), 3 (binário ausente no `backup`) e 1/2 do `restore` vêm do **corpo** do
/// comando, e não da tabela de exceções do `Program.cs` — por isso não passam por [`CoreError`], que
/// mapearia tudo para 40/41.
#[derive(Debug)]
pub(crate) struct Printed {
    /// Código de saída do legado.
    pub code: i32,
    /// Mensagem já pronta, com o prefixo `Error: ` quando o legado o usa.
    pub message: String,
}
impl Printed {
    /// Monta a falha com a mensagem final.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_message_drops_the_legacy_prefix() {
        let printed = Printed::new(1, "Error: pg_restore failed with exit code 1. Error: boom");
        assert_eq!(
            printed.bare_message(),
            "pg_restore failed with exit code 1. Error: boom"
        );

        // Mensagem sem o prefixo volta como veio.
        let sem_prefixo = Printed::new(1, "Invalid database name: 'x'");
        assert_eq!(sem_prefixo.bare_message(), "Invalid database name: 'x'");
    }
}

/// Desfecho de um comando do grupo `database`.
///
/// Unifica as duas formas de terminar do legado: imprimir a mensagem e devolver código (o caso
/// comum) ou deixar o erro subir como [`CoreError`] — cancelamento (130), prompt sem terminal (40)
/// e o resto pela tabela do `Program.cs`.
pub(crate) enum Failure {
    /// Imprime a mensagem e termina com o código indicado.
    Printed(Printed),
    /// Sobe como erro do núcleo.
    Fatal(CoreError),
}
impl Failure {
    /// Aplica o desfecho.
    pub fn resolve(self) -> Result<i32, CoreError> {
        match self {
            Self::Printed(Printed { code, message }) => {
                println!("{message}");
                Ok(code)
            }
            Self::Fatal(error) => Err(error),
        }
    }
}

impl From<CoreError> for Failure {
    fn from(error: CoreError) -> Self {
        Self::Fatal(error)
    }
}

/// Converte a falha do runner de processos: cancelamento sobe; o resto é erro geral (exit 1), como
/// no `Program.cs` para exceção não tratada.
impl From<mustel_core::process::ProcessError> for Failure {
    fn from(error: mustel_core::process::ProcessError) -> Self {
        match error {
            mustel_core::process::ProcessError::Cancelled => Self::Fatal(CoreError::Cancelled),
            other => Self::Printed(Printed::new(1, format!("Error: {other}"))),
        }
    }
}

/// Despacha `mustel database <subcomando>`.
pub async fn run(args: DatabaseArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    match args.command {
        DatabaseCommand::Backup(args) => backup::run(args, token).await,
        DatabaseCommand::Restore(args) => restore::run(args, token).await,
        DatabaseCommand::Pgpass(pgpass) => match pgpass.command {
            PgpassCommand::Add(args) => pgpass::add::run(args),
            PgpassCommand::List => pgpass::list::run(),
            PgpassCommand::Remove(args) => pgpass::remove::run(args),
        },
    }
}
