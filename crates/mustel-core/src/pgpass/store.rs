//! Leitura e escrita do arquivo `pgpass.conf`.
//!
//! Porte fiel de `legacy/FurLab.Core/Services/PgPassService.cs`. O que o arquivo tem de peculiar e
//! que a implementação **preserva**:
//!
//! - `add` **anexa** a linha (não reescreve o arquivo): se o arquivo não terminar em quebra de linha,
//!   a entrada nova fica colada na última — foi assim que o legado gravou, e é assim que se lê;
//! - quebra de linha é a do SO (CRLF no Windows), como `Environment.NewLine`;
//! - `remove` **reescreve** o arquivo inteiro com as linhas restantes, sempre terminando com quebra de
//!   linha (semântica de `File.WriteAllLines`); comentários e linhas em branco são preservados;
//! - `remove` de entrada inexistente **não** escreve nada (o arquivo fica byte a byte como estava).
//!
//! Duas diferenças deliberadas, registradas em `fixtures/known-divergences.md`:
//!
//! 1. no Unix o arquivo fica com modo `0600` (o legado usa o padrão do processo) — é um arquivo de
//!    senhas em texto claro e é o modo que o `libpq` exige;
//! 2. a leitura aceita bytes inválidos (UTF-8 lossy) e descarta BOM, como o `StreamReader` do .NET;
//!    arquivo em UTF-16 é lido como UTF-8 lossy (não há suporte a UTF-16).

use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use super::entry::{PgPassEntry, parse_line};
use super::paths::pgpass_file_path;

/// Falhas que o usuário vê como mensagem, não como erro de I/O cru.
///
/// O legado distingue `UnauthorizedAccessException` de `IOException` e devolve `1` nos dois casos;
/// as mensagens abaixo são o texto observado (sem o prefixo `Error: `, que o CLI acrescenta).
#[derive(Debug, thiserror::Error)]
pub enum PgPassError {
    /// O SO não expõe pasta de dados de aplicação e `MUSTEL_PGPASS_FILE` não foi definida.
    #[error("could not determine the pgpass.conf path for this platform")]
    NoPath,
    /// O arquivo existe mas o processo não pode escrever nele.
    #[error(
        "no permission to write to {directory}. Run the command in a terminal with administrator privileges."
    )]
    PermissionDenied {
        /// Diretório do arquivo, como o `Path.GetDirectoryName` do legado.
        directory: String,
    },
    /// Qualquer outra falha de E/S ao ler ou escrever o arquivo.
    #[error(
        "could not write to pgpass.conf — the file may be read-only or in use by another process."
    )]
    Write,
    /// Senha vazia: o legado recusa gravar a entrada.
    #[error("password cannot be empty.")]
    EmptyPassword,
}

/// O que aconteceu com a operação.
///
/// O legado devolve um `PgPassResult` com uma mensagem já formatada e o CLI decide o exit code pelo
/// prefixo do texto (`StartsWith("Entry not found")`); aqui o caso é **tipado**, e a mensagem sai de
/// [`PgPassOutcome::message`]. Semântica idêntica, sem depender de comparar strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PgPassOutcome {
    /// Entrada gravada.
    Added,
    /// A identidade já existia; nada foi gravado (e isso não é erro).
    Duplicate,
    /// Entrada removida.
    Removed,
    /// Não havia entrada com essa identidade (o arquivo ficou intacto).
    NotFound,
}

impl PgPassOutcome {
    /// Mensagem exibida ao usuário (texto do legado).
    pub fn message(&self, key: &PgPassEntry) -> String {
        let key = key.formatted_key();
        match self {
            Self::Added => format!("Entry added: {key}"),
            Self::Duplicate => format!("Entry already exists: {key}"),
            Self::Removed => format!("Entry removed: {key}"),
            Self::NotFound => format!("Entry not found: {key}"),
        }
    }
}

/// Acesso ao arquivo `pgpass.conf`.
#[derive(Debug, Clone)]
pub struct PgPassStore {
    path: PathBuf,
}

impl PgPassStore {
    /// Cria um store para um caminho explícito (testes, harness e `MUSTEL_PGPASS_FILE`).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Cria um store no caminho padrão do SO.
    pub fn with_default_path() -> Result<Self, PgPassError> {
        pgpass_file_path().map(Self::new).ok_or(PgPassError::NoPath)
    }

