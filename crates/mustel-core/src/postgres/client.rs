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

/// Valor cru de uma célula: aceita **qualquer** tipo do servidor.
///
/// `tokio_postgres` só sabe ler os tipos que têm `FromSql`; para reproduzir o
/// `reader.GetValue(i)?.ToString()` do legado é preciso ver o tipo e os bytes de qualquer coluna —
/// inclusive `numeric`, `bytea`, arrays e tipos que o driver não mapeia. Este newtype é o escape:
/// aceita tudo e guarda os bytes binários, que [`Self::to_text`] decodifica conforme o tipo.
#[derive(Debug)]
pub struct RawValue {
    /// Tipo da coluna, como o servidor informou.
    type_: tokio_postgres::types::Type,
    /// Bytes na representação binária do protocolo.
    bytes: Vec<u8>,
}

impl<'a> tokio_postgres::types::FromSql<'a> for RawValue {
    fn from_sql(
        type_: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self {
            type_: type_.clone(),
            bytes: raw.to_vec(),
        })
    }

    fn accepts(_type_: &tokio_postgres::types::Type) -> bool {
        true
    }
}

impl RawValue {
    /// Texto da célula com a semântica do `ToString()` do .NET (ver `postgres::value`).
    pub fn to_text(&self) -> Result<String, CoreError> {
        Ok(crate::postgres::value::format_cell(&self.as_text_value()?))
    }

    /// Traduz o valor cru para o que a formatação entende.
    ///
    /// Casa pelo **nome** do tipo (`int4`, `jsonb`, ...) porque `Type` não é um enum destruturável no
    /// `tokio_postgres`; os arrays são reconhecidos pelo `kind()`.
    fn as_text_value(&self) -> Result<crate::postgres::value::TextValue<'_>, CoreError> {
        use crate::postgres::value::TextValue;
        use tokio_postgres::types::Kind;

        let bytes = &self.bytes;

        Ok(match self.type_.name() {
            "bool" => TextValue::Bool(bytes.first().copied().unwrap_or_default() != 0),
            "int2" => TextValue::Int(i16::from_be_bytes(pair(bytes)?).into()),
            "int4" => TextValue::Int(i32::from_be_bytes(quad(bytes)?).into()),
            "int8" => TextValue::Int(i64::from_be_bytes(octet(bytes)?)),
            "float4" => TextValue::Float32(f32::from_be_bytes(quad(bytes)?)),
            "float8" => TextValue::Float(f64::from_be_bytes(octet(bytes)?)),
            "numeric" => TextValue::Decimal(decimal_from_bytes(bytes)?),
            "text" | "varchar" | "bpchar" | "name" | "unknown" | "json" => {
                TextValue::Text(std::str::from_utf8(bytes).unwrap_or_default())
            }
            // `jsonb` tem um byte de versão na frente do texto canônico.
            "jsonb" => TextValue::Text(
                std::str::from_utf8(bytes.get(1..).unwrap_or_default()).unwrap_or_default(),
            ),
            "bytea" => TextValue::Bytes,
            "uuid" => TextValue::Uuid(sedecim(bytes)?),
            "date" => TextValue::Date(i32::from_be_bytes(quad(bytes)?)),
            "time" => TextValue::Time(i64::from_be_bytes(octet(bytes)?)),
            "timestamp" | "timestamptz" => TextValue::Timestamp(i64::from_be_bytes(octet(bytes)?)),
            "void" => TextValue::Null,
            _ => match self.type_.kind() {
                // Qualquer array vira o nome do tipo do array no .NET (o conteúdo não é usado).
                Kind::Array(element) => TextValue::Array(dotnet_array_name(element)),
                _ => {
                    return Err(CoreError::InvalidState(format!(
                        "tipo '{}' não é suportado pelo export do Mustel (o Npgsql do legado também \
                         recusaria)",
                        self.type_.name()
                    )));
                }
            },
        })
    }
}

/// Lê todos os valores de uma linha como texto, na ordem das colunas.
pub fn row_text(row: &tokio_postgres::Row) -> Result<Vec<String>, CoreError> {
    (0..row.len())
        .map(|index| {
            let raw: RawValue = row
                .try_get(index)
                .map_err(|error| CoreError::postgres(error.to_string()))?;
            raw.to_text()
        })
        .collect()
}

/// `numeric` → texto com a **escala** preservada (`0.00`), como o `decimal.ToString()` do .NET.
///
/// O `rust_decimal` implementa o `FromSql` do `numeric` (feature `db-postgres`) e o `Display` dele
/// mantém a escala, que é exatamente o que o CSV do oráculo mostra.
fn decimal_from_bytes(bytes: &[u8]) -> Result<String, CoreError> {
    use tokio_postgres::types::{FromSql, Type};

    let decimal = rust_decimal::Decimal::from_sql(&Type::NUMERIC, bytes)
        .map_err(|error| CoreError::InvalidState(format!("numeric inválido: {error}")))?;

    Ok(decimal.to_string())
}

/// Nome do tipo do array no .NET, por tipo do elemento.
///
/// É o que aparece no CSV do oráculo (`System.String[]` para `text[]`), porque o `ToString()` de um
/// array é o nome do tipo. Tipos sem tradução conhecida caem em `System.Object[]`.
fn dotnet_array_name(element: &tokio_postgres::types::Type) -> &'static str {
    use tokio_postgres::types::Type;

    match *element {
        Type::BOOL => "System.Boolean[]",
        Type::INT2 => "System.Int16[]",
        Type::INT4 => "System.Int32[]",
        Type::INT8 => "System.Int64[]",
        Type::FLOAT4 => "System.Single[]",
        Type::FLOAT8 => "System.Double[]",
        Type::NUMERIC => "System.Decimal[]",
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => "System.String[]",
        Type::UUID => "System.Guid[]",
        Type::DATE => "System.DateOnly[]",
        Type::TIME => "System.TimeOnly[]",
        Type::TIMESTAMP | Type::TIMESTAMPTZ => "System.DateTime[]",
        Type::BYTEA => "System.Byte[][]",
        _ => "System.Object[]",
    }
}

/// Quatro bytes exatamente, ou erro de protocolo.
fn quad(bytes: &[u8]) -> Result<[u8; 4], CoreError> {
    bytes
        .try_into()
        .map_err(|_| CoreError::InvalidState("valor com tamanho inesperado".to_string()))
}

/// Dois bytes exatamente.
fn pair(bytes: &[u8]) -> Result<[u8; 2], CoreError> {
    bytes
        .try_into()
        .map_err(|_| CoreError::InvalidState("valor com tamanho inesperado".to_string()))
}

/// Dezesseis bytes exatamente (para `uuid`).
fn sedecim(bytes: &[u8]) -> Result<[u8; 16], CoreError> {
    bytes
        .try_into()
        .map_err(|_| CoreError::InvalidState("valor com tamanho inesperado".to_string()))
}

/// Oito bytes exatamente.
fn octet(bytes: &[u8]) -> Result<[u8; 8], CoreError> {
    bytes
        .try_into()
        .map_err(|_| CoreError::InvalidState("valor com tamanho inesperado".to_string()))
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
