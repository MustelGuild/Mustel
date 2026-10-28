//! Fronteira fina com o driver `tokio-postgres`.
//!
//! `Disable` abre sem TLS. `Prefer` e `Require` usam `tokio-postgres-rustls` com os certificados
//! nativos do sistema; o verificador padrão do rustls valida cadeia **e** hostname. O driver já
//! implementa o fallback do `Prefer` somente quando o servidor não suporta TLS; erro de certificado
//! não vira texto puro. Cair para texto puro é o que o `Prefer` pede, então a conexão não avisa:
//! quem quer TLS garantido escolhe `Require`.

use std::pin::Pin;
use std::time::Duration;

use futures_util::Stream;

use crate::error::CoreError;
use crate::postgres::settings::ConnectionSettings;
use crate::postgres::tls::{SslMode, tls_connector};

/// Abre uma conexão e devolve o cliente pronto para consultas.
///
/// Erros de conexão viram [`CoreError::Postgres`] **com** `sqlstate` quando o servidor informou um (o
/// oráculo usa `10` nesse caso e `11` quando não há SQLSTATE).
pub async fn connect(settings: &ConnectionSettings) -> Result<tokio_postgres::Client, CoreError> {
    let mut config = settings.to_pg_config();
    config.ssl_mode(driver_ssl_mode(settings.ssl_mode));

    match settings.ssl_mode {
        SslMode::Disable => {
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
        SslMode::Prefer | SslMode::Require => {
            let tls = tls_connector()?;
            let (client, connection) = config.connect(tls).await.map_err(map_error)?;

            tokio::spawn(async move {
                let _ = connection.await;
            });

            Ok(client)
        }
    }
}

/// Traduz o enum do produto para o enum do driver.
fn driver_ssl_mode(mode: SslMode) -> tokio_postgres::config::SslMode {
    match mode {
        SslMode::Disable => tokio_postgres::config::SslMode::Disable,
        SslMode::Prefer => tokio_postgres::config::SslMode::Prefer,
        SslMode::Require => tokio_postgres::config::SslMode::Require,
    }
}

/// Uma base do servidor, como o catálogo `pg_database` a descreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDatabase {
    /// Nome da base.
    pub name: String,
    /// É um template (`template0`, `template1`)?
    pub template: bool,
    /// Aceita conexão (`datallowconn`)? `template0` não aceita.
    pub allow_connections: bool,
}

impl CatalogDatabase {
    /// Uma base comum: não é template e aceita conexão.
    ///
    /// Atalho para testes e para quem só quer representar um nome utilizável.
    pub fn usable(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: false,
            allow_connections: true,
        }
    }

    /// Serve como alvo de execução (não é template e aceita conexão)?
    pub fn is_usable(&self) -> bool {
        !self.template && self.allow_connections
    }
}

impl From<&str> for CatalogDatabase {
    fn from(name: &str) -> Self {
        Self::usable(name)
    }
}

impl From<String> for CatalogDatabase {
    fn from(name: String) -> Self {
        Self::usable(name)
    }
}

