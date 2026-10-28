//! `mustel database restore` — o adaptador de terminal do caso de uso.
//!
//! A regra mora em [`mustel_core::usecases::database_restore`]; aqui ficam prompts, resolução da
//! senha e a **tabela tipo de falha → código de saída** do `restore`.
//!
//! A tabela difere da do `backup` de propósito: neste comando o legado manda **1** quando o binário
//! externo não é encontrado (no `backup` é **3**). É o tipo de quirk que (b) deixa explícito no
//! frontend, em vez de escondido no núcleo.

use mustel_core::config::ConfigStore;
use mustel_core::credentials::SystemKeyring;
use mustel_core::error::CoreError;
use mustel_core::postgres::locator::PostgresBinaryLocator;
use mustel_core::postgres::password::{self, PASSWORD_PROMPT};
use mustel_core::signal::CancellationToken;
use mustel_core::usecases::database::{DatabaseFailure, FailureKind};
use mustel_core::usecases::database_restore::{self, RestoreOptions};

use crate::cli::DatabaseRestoreArgs;
use crate::ui::{password_input, progress, prompts};

/// Código de saída de uma falha de comando do `restore` (tabela do legado).
///
/// Repare no `MissingBinary`: **1** aqui, contra **3** no `backup`.
fn exit_code(kind: FailureKind) -> i32 {
    match kind {
        FailureKind::Usage => 2,
        FailureKind::MissingBinary | FailureKind::OperationFailed => 1,
    }
}

/// Executa o comando.
pub async fn run(args: DatabaseRestoreArgs, token: &CancellationToken) -> Result<i32, CoreError> {
    let options = options_from(&args);

    let connection = match database_restore::validate(&options) {
        Ok(connection) => connection,
        Err(failure) => {
            println!("{}", failure.message);
            return Ok(exit_code(failure.kind));
        }
    };

    let stdin_password = if args.password_stdin {
        Some(password_input::read_from_stdin()?)
    } else {
        None
    };
    let password = password::resolve_password(
        stdin_password.as_deref().or(args.password.as_deref()),
        stored_password(&connection.host, &connection.port).as_deref(),
        || prompts::password(PASSWORD_PROMPT),
    )?;

    let sink = progress::TerminalSink::new();
    let outcome = database_restore::restore(
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
fn stored_password(host: &str, port: &str) -> Option<String> {
    let config = ConfigStore::with_default_path().ok()?;
    password::stored_password(&config, &SystemKeyring::new(), host, port)
}

/// Traduz as flags para a entrada do núcleo.
fn options_from(args: &DatabaseRestoreArgs) -> RestoreOptions {
    RestoreOptions {
        database: args.database.clone(),
        input_file: args.input_file.clone(),
        host: args.host.clone(),
        port: args.port.clone(),
        username: args.username.clone(),
        ssl_mode: args.ssl_mode.clone(),
        timeout: args.timeout,
        command_timeout: args.command_timeout,
        all: args.all,
        directory: args.directory.clone(),
        verbose: args.verbose,
        no_clean: args.no_clean,
        no_owner: args.no_owner,
        no_acl: args.no_acl,
        single_transaction: args.single_transaction,
        jobs: args.jobs,
        create_if_not_exists: args.create_if_not_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(database: Option<&str>) -> DatabaseRestoreArgs {
        DatabaseRestoreArgs {
            database: database.map(str::to_string),
            input_file: None,
            host: None,
            port: None,
            username: None,
            password: Some("senha".to_string()),
            password_stdin: false,
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            all: false,
            directory: None,
            verbose: false,
            no_clean: false,
            no_owner: false,
            no_acl: false,
            single_transaction: false,
            jobs: None,
            create_if_not_exists: false,
        }
    }

    #[test]
    /// CONTRACT: IF-081
    fn the_exit_codes_are_the_restore_ones() {
        assert_eq!(exit_code(FailureKind::Usage), 2);
        assert_eq!(
            exit_code(FailureKind::MissingBinary),
            1,
            "no restore o binário ausente sai com 1 (no backup é 3)"
        );
        assert_eq!(exit_code(FailureKind::OperationFailed), 1);
    }

    #[tokio::test]
    /// CONTRACT: IF-077
    async fn validation_failures_exit_with_2() {
        let exit = run(args(None), &CancellationToken::new())
            .await
            .expect("roda");
        assert_eq!(exit, 2, "sem banco e sem --all");
    }

    #[tokio::test]
    /// CONTRACT: IF-077
    async fn an_invalid_ssl_mode_is_refused_with_2() {
        let mut command = args(Some("mydb"));
        command.ssl_mode = Some("Allow".to_string());

        let exit = run(command, &CancellationToken::new()).await.expect("roda");
        assert_eq!(exit, 2);
    }

    #[test]
    /// CONTRACT: IF-082
    fn the_flags_are_translated_verbatim() {
        let mut command = args(Some("mydb"));
        command.jobs = Some(4);
        command.no_clean = true;
        command.create_if_not_exists = true;
        command.directory = Some("dumps".to_string());

        let options = options_from(&command);

        assert_eq!(options.database.as_deref(), Some("mydb"));
        assert_eq!(options.jobs, Some(4));
        assert!(options.no_clean);
        assert!(options.create_if_not_exists);
        assert_eq!(options.directory.as_deref(), Some("dumps"));
        assert!(!options.all, "`--all` só quando a flag vem");
    }
}
