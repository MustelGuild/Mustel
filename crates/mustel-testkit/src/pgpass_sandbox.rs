//! Sandbox do `pgpass.conf` do legado.
//!
//! O FurLab resolve o arquivo por `Environment.SpecialFolder.ApplicationData`, que no Windows vem
//! do **registro** (não de `%APPDATA%`): trocar a variável de ambiente não muda o arquivo lido —
//! verificado rodando o oráculo com dois `%APPDATA%` diferentes, e ele lendo sempre o mesmo arquivo.
//! Como não há como redirecionar o caminho sem mexer no registro, o harness faz o que já faz com a
//! configuração do legado: guarda o arquivo real, semeia o fixture, captura e restaura no fim
//! (inclusive em pânico, via `Drop`).
//!
//! O `pgpass.conf` guarda senhas em **texto claro** — o do usuário pode ter credenciais reais —,
//! então além do backup em memória o harness grava uma cópia em `oracle/workdir/` **antes** de
//! sobrescrever: se o processo for morto de forma violenta (sem `Drop`), recupera-se copiando o
//! arquivo de volta. O diretório é ignorado pelo Git.

use std::path::{Path, PathBuf};

use crate::capture::PgPassSeed;

/// Nome do arquivo de credenciais do PostgreSQL.
const FILE_NAME: &str = "pgpass.conf";

/// Nome do diretório dentro da pasta de dados de aplicação.
const DIR_NAME: &str = "postgresql";

/// Cópia do arquivo real, para o caso de o processo morrer sem passar pelo `Drop`.
const BACKUP_NAME: &str = "legacy-pgpass.backup";

/// Sandbox que preserva o `pgpass.conf` real durante a captura.
pub struct LegacyPgPassSandbox {
    path: PathBuf,
    backup_path: PathBuf,
    backup: Option<Vec<u8>>,
    existed: bool,
}

impl LegacyPgPassSandbox {
    /// Caminho do `pgpass.conf` do legado no SO atual.
    ///
    /// **Não** reusa `mustel_core::pgpass::pgpass_file_path` de propósito: o caminho do oráculo tem
    /// de ser observado, não herdado do código sob teste — se a resolução do Mustel estiver errada,
    /// as duas pontas erradas concordariam e a comparação não acusaria nada.
    pub fn file_path() -> PathBuf {
        legacy_application_data_dir()
            .expect("application data dir")
            .join(DIR_NAME)
            .join(FILE_NAME)
    }

    /// Assume o controle do arquivo, guardando o conteúdo atual.
    pub fn take() -> std::io::Result<Self> {
        let path = Self::file_path();
        let existed = path.is_file();
        let backup = if existed {
            Some(std::fs::read(&path)?)
        } else {
            None
        };

        let backup_path = crate::paths::workspace_dir().join(BACKUP_NAME);
        if let Some(bytes) = &backup {
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&backup_path, bytes)?;
        }

        Ok(Self {
            path,
            backup_path,
            backup,
            existed,
        })
    }

    /// Caminho gerenciado.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Caminho da cópia em disco (existe só quando havia arquivo real).
    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    /// Restaura o estado original do arquivo.
    pub fn restore(&mut self) -> std::io::Result<()> {
        match self.backup.take() {
            Some(bytes) => std::fs::write(&self.path, bytes),
            None if self.existed => Ok(()),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        }
    }
}

impl Drop for LegacyPgPassSandbox {
    fn drop(&mut self) {
        if self.backup.is_some() {
            let _ = self.restore();
        }
    }
}