    /// Caminho do arquivo gerenciado.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lista as entradas válidas.
    ///
    /// Erro de leitura sai **cru** (o comando `list` do legado não trata exceção e o `Program.cs`
    /// mapeia `IOException`/`UnauthorizedAccessException` para 20/30).
    pub fn list(&self) -> io::Result<Vec<PgPassEntry>> {
        Ok(read_entries(&self.path)?.unwrap_or_default())
    }

    /// Grava uma entrada, criando o diretório se preciso; identidade repetida não altera o arquivo.
    pub fn add(&self, entry: &PgPassEntry) -> Result<PgPassOutcome, PgPassError> {
        if entry.password.is_empty() {
            return Err(PgPassError::EmptyPassword);
        }

        if read_entries(&self.path)
            .map_err(|error| self.translate(&error))?
            .unwrap_or_default()
            .iter()
            .any(|existing| existing.has_same_identity_as(entry))
        {
            return Ok(PgPassOutcome::Duplicate);
        }

        self.write(|| {
            if let Some(dir) = parent_dir(&self.path) {
                fs::create_dir_all(dir)?;
            }

            let mut line = entry.serialize();
            line.push_str(line_ending());
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?
                .write_all(line.as_bytes())?;
            restrict_permissions(&self.path)
        })?;

        Ok(PgPassOutcome::Added)
    }

    /// Remove a entrada de mesma identidade; arquivo sem a entrada fica intacto.
    pub fn remove(&self, key: &PgPassEntry) -> Result<PgPassOutcome, PgPassError> {
        // `File.Exists` (que engole erro de acesso) decide se há o que remover, como no legado.
        if !self.path.exists() {
            return Ok(PgPassOutcome::NotFound);
        }

        let text = read_text(&self.path).map_err(|error| self.translate(&error))?;
        let mut kept: Vec<&str> = Vec::new();
        let mut found = false;

        for line in split_lines(&text) {
            if line.starts_with('#') || line.trim().is_empty() {
                kept.push(line);
                continue;
            }

            match parse_line(line) {
                Some(entry) if entry.has_same_identity_as(key) => found = true,
                _ => kept.push(line),
            }
        }

        if !found {
            return Ok(PgPassOutcome::NotFound);
        }

        let mut rewritten = String::new();
        for line in kept {
            rewritten.push_str(line);
            rewritten.push_str(line_ending());
        }

        self.write(|| {
            fs::write(&self.path, rewritten.as_bytes())?;
            restrict_permissions(&self.path)
        })?;

        Ok(PgPassOutcome::Removed)
    }

    /// Executa a escrita traduzindo a falha para a mensagem do legado.
    fn write(&self, operation: impl FnOnce() -> io::Result<()>) -> Result<(), PgPassError> {
        operation().map_err(|error| self.translate(&error))
    }

    /// Traduz o erro de E/S na mensagem observada no legado.
    fn translate(&self, error: &io::Error) -> PgPassError {
        if error.kind() == ErrorKind::PermissionDenied {
            PgPassError::PermissionDenied {
                directory: parent_dir(&self.path)
                    .map(|dir| dir.to_string_lossy().to_string())
                    .unwrap_or_default(),
            }
        } else {
            PgPassError::Write
        }
    }
}

/// Lê as entradas do arquivo: `None` quando ele não existe, lista vazia quando não há linha válida.
fn read_entries(path: &Path) -> io::Result<Option<Vec<PgPassEntry>>> {
    if !path.exists() {
        return Ok(None);
    }

    let text = read_text(path)?;
    Ok(Some(
        split_lines(&text)
            .into_iter()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(parse_line)
            .collect(),
    ))
}

/// Conteúdo do arquivo como texto: bytes inválidos viram `U+FFFD`, BOM UTF-8 é descartado.
fn read_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text
        .strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(text))
}

/// Quebra de linha do SO, como o `Environment.NewLine` do legado.
fn line_ending() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

/// Diretório do arquivo, quando existe um (caminho só com nome de arquivo não tem).
fn parent_dir(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

/// Divide o texto em linhas como o `File.ReadAllLines` do .NET (`\n`, `\r\n` e `\r`).
///
/// A quebra final não gera uma linha vazia extra: `"a\n"` é `["a"]` (e `"a\n\n"` é `["a", ""]`).
fn split_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(&text[start..index]);
                index += 1;
                start = index;
            }
            b'\r' => {
                lines.push(&text[start..index]);
                index += if bytes.get(index + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                start = index;
            }
            _ => index += 1,
        }
    }

    if start < bytes.len() {
        lines.push(&text[start..]);
    }

    lines
}

