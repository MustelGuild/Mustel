//! Fronteira fina com o driver `tokio-postgres`.
//!
//! Estado (T19, parcial): a conexão **sem TLS** está implementada e é exercitada pelos testes de
//! integração; os modos que exigem TLS devolvem erro explícito em vez de silenciosamente cair para
//! texto puro. Ligar TLS (rustls + certificados do sistema) e separar `VerifyCA` de `VerifyFull` é o
//! que falta — ver `docs/rust-postgres-spike.md`.

use crate::error::CoreError;
use crate::postgres::settings::ConnectionSettings;

/// Abre uma conexão e devolve o cliente pronto para consultas.
///
/// Erros de conexão viram [`CoreError::Postgres`] **com** `sqlstate` quando o servidor informou um (o
/// oráculo usa `10` nesse caso e `11` quando não há SQLSTATE).
pub async fn connect(settings: &ConnectionSettings) -> Result<tokio_postgres::Client, CoreError> {
    if settings.ssl_mode.requires_tls() {
        return Err(CoreError::InvalidState(format!(
            "sslMode '{}' ainda não é suportado pelo Mustel; use Prefer, Allow ou Disable enquanto a \
             TLS não é ligada (ver docs/rust-postgres-spike.md)",
            settings.ssl_mode
        )));
    }

    let config = settings.to_pg_config();
    let (client, connection) = config
        .connect(tokio_postgres::NoTls)
        .await
        .map_err(map_error)?;

    // A conexão precisa de uma task própria; quando ela cai, `client.is_closed()` fica true.
    tokio::spawn(async move {
        let _ = connection.await;
    });

    Ok(client)
}

/// Lista os bancos acessíveis, com a **mesma query** que o legado usa.
///
/// O FurLab faz isso executando o `psql` externo; aqui a consulta vai pelo driver (D12), o que
/// dispensa o `psql` instalado no caminho do `settings db-servers test`. A ordem é a mesma
/// (`ORDER BY datname`) para que a mensagem `Databases found: ...` seja comparável.
pub async fn list_databases(client: &tokio_postgres::Client) -> Result<Vec<String>, CoreError> {
    let rows = client
        .query(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname;",
            &[],
        )
        .await
        .map_err(map_error)?;

    Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
}

/// Cria o banco `database` se ele ainda **não** existir; devolve `true` quando criou.
///
/// Adição do Mustel (não existe no legado): o `CreateDatabaseIfNeeded` do FurLab só criava quando o
/// `psql` saísse com código **3**, o que nunca acontece — um `SELECT` sem linhas sai com **0** —,
/// então restaurar para um banco inexistente sempre falhava. Este é o comportamento que a flag
/// `--create-if-not-exists` liga, e ele vai pelo **driver** (D12): nem o `psql` nem o parsing de
/// código de saída entram na conta.
///
/// Conecta no banco de manutenção (`postgres`), como o `psql -d postgres` do legado, porque o banco
/// alvo é justamente o que pode não existir. O nome vai **citado** com aspas dobradas, como no
/// `CREATE DATABASE \"...\"` do original.
pub async fn create_database_if_missing(
    settings: &ConnectionSettings,
    database: &str,
) -> Result<bool, CoreError> {
    let maintenance = ConnectionSettings {
        database: Some("postgres".to_string()),
        ..settings.clone()
    };
    let client = connect(&maintenance).await?;

    let existing = client
        .query("SELECT 1 FROM pg_database WHERE datname = $1", &[&database])
        .await
        .map_err(map_error)?;
    if !existing.is_empty() {
        return Ok(false);
    }

    // Nome citado como identificador: a aspa dobrada é o escape do SQL, como no legado.
    let statement = format!("CREATE DATABASE \"{}\"", database.replace('"', "\"\""));
    client.batch_execute(&statement).await.map_err(map_error)?;

    Ok(true)
}

/// Classifica o erro do driver no erro do núcleo.
fn map_error(error: tokio_postgres::Error) -> CoreError {
    let sqlstate = error.code().map(|code| code.code().to_string());
    let message = error
        .as_db_error()
        .map(|db_error| format!("{}: {}", db_error.severity(), db_error.message()))
        .unwrap_or_else(|| error.to_string());

    CoreError::Postgres { sqlstate, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres::tls::SslMode;

    #[tokio::test]
    async fn tls_modes_fail_clearly_instead_of_silently_downgrading() {
        let settings = ConnectionSettings {
            host: "localhost".to_string(),
            user: "postgres".to_string(),
            ssl_mode: SslMode::Require,
            ..ConnectionSettings::default()
        };

        let error = connect(&settings).await.expect_err("must fail");
        let message = error.to_string();
        assert!(message.contains("sslMode 'Require'"), "{message}");
        assert!(message.contains("docs/rust-postgres-spike.md"), "{message}");
    }
}
