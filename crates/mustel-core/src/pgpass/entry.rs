//! Uma linha do `pgpass.conf`: campos, identidade, leitura e escrita.
//!
//! Porte fiel de `legacy/FurLab.Core/Services/PgPassService.cs` (e de `PgPassEntry.cs`), inclusive
//! nas regras frouxas:
//!
//! - o separador é `:` e `\` escapa o caractere seguinte (então `\:` é um dois-pontos literal dentro
//!   do campo);
//! - campo de senha com **mais de um** `:` continua sendo senha: os campos 5+ são reunidos de volta
//!   (`string.Join(":", fields.Skip(4))`);
//! - linha com menos de 5 campos é **ignorada** (não é erro);
//! - a identidade de uma entrada são os 4 primeiros campos (a senha não participa).

/// Host usado quando o chamador não informa um (`FurLabConstants.DefaultHost`).
pub const DEFAULT_HOST: &str = "localhost";

/// Porta usada quando o chamador não informa uma (`FurLabConstants.DefaultPort`).
pub const DEFAULT_PORT: &str = "5432";

/// Banco usado quando o chamador não informa um (`PgPassEntry.Database`).
pub const DEFAULT_DATABASE: &str = "*";

/// Usuário usado quando o chamador não informa um (`PgPassEntry.Username`).
pub const DEFAULT_USERNAME: &str = "postgres";

/// Uma entrada do `pgpass.conf`.
///
/// Os campos `hostname`, `port`, `database` e `username` aceitam `*` como curinga. A senha é
/// gravada **em texto claro**, escapando apenas `\` e `:` — é o formato do arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgPassEntry {
    /// Hostname ou IP. Curinga: `*`.
    pub hostname: String,
    /// Porta TCP. Curinga: `*`.
    pub port: String,
    /// Banco. Curinga: `*`.
    pub database: String,
    /// Usuário. Curinga: `*`.
    pub username: String,
    /// Senha em texto claro (nunca vazia, nunca curinga).
    pub password: String,
}

impl Default for PgPassEntry {
    fn default() -> Self {
        Self {
            hostname: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT.to_string(),
            database: DEFAULT_DATABASE.to_string(),
            username: DEFAULT_USERNAME.to_string(),
            password: String::new(),
        }
    }
}

impl PgPassEntry {
    /// Entrada com todos os defaults do legado e a senha informada.
    pub fn with_password(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
            ..Self::default()
        }
    }

    /// Identidade da entrada: os quatro campos que decidem duplicata e remoção.
    pub fn identity_key(&self) -> (&str, &str, &str, &str) {
        (
            self.hostname.as_str(),
            self.port.as_str(),
            self.database.as_str(),
            self.username.as_str(),
        )
    }

    /// Identidade formatada para exibição (`host:porta:banco:usuário`).
    pub fn formatted_key(&self) -> String {
        let (host, port, database, username) = self.identity_key();
        format!("{host}:{port}:{database}:{username}")
    }

    /// Mesma identidade de `other` (comparação campo a campo, como o `IdentityKey` do legado).
    pub fn has_same_identity_as(&self, other: &Self) -> bool {
        self.identity_key() == other.identity_key()
    }

    /// Serializa para uma linha do arquivo, escapando a senha.
    pub fn serialize(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.hostname,
            self.port,
            self.database,
            self.username,
            escape_password(&self.password)
        )
    }
}

/// Lê uma linha do arquivo; `None` quando ela tem menos de 5 campos.
pub fn parse_line(line: &str) -> Option<PgPassEntry> {
    let fields = split_escaped(line);
    if fields.len() < 5 {
        return None;
    }

    Some(PgPassEntry {
        hostname: fields[0].clone(),
        port: fields[1].clone(),
        database: fields[2].clone(),
        username: fields[3].clone(),
        password: unescape_password(&fields[4..].join(":")),
    })
}

/// Divide a linha por `:` **não escapados**.
///
/// Uma `\` consome o caractere seguinte, que vai para o campo como veio (o desescape é feito depois,
/// só na senha). É o mesmo laço do legado.
fn split_escaped(line: &str) -> Vec<String> {
    let characters: Vec<char> = line.chars().collect();
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] == '\\' && index + 1 < characters.len() {
            current.push(characters[index]);
            current.push(characters[index + 1]);
            index += 2;
        } else if characters[index] == ':' {
            fields.push(std::mem::take(&mut current));
            index += 1;
        } else {
            current.push(characters[index]);
            index += 1;
        }
    }

    fields.push(current);
    fields
}