/// Deixa o `pgpass.conf` em `path` no estado pedido pelo caso.
///
/// Regra única de semeadura, usada pelo `oracle-capture` (no arquivo do legado) e pelo
/// `oracle-compare` (no arquivo isolado do Mustel): os dois lados partem do **mesmo** conteúdo.
/// `Unchanged` não toca no arquivo.
pub fn apply_seed(path: &Path, seed: PgPassSeed) -> std::io::Result<()> {
    let content = match seed {
        PgPassSeed::Unchanged => return Ok(()),
        PgPassSeed::Absent => None,
        PgPassSeed::Fixture => Some(fixture()),
        PgPassSeed::NoTrailingNewline => Some(fixture_without_trailing_newline()),
        PgPassSeed::Wildcard => Some(wildcard_fixture()),
    };

    match content {
        Some(text) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, text)
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

/// Entradas de exemplo usadas nas capturas: um comentário e duas entradas, com a quebra do SO.
///
/// O comentário existe para pinar que ele **não** entra na listagem e que **sobrevive** a um
/// `remove` (o arquivo tem de sair byte a byte igual).
pub fn fixture() -> String {
    [
        "# exemplo",
        "localhost:5432:mydb:postgres:senha123",
        "db.interno:5433:app_dev:deploy:outra_senha",
    ]
    .join(native_line_ending())
        + native_line_ending()
}

/// Fixture de uma entrada **sem** quebra de linha no fim.
///
/// É o caso que pina o `File.AppendAllText` do legado: sem quebra final, a entrada nova entra colada
/// na última (ver `mustel-core/src/pgpass/store.rs`).
pub fn fixture_without_trailing_newline() -> String {
    "localhost:5432:mydb:postgres:senha123".to_string()
}

/// Fixture de uma entrada **curinga**, para o `remove --host "*"` da spec `pgpass-cli-setup`.
pub fn wildcard_fixture() -> String {
    format!("*:5432:mydb:postgres:senha_curinga{}", native_line_ending())
}

/// Quebra de linha do SO.
fn native_line_ending() -> &'static str {
    if cfg!(windows) { "\r\n" } else { "\n" }
}

/// Pasta de dados de aplicação do usuário, como o `SpecialFolder.ApplicationData` do .NET.
fn legacy_application_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::config_dir()
    }

    #[cfg(not(windows))]
    {
        // O .NET trata macOS como Unix puro: `$XDG_CONFIG_HOME` ou `~/.config`.
        if let Some(value) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(value));
        }
        dirs::home_dir().map(|home| home.join(".config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_pgpass_path_ends_with_postgresql_pgpass_conf() {
        let path = LegacyPgPassSandbox::file_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(FILE_NAME)
        );
        assert!(path.to_string_lossy().contains(DIR_NAME), "{:?}", path);
    }

    #[test]
    fn the_fixture_has_a_comment_and_two_valid_entries() {
        let fixture = fixture();
        assert!(fixture.ends_with(native_line_ending()));
        assert!(
            fixture.starts_with("# exemplo"),
            "o comentário vem primeiro"
        );

        let entries: Vec<String> = fixture
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(mustel_core::pgpass::parse_line)
            .map(|entry| entry.formatted_key())
            .collect();
        assert_eq!(
            entries,
            vec![
                "localhost:5432:mydb:postgres".to_string(),
                "db.interno:5433:app_dev:deploy".to_string()
            ]
        );
    }

    #[test]
    fn the_no_trailing_newline_fixture_has_no_newline() {
        let fixture = fixture_without_trailing_newline();
        assert!(!fixture.ends_with('\n'));
        assert!(!fixture.ends_with('\r'));
    }

    #[test]
    fn the_wildcard_fixture_holds_exactly_one_wildcard_entry() {
        let entry =
            mustel_core::pgpass::parse_line(wildcard_fixture().trim_end()).expect("entrada válida");
        assert_eq!(entry.formatted_key(), "*:5432:mydb:postgres");
    }

    #[test]
    fn seeding_writes_and_removes_the_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("postgresql").join("pgpass.conf");

        apply_seed(&path, PgPassSeed::Fixture).expect("seed");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            fixture(),
            "o diretório é criado e o conteúdo é o fixture"
        );

        apply_seed(&path, PgPassSeed::Absent).expect("remover");
        assert!(!path.exists());

        // Remover de novo não é erro, e `Unchanged` não cria nada.
        apply_seed(&path, PgPassSeed::Absent).expect("remover de novo");
        apply_seed(&path, PgPassSeed::Unchanged).expect("não mexer");
        assert!(!path.exists());
    }
}
