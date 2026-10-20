//! `mustel database backup` — o adaptador de terminal do caso de uso.
//!
//! A regra (validação, argumentos do `pg_dump`, laço do `--all`, criação de diretório) mora em
//! [`mustel_core::usecases::database_backup`]; aqui ficam os prompts, a resolução da senha e a
//! **tabela tipo de falha → código de saída**.
//!
//! Um detalhe de contrato que continua aqui e é o motivo de a tabela existir: o legado usa códigos do
//! **corpo** do comando (não da tabela do `Program.cs`), e eles mudam entre os dois comandos — no
//! `backup` binário ausente sai **3**; no `restore`, **1**.

use mustel_core::config::ConfigStore;
use mustel_core::credentials::SystemKeyring;
use mustel_core::error::CoreError;
use mustel_core::postgres::locator::PostgresBinaryLocator;
use mustel_core::postgres::password::{self, PASSWORD_PROMPT};
use mustel_core::signal::CancellationToken;
use mustel_core::usecases::database::{DatabaseFailure, FailureKind};
use mustel_core::usecases::database_backup::{self, BackupOptions};

use crate::cli::DatabaseBackupArgs;
use crate::ui::{progress, prompts};

/// Código de saída de uma falha de comando do `backup` (tabela do legado).
fn exit_code(kind: FailureKind) -> i32 {
    match kind {
        FailureKind::Usage => 2,
        FailureKind::MissingBinary => 3,
        FailureKind::OperationFailed => 1,
    }
}

/// Executa o comando.
pub async fn run(args: DatabaseBackupArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    let options = options_from(&args);

    // A validação vem **antes** da senha, como no legado: argumento inválido não chega a perguntar.
    let connection = match database_backup::validate(&options) {
        Ok(connection) => connection,
        Err(failure) => {
            println!("{}", failure.message);
            return Ok(exit_code(failure.kind));
        }
    };

    let password = password::resolve_password(
        args.password.as_deref(),
        stored_password(&connection.host, &connection.port).as_deref(),
        || prompts::password(PASSWORD_PROMPT),
    )?;

    // O `--all` não tem progresso visível no legado (o resumo é `LogInformation`, filtrado) e o
    // caminho de sucesso não imprime nada; o sink só formata os avisos do laço.
    let sink = progress::TerminalSink::new();
    let outcome = database_backup::backup(
        &options,
        &connection,
        &password,
        &PostgresBinaryLocator::new(),
        &sink,
        token,
    )
    .await;
    sink.finish();

    resolve(outcome)
}

/// Aplica o desfecho: imprime a mensagem e devolve o código, ou sobe o erro do núcleo.
fn resolve(outcome: Result<(), DatabaseFailure>) -> Result<i32, CoreError> {
    match outcome {
        Ok(()) => Ok(0),
        Err(DatabaseFailure::Fatal(error)) => Err(error),
        Err(DatabaseFailure::Command(failure)) => {
            println!("{}", failure.message);
            Ok(exit_code(failure.kind))
        }
    }
}

/// Senha guardada do servidor que casa com `host:porta` (config + cofre reais).
///
/// Sem diretório de configuração ou sem servidor correspondente não há senha guardada — o legado
/// engolia esses casos num `try`/`catch` vazio pelo mesmo motivo.
fn stored_password(host: &str, port: &str) -> Option<String> {
    let config = ConfigStore::with_default_path().ok()?;
    password::stored_password(&config, &SystemKeyring::new(), host, port)
}

/// Traduz as flags para a entrada do núcleo.
fn options_from(args: &DatabaseBackupArgs) -> BackupOptions {
    BackupOptions {
        database: args.database.clone(),
        host: args.host.clone(),
        port: args.port.clone(),
        username: args.username.clone(),
        all: args.all,
        output: args.output.clone(),
        format: args.format.clone(),
        verbose: args.verbose,
        exclude: args.exclude.clone(),
        exclude_table_data: args.exclude_table_data.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(database: Option<&str>) -> DatabaseBackupArgs {
        DatabaseBackupArgs {
            database: database.map(str::to_string),
            host: None,
            port: None,
            username: None,
            password: Some("senha".to_string()),
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            all: false,
            exclude: Vec::new(),
            exclude_table_data: Vec::new(),
            verbose: false,
            output: None,
            format: None,
        }
    }

    #[tokio::test]
    async fn validation_failures_exit_with_2() {
        for database in [None, Some("")] {
            let exit = run(args(database), &CancellationToken::new())
                .await
                .expect("roda");
            assert_eq!(exit, 2, "banco {database:?}");
        }
    }

    #[tokio::test]
    async fn a_missing_pg_dump_exits_with_3() {
        // Localizador real: nesta máquina não há `pg_dump` no PATH do teste, mas se houver o
        // argumento é válido e o comando só tentaria o dump; o que importa aqui é a tabela de
        // código para `MissingBinary`.
        assert_eq!(exit_code(FailureKind::MissingBinary), 3);
        assert_eq!(exit_code(FailureKind::Usage), 2);
        assert_eq!(exit_code(FailureKind::OperationFailed), 1);
    }

    #[test]
    fn the_flags_are_translated_verbatim() {
        let mut command = args(Some("mydb"));
        command.host = Some("127.0.0.1".to_string());
        command.port = Some("55432".to_string());
        command.format = Some("p".to_string());
        command.exclude = vec!["log*".to_string()];

        let options = options_from(&command);

        assert_eq!(options.database.as_deref(), Some("mydb"));
        assert_eq!(options.host.as_deref(), Some("127.0.0.1"));
        assert_eq!(options.port.as_deref(), Some("55432"));
        assert_eq!(options.format.as_deref(), Some("p"));
        assert_eq!(options.exclude, vec!["log*".to_string()]);
    }
}
