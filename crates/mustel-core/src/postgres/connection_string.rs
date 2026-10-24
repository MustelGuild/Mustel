//! Leitura de connection string no estilo Npgsql (`Chave=Valor;Chave=Valor`).
//!
//! Uma implementação só para o CLI (`--npgsql-connection-string`) e para a janela (query de seleção
//! de bases). O CLI **descarta** a senha por paridade com o legado; a janela usa. Essa diferença é do
//! chamador, não do parser — é por isso que o parser lê tudo e quem decide o que aproveitar é quem
//! chama.

/// O que uma connection string Npgsql pode trazer de útil.
///
/// Chaves desconhecidas (pooling, keepalive, ...) são ignoradas: não há pool implementado.
#[derive(Default, PartialEq, Eq)]
pub struct ParsedConnectionString {
    /// Host (`Host`, `Server`, `Data Source`).
    pub host: Option<String>,
    /// Porta (`Port`).
    pub port: Option<u16>,
    /// Banco (`Database`, `Initial Catalog`).
    pub database: Option<String>,
    /// Usuário (`Username`, `User ID`, `UserId`, `User`).
    pub user: Option<String>,
    /// Modo SSL (`SslMode`, `SSL Mode`).
    pub ssl_mode: Option<String>,
    /// Timeout de conexão (`Timeout`).
    pub timeout: Option<u32>,
    /// Senha (`Password`, `Pwd`).
    ///
    /// **Nunca** é gravada, logada ou impressa. Quem não deve usá-la (o CLI) simplesmente ignora o
    /// campo.
    pub password: Option<String>,
}

/// `Debug` sem a senha: a struct pode cair num log por acidente, e credencial não vaza em `{:?}`.
impl core::fmt::Debug for ParsedConnectionString {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ParsedConnectionString")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("ssl_mode", &self.ssl_mode)
            .field("timeout", &self.timeout)
            .field("password", &self.password.as_ref().map(|_| "<oculta>"))
            .finish()
    }
}

/// Lê uma connection string no estilo Npgsql.
///
pub fn parse(connection_string: &str) -> ParsedConnectionString {
    let mut parsed = ParsedConnectionString::default();

    for pair in connection_string.split(';') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "host" | "server" | "data source" => parsed.host = Some(value.to_string()),
            "port" => parsed.port = value.parse().ok(),
            "database" | "initial catalog" => parsed.database = Some(value.to_string()),
            "username" | "user id" | "userid" | "user" => parsed.user = Some(value.to_string()),
            "sslmode" | "ssl mode" => parsed.ssl_mode = Some(value.to_string()),
            "timeout" => parsed.timeout = value.parse().ok(),
            "password" | "pwd" => parsed.password = Some(value.to_string()),
            _ => {}
        }
    }

    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lê as chaves que o produto usa, inclusive a senha.
    /// CONTRACT: IF-223
    #[test]
    fn the_connection_string_is_parsed_like_npgsql() {
        let parsed = parse(
            "Host=127.0.0.1;Port=55432;Database=mustel_test;Username=postgres;Password=dev;\
             SslMode=Disable;Timeout=15;Pooling=false",
        );

        assert_eq!(
            parsed,
            ParsedConnectionString {
                host: Some("127.0.0.1".to_string()),
                port: Some(55432),
                database: Some("mustel_test".to_string()),
                user: Some("postgres".to_string()),
                ssl_mode: Some("Disable".to_string()),
                timeout: Some(15),
                password: Some("dev".to_string()),
            }
        );
    }

    /// Aceita os apelidos que o Npgsql também aceita e ignora chaves desconhecidas.
    #[test]
    fn aliases_and_unknown_keys_are_handled() {
        let parsed =
            parse("Data Source=db;Initial Catalog=app;User Id=ana;Pwd=segredo;Pooling=true");

        assert_eq!(parsed.host.as_deref(), Some("db"));
        assert_eq!(parsed.database.as_deref(), Some("app"));
        assert_eq!(parsed.user.as_deref(), Some("ana"));
        assert_eq!(parsed.password.as_deref(), Some("segredo"));
        assert_eq!(parsed.port, None, "sem porta, quem chama decide o padrão");
    }

    /// String vazia ou inválida não explode: devolve tudo vazio.
    #[test]
    fn rubbish_yields_empty_fields() {
        assert_eq!(parse(""), ParsedConnectionString::default());
        assert_eq!(parse("sem igual"), ParsedConnectionString::default());
    }

    /// A senha não aparece em `Debug`.
    ///
    /// O parser é usado em caminho onde a struct pode ser despejada num log; um `{:?}` acidental não
    /// pode vazar credencial.
    /// CONTRACT: IF-223
    #[test]
    fn debug_never_shows_the_password() {
        let parsed = parse("Host=db;Password=segredo");
        let text = format!("{parsed:?}");

        assert!(!text.contains("segredo"), "a senha vazou no Debug: {text}");
        assert!(text.contains("db"), "o resto continua visível: {text}");
    }
}