/// Lista **todas** as bases do catálogo, inclusive os templates.
///
/// Separada de [`list_databases`] (que o `settings db-servers test` usa, com a redação do legado)
/// e de [`list_databases_for_discovery`] (a lista de **execução**, sem template nem
/// `datallowconn = false`). Esta serve para a tela **contar e mostrar** o que o servidor tem: se a
/// descoberta escondesse o `template0`, qualquer seletor que o nomeasse mudaria a contagem de
/// bases do servidor.
pub async fn list_catalog_databases(
    client: &tokio_postgres::Client,
) -> Result<Vec<CatalogDatabase>, CoreError> {
    let rows = client
        .query(
            "SELECT datname, datistemplate, datallowconn FROM pg_database ORDER BY datname;",
            &[],
        )
        .await
        .map_err(map_error)?;

    Ok(rows
        .iter()
        .map(|row| CatalogDatabase {
            name: row.get::<_, String>(0),
            template: row.get::<_, bool>(1),
            allow_connections: row.get::<_, bool>(2),
        })
        .collect())
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
///
/// O `Option` é obrigatório: `NULL` não chega ao `from_sql` (o driver devolve `None`), e sem ele o
/// `try_get` falha com "error deserializing column".
pub fn row_text(row: &tokio_postgres::Row) -> Result<Vec<String>, CoreError> {
    (0..row.len())
        .map(|index| {
            let raw: Option<RawValue> = row
                .try_get(index)
                .map_err(|error| CoreError::postgres(error.to_string()))?;

            match raw {
                Some(raw) => raw.to_text(),
                None => Ok(crate::postgres::value::format_cell(
                    &crate::postgres::value::TextValue::Null,
                )),
            }
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

/// Consulta **em andamento**: as linhas chegam uma a uma, já formatadas para o CSV.
///
/// É o equivalente do `NpgsqlDataReader` do legado, e existe para o `query run` escrever o CSV
/// conforme lê (critério de memória limitada do T34) em vez de materializar o resultado: o
/// `RowStream` só avança quando alguém o consome, então o back-pressure do canal do executor chega
/// até o servidor.
///
/// Quem consome conta quantas linhas passaram por aqui — é de onde saem o `RowCount` do log e o
/// `Total rows` do resumo (o legado conta do mesmo jeito, só que sobre o `List<Dictionary>` que ele
/// materializa).
pub struct QueryStream {
    /// Mantém a conexão viva enquanto o fluxo é lido: sem o cliente a task da conexão cai e o fluxo
    /// termina. O campo não é lido de propósito.
    _client: tokio_postgres::Client,
    /// Colunas do resultado, conhecidas já no `prepare` (o legado as lê do `FieldCount`/`GetName`).
    columns: Vec<String>,
    /// Linhas do driver, uma por vez.
    rows: Pin<Box<tokio_postgres::RowStream>>,
    /// Timeout de comando, aplicado **por leitura** (ver [`Self::next_row`]).
    command_timeout: Duration,
}

impl QueryStream {
    /// Prepara e dispara a consulta; devolve o fluxo pronto para ser consumido.
    ///
    /// O `prepare` (que valida a consulta e devolve as colunas) está sob o mesmo timeout de comando,
    /// como o `ExecuteReaderAsync` do legado: uma consulta que nunca responde estoura aqui.
    pub async fn open(
        settings: &ConnectionSettings,
        sql: &str,
        command_timeout: Duration,
    ) -> Result<Self, CoreError> {
        let client = connect(settings).await?;

        let statement = match tokio::time::timeout(command_timeout, client.prepare(sql)).await {
            Ok(result) => result.map_err(map_error)?,
            Err(_) => return Err(timeout_error(command_timeout)),
        };

        let columns = statement
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect();

        // Sem parâmetros: a consulta vem inteira do `-c`/`-i`, como no legado (que também não passa
        // parâmetro nenhum).
        //
        // O `await` está sob o timeout porque o servidor só manda a descrição das colunas quando
        // **começa** a produzir o resultado: uma consulta que trava antes disso trava aqui (o
        // `ExecuteReaderAsync` do legado espera o mesmo e está sob o `CommandTimeout`).
        let rows = match tokio::time::timeout(
            command_timeout,
            client.query_raw(
                &statement,
                std::iter::empty::<&(dyn tokio_postgres::types::ToSql + Sync)>(),
            ),
        )
        .await
        {
            Ok(result) => result.map_err(map_error)?,
            Err(_) => return Err(timeout_error(command_timeout)),
        };

        Ok(Self {
            _client: client,
            columns,
            rows: Box::pin(rows),
            command_timeout,
        })
    }

    /// Colunas do resultado, na ordem do `SELECT`.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Próxima linha, com os valores já no texto do `ToString()` do .NET (ver
    /// [`crate::postgres::value`]); `None` quando o resultado acabou.
    ///
    /// O timeout de comando vale **por leitura** (e na abertura), como o `CommandTimeout` do Npgsql
    /// (que é renovado a cada `ReadAsync`), e não para o resultado inteiro: um export longo que
    /// devolve linha a linha não é interrompido só por demorar, mas uma consulta que trava no meio
    /// estoura.
    pub async fn next_row(&mut self) -> Result<Option<Vec<String>>, CoreError> {
        let next = std::future::poll_fn(|context| self.rows.as_mut().poll_next(context));

        match tokio::time::timeout(self.command_timeout, next).await {
            Ok(Some(Ok(row))) => row_text(&row).map(Some),
            Ok(Some(Err(error))) => Err(map_error(error)),
            Ok(None) => Ok(None),
            Err(_) => Err(timeout_error(self.command_timeout)),
        }
    }
}

/// Erro de timeout do comando, no formato que o log e o `_erros.csv` mostram.
fn timeout_error(command_timeout: Duration) -> CoreError {
    CoreError::Timeout(format!("query exceeded {command_timeout:?}"))
}

/// Lista os bancos para a **descoberta** do `query run`, com a query do legado.
///
/// Diferente de [`list_databases`] em dois pontos que o legado também tem: filtra
/// `datallowconn = true` e **não** ordena (a ordem é a que o servidor devolver).
pub async fn list_databases_for_discovery(
    settings: &ConnectionSettings,
) -> Result<Vec<String>, CoreError> {
    let client = connect(settings).await?;

    let rows = client
        .query(
            "SELECT datname FROM pg_database WHERE datistemplate = false AND datallowconn = true",
            &[],
        )
        .await
        .map_err(map_error)?;

    Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
}

/// Classifica o erro do driver no erro do núcleo.
///
/// A mensagem do erro **do Postgres** sai no formato do Npgsql, medido no oráculo: `SQLSTATE: texto`
/// mais os campos extras que o servidor mandar (ver [`postgres_message`]). Como o texto vem do
/// servidor, os dois lados ficam iguais quando o erro nasce lá; sem SQLSTATE (falha de conexão, por
/// exemplo) não há prefixo e a redação é a do driver — é o que a divergência 14 registra.
pub fn map_error(error: tokio_postgres::Error) -> CoreError {
    let sqlstate = error.code().map(|code| code.code().to_string());
    let message = match (error.as_db_error(), &sqlstate) {
        (Some(db_error), Some(state)) => {
            let position = db_error.position().map(|position| match position {
                tokio_postgres::error::ErrorPosition::Original(position)
                | tokio_postgres::error::ErrorPosition::Internal { position, .. } => *position,
            });

            postgres_message(
                state,
                db_error.message(),
                db_error.detail().is_some(),
                position,
            )
        }
        (Some(db_error), None) => db_error.message().to_string(),
        (None, _) => error.to_string(),
    };

    CoreError::Postgres { sqlstate, message }
}

/// Texto que o Npgsql põe no lugar do detalhe quando o `Include Error Detail` está desligado.
///
/// É o **default** do Npgsql, e o legado nunca o liga (a connection string dele não tem essa chave),
/// então é esta sentença fixa que aparece no log e no `_erros.csv` — não o detalhe do servidor.
const REDACTED_DETAIL: &str = "Detail redacted as it may contain sensitive data. Specify 'Include Error Detail' in the connection string to include this information.";

/// Mensagem de um erro do Postgres no formato do Npgsql, medido no oráculo.
///
/// O que foi medido, byte a byte (T32/T36):
///
/// - `SQLSTATE: texto`;
/// - `DETAIL` (redigido, ver [`REDACTED_DETAIL`]) e `POSITION`, **nesta ordem**, cada um numa linha
///   própria depois de uma linha em branco (`\r\n\r\n`);
/// - o `HINT` **não** entra: medido num `42883` em que o servidor manda o hint e o Npgsql o omite.
fn postgres_message(sqlstate: &str, text: &str, has_detail: bool, position: Option<u32>) -> String {
    let mut message = format!("{sqlstate}: {text}");

    if has_detail {
        message.push_str("\r\n\r\nDETAIL: ");
        message.push_str(REDACTED_DETAIL);
    }

    if let Some(position) = position {
        message.push_str(&format!("\r\n\r\nPOSITION: {position}"));
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres::tls::SslMode;

    #[test]
    fn every_ssl_mode_maps_to_the_driver_mode() {
        assert_eq!(
            driver_ssl_mode(SslMode::Disable),
            tokio_postgres::config::SslMode::Disable
        );
        assert_eq!(
            driver_ssl_mode(SslMode::Prefer),
            tokio_postgres::config::SslMode::Prefer
        );
        assert_eq!(
            driver_ssl_mode(SslMode::Require),
            tokio_postgres::config::SslMode::Require
        );
    }

    #[test]
    /// CONTRACT: IF-038
    fn a_plain_postgres_error_is_the_sqlstate_and_the_text() {
        assert_eq!(
            postgres_message(
                "3D000",
                "database \"nao_existe\" does not exist",
                false,
                None
            ),
            "3D000: database \"nao_existe\" does not exist"
        );
    }

    #[test]
    /// CONTRACT: IF-038
    fn the_position_comes_after_a_blank_line_like_npgsql_writes_it() {
        // Medido no oráculo (`SELECT nao_existe FROM clientes`): os bytes do `_erros.csv` são
        // `"42703: column ""nao_existe"" does not exist\r\n\r\nPOSITION: 8"`.
        assert_eq!(
            postgres_message(
                "42703",
                "column \"nao_existe\" does not exist",
                false,
                Some(8)
            ),
            "42703: column \"nao_existe\" does not exist\r\n\r\nPOSITION: 8"
        );
    }

    #[test]
    /// CONTRACT: IF-038
    fn the_detail_is_the_redacted_sentence_npgsql_shows_by_default() {
        // Medido no oráculo (`INSERT INTO clientes SELECT * FROM clientes`, que falha e não muda
        // nada): o `DETAIL` do servidor **não** aparece — o Npgsql o redige por default.
        assert_eq!(
            postgres_message(
                "23505",
                "duplicate key value violates unique constraint \"clientes_pkey\"",
                true,
                None
            ),
            "23505: duplicate key value violates unique constraint \"clientes_pkey\"\r\n\r\n\
             DETAIL: Detail redacted as it may contain sensitive data. Specify 'Include Error \
             Detail' in the connection string to include this information."
        );
    }

    #[test]
    fn the_hint_is_left_out_and_the_detail_comes_before_the_position() {
        // O servidor manda `HINT` no `42883` e a mensagem do oráculo **não** o mostra (medido).
        assert_eq!(
            postgres_message(
                "42883",
                "function to_date(unknown) does not exist",
                false,
                Some(8)
            ),
            "42883: function to_date(unknown) does not exist\r\n\r\nPOSITION: 8"
        );

        // Com os dois campos, a ordem é a do Npgsql: `DETAIL` e depois `POSITION`.
        let both = postgres_message("23505", "duplicado", true, Some(3));
        assert!(
            both.find("DETAIL") < both.find("POSITION"),
            "o detalhe vem antes da posição: {both}"
        );
    }
}