/// Deixa o arquivo com modo `0600` (só Unix; no Windows não há equivalente no `libpq`).
///
/// Divergência deliberada em relação ao legado, que usava o modo padrão do processo.
#[cfg(unix)]
fn restrict_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// No Windows as permissões vêm da ACL herdada do diretório, como no legado.
#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
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
    fn add_creates_the_directory_and_the_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("postgresql").join("pgpass.conf");
        let store = PgPassStore::new(&path);

        let outcome = store
            .add(&entry(
                "localhost",
                "5432",
                "meu_banco",
                "postgres",
                "senha123",
            ))
            .expect("add");

        assert_eq!(outcome, PgPassOutcome::Added);
        assert!(path.is_file(), "o arquivo deve existir");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            format!(
                "localhost:5432:meu_banco:postgres:senha123{}",
                line_ending()
            )
        );
    }

    #[test]
    /// CONTRACT: IF-096
    fn add_preserves_existing_entries_and_appends_colada_when_there_is_no_final_newline() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        std::fs::write(
            &path,
            "localhost:5432:banco_existente:postgres:senha_antiga",
        )
        .expect("seed");
        let store = PgPassStore::new(&path);

        store
            .add(&entry(
                "localhost",
                "5432",
                "banco_novo",
                "postgres",
                "senha_nova",
            ))
            .expect("add");

        // Sem quebra de linha no fim, a entrada nova fica colada — comportamento do `AppendAllText`.
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            format!(
                "localhost:5432:banco_existente:postgres:senha_antiga\
                 localhost:5432:banco_novo:postgres:senha_nova{}",
                line_ending()
            )
        );
    }

    #[test]
    /// CONTRACT: IF-097
    fn duplicate_identity_does_not_touch_the_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);
        let first = entry("localhost", "5432", "meu_banco", "postgres", "senha123");

        store.add(&first).expect("add");
        let after_first = std::fs::read(&path).expect("read");
        // Mesma identidade, senha diferente: continua duplicata (a senha não faz parte da chave).
        let outcome = store
            .add(&entry(
                "localhost",
                "5432",
                "meu_banco",
                "postgres",
                "outra_senha",
            ))
            .expect("add");

        assert_eq!(outcome, PgPassOutcome::Duplicate);
        assert_eq!(std::fs::read(&path).expect("read"), after_first);
        assert_eq!(store.list().expect("list").len(), 1);
    }

    #[test]
    fn add_rejects_an_empty_password_without_creating_the_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("postgresql").join("pgpass.conf");
        let store = PgPassStore::new(&path);

        let error = store
            .add(&entry("localhost", "5432", "meu_banco", "postgres", ""))
            .expect_err("senha vazia deve falhar");

        assert!(matches!(error, PgPassError::EmptyPassword));
        assert_eq!(error.to_string(), "password cannot be empty.");
        assert!(!path.exists(), "nem o diretório deve ser criado");
    }

    #[test]
    fn add_escapes_the_password_and_list_unescapes_it() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);

        store
            .add(&entry(
                "localhost",
                "5432",
                "meu_banco",
                "postgres",
                "senha:com:dois:pontos",
            ))
            .expect("add");

        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains(r"senha\:com\:dois\:pontos")
        );
        assert_eq!(
            store.list().expect("list")[0].password,
            "senha:com:dois:pontos"
        );
    }

    #[test]
    fn add_records_wildcards_literally() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);

        store
            .add(&entry("*", "*", "*", "*", "senhapadrao"))
            .expect("add");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            format!("*:*:*:*:senhapadrao{}", line_ending())
        );
    }

    #[test]
    /// CONTRACT: IF-098
    fn list_is_empty_when_the_file_does_not_exist_or_has_no_valid_line() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);

        assert!(store.list().expect("list").is_empty(), "arquivo ausente");

        std::fs::write(&path, "").expect("seed");
        assert!(store.list().expect("list").is_empty(), "arquivo vazio");

        std::fs::write(&path, "# comentário\nsem campos\n\n").expect("seed");
        assert!(
            store.list().expect("list").is_empty(),
            "comentário, linha inválida e linha em branco"
        );
    }

    #[test]
    /// CONTRACT: IF-098
    fn list_ignores_comments_and_reads_every_valid_line() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        std::fs::write(
            &path,
            "# Exemplo\r\nlocalhost:5432:banco1:postgres:senha1\r\n# Outro\r\ndb.empresa.com:5433:banco2:deploy:senha2\r\n",
        )
        .expect("seed");

        let entries = PgPassStore::new(&path).list().expect("list");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].database, "banco1");
        assert_eq!(entries[1].hostname, "db.empresa.com");
        assert_eq!(entries[1].username, "deploy");
        assert_eq!(entries[1].password, "senha2");
    }

    #[test]
    /// CONTRACT: IF-099
    fn list_reads_a_file_with_utf8_bom_and_invalid_bytes() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"localhost:5432:banco1:postgres:se");
        bytes.push(0xFF);
        bytes.push(b'n');
        std::fs::write(&path, bytes).expect("seed");

        let entries = PgPassStore::new(&path).list().expect("list");

        assert_eq!(entries.len(), 1, "o BOM não pode grudar no hostname");
        assert_eq!(entries[0].hostname, "localhost");
        assert_eq!(entries[0].password, "se\u{fffd}n");
    }

    #[test]
    /// CONTRACT: IF-102
    fn remove_deletes_only_the_matching_entry() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        std::fs::write(
            &path,
            "# cabeçalho\n\nlocalhost:5432:banco1:postgres:senha1\nlocalhost:5432:banco2:postgres:senha2\nlocalhost:5432:banco3:postgres:senha3\n",
        )
        .expect("seed");
        let store = PgPassStore::new(&path);

        let outcome = store
            .remove(&entry("localhost", "5432", "banco2", "postgres", ""))
            .expect("remove");

        assert_eq!(outcome, PgPassOutcome::Removed);
        assert_eq!(
            outcome.message(&entry("localhost", "5432", "banco2", "postgres", "")),
            "Entry removed: localhost:5432:banco2:postgres"
        );
        // Comentário e linha em branco sobrevivem; a última linha ganha quebra final (`WriteAllLines`).
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "# cabeçalho{ending}{ending}localhost:5432:banco1:postgres:senha1{ending}localhost:5432:banco3:postgres:senha3{ending}"
                .replace("{ending}", line_ending())
        );
    }

    #[test]
    /// CONTRACT: IF-102
    fn remove_leaves_the_file_untouched_when_the_entry_is_not_there() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let original = "localhost:5432:banco1:postgres:senha1";
        std::fs::write(&path, original).expect("seed");
        let store = PgPassStore::new(&path);

        let outcome = store
            .remove(&entry("remotehost", "5432", "banco1", "postgres", ""))
            .expect("remove");

        assert_eq!(outcome, PgPassOutcome::NotFound);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), original);
    }

    #[test]
    fn remove_of_a_missing_file_is_not_found_and_creates_nothing() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);

        let outcome = store
            .remove(&entry("localhost", "5432", "banco1", "postgres", ""))
            .expect("remove");

        assert_eq!(outcome, PgPassOutcome::NotFound);
        assert!(!path.exists());
    }

    #[test]
    fn removing_the_last_entry_leaves_an_empty_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        std::fs::write(&path, "localhost:5432:unico:postgres:senha\n").expect("seed");
        let store = PgPassStore::new(&path);

        store
            .remove(&entry("localhost", "5432", "unico", "postgres", ""))
            .expect("remove");

        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    }

    #[test]
    fn remove_matches_the_wildcard_fields_literally() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        std::fs::write(
            &path,
            "*:5432:meu_banco:postgres:senha\nlocalhost:*:meu_banco:postgres:senha\n",
        )
        .expect("seed");
        let store = PgPassStore::new(&path);

        let outcome = store
            .remove(&entry("*", "5432", "meu_banco", "postgres", ""))
            .expect("remove");

        assert_eq!(outcome, PgPassOutcome::Removed);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            format!("localhost:*:meu_banco:postgres:senha{}", line_ending())
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_0600_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("pgpass.conf");
        let store = PgPassStore::new(&path);

        store
            .add(&entry("localhost", "5432", "mydb", "postgres", "senha"))
            .expect("add");
        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        // `remove` reescreve o arquivo: não pode afrouxar o modo de volta.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        store
            .remove(&entry("localhost", "5432", "mydb", "postgres", ""))
            .expect("remove");
        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn split_lines_matches_read_all_lines() {
        assert_eq!(split_lines(""), Vec::<&str>::new());
        assert_eq!(split_lines("a"), vec!["a"]);
        assert_eq!(split_lines("a\n"), vec!["a"]);
        assert_eq!(split_lines("a\n\n"), vec!["a", ""]);
        assert_eq!(split_lines("a\r\nb\rc\n"), vec!["a", "b", "c"]);
        assert_eq!(split_lines("\n"), vec![""]);
    }
}