/// Escapa a senha para o arquivo: primeiro `\` → `\\`, depois `:` → `\:`.
///
/// A ordem importa: inverter produziria `\\:` a partir de `:`, e a releitura devolveria `\:`.
pub fn escape_password(password: &str) -> String {
    password.replace('\\', "\\\\").replace(':', "\\:")
}

/// Desfaz o escape da senha: `\:` → `:` e `\\` → `\`.
///
/// Uma `\` seguida de qualquer outro caractere é mantida como veio (o legado só trata os dois casos).
pub fn unescape_password(escaped: &str) -> String {
    let characters: Vec<char> = escaped.chars().collect();
    let mut result = String::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] == '\\' && index + 1 < characters.len() {
            match characters[index + 1] {
                ':' => {
                    result.push(':');
                    index += 2;
                }
                '\\' => {
                    result.push('\\');
                    index += 2;
                }
                _ => {
                    result.push(characters[index]);
                    index += 1;
                }
            }
        } else {
            result.push(characters[index]);
            index += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        host: &str,
        port: &str,
        database: &str,
        username: &str,
        password: &str,
    ) -> PgPassEntry {
        PgPassEntry {
            hostname: host.to_string(),
            port: port.to_string(),
            database: database.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    #[test]
    /// CONTRACT: IF-093
    fn defaults_match_the_legacy_record() {
        let entry = PgPassEntry::default();
        assert_eq!(entry.hostname, "localhost");
        assert_eq!(entry.port, "5432");
        assert_eq!(entry.database, "*");
        assert_eq!(entry.username, "postgres");
        assert!(entry.password.is_empty());
    }

    #[test]
    fn serialize_writes_the_five_fields_in_order() {
        let entry = entry("localhost", "5432", "meu_banco", "postgres", "senha123");
        assert_eq!(
            entry.serialize(),
            "localhost:5432:meu_banco:postgres:senha123"
        );
    }

    #[test]
    fn wildcards_are_written_literally() {
        let entry = entry("*", "*", "*", "*", "senha123");
        assert_eq!(entry.serialize(), "*:*:*:*:senha123");
    }

    #[test]
    /// CONTRACT: IF-095
    fn serialize_escapes_backslash_before_colon() {
        // A ordem do legado: `\` primeiro, depois `:`. `a\b:c` vira `a\\b\:c`.
        assert_eq!(
            escape_password("senha:com:dois:pontos"),
            "senha\\:com\\:dois\\:pontos"
        );
        assert_eq!(escape_password("senha\\invertida"), "senha\\\\invertida");
    }

    #[test]
    /// CONTRACT: IF-095
    fn escape_and_unescape_are_inverse() {
        for password in ["simples", "com:dois", "com\\barra", "\\:misturado", "fim\\"] {
            assert_eq!(
                unescape_password(&escape_password(password)),
                password,
                "round-trip falhou para {password:?}"
            );
        }
    }

    #[test]
    fn parse_reads_the_five_fields_and_unescapes_the_password() {
        let parsed = parse_line(r"localhost:5432:meu_banco:postgres:senha\:com\:dois")
            .expect("linha válida");
        assert_eq!(parsed.hostname, "localhost");
        assert_eq!(parsed.port, "5432");
        assert_eq!(parsed.database, "meu_banco");
        assert_eq!(parsed.username, "postgres");
        assert_eq!(parsed.password, "senha:com:dois");
    }

    #[test]
    fn parse_joins_extra_fields_back_into_the_password() {
        // Senha com `:` gravada sem escape: o legado relê os campos 5+ como senha.
        let parsed = parse_line("h:1:d:u:a:b").expect("linha válida");
        assert_eq!(parsed.password, "a:b");
    }

    #[test]
    fn parse_requires_five_fields() {
        assert!(parse_line("a:b:c:d").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("# comentário").is_none());
        assert!(parse_line("a:b:c:d:").is_some());
    }

    #[test]
    fn identity_ignores_the_password() {
        let first = entry("localhost", "5432", "mydb", "postgres", "uma");
        let second = entry("localhost", "5432", "mydb", "postgres", "outra");
        assert!(first.has_same_identity_as(&second));
        assert_eq!(first.formatted_key(), "localhost:5432:mydb:postgres");

        let other_host = entry("remotehost", "5432", "mydb", "postgres", "uma");
        assert!(!first.has_same_identity_as(&other_host));
        assert_eq!(other_host.formatted_key(), "remotehost:5432:mydb:postgres");
    }
}
