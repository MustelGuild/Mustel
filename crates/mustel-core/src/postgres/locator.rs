//! Localização dos executáveis do PostgreSQL (`pg_dump`, `pg_restore`, `psql`).
//!
//! Porte de `legacy/FurLab.Core/Services/PostgresBinaryLocator.cs`, com a **mesma ordem de busca**:
//!
//! 1. cada diretório do `PATH`, procurando `<dir>/<nome>.exe` e depois `<dir>/<nome>`;
//! 2. `C:\Program Files\PostgreSQL` e `C:\PostgreSQL`, procurando em cada **subpasta** (a versão)
//!    o caminho `<subpasta>/bin/<nome>.exe`;
//! 3. `/usr/bin`, `/usr/local/bin` e `/usr/local/pgsql/bin`, procurando `<dir>/<nome>`.
//!
//! As três etapas rodam em qualquer SO, como no legado (que não separa por plataforma): no Linux as
//! pastas do Windows simplesmente não existem, e vice-versa.
//!
//! Diferença deliberada: as subpastas de versão são visitadas em **ordem alfabética**. O
//! `Directory.GetDirectories` do .NET devolve a ordem do sistema de arquivos, que no NTFS também é
//! alfabética — a ordem observada é a mesma, mas aqui é determinística.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Nome do executável de dump (`FurLabConstants.PgDumpExecutable`).
pub const PG_DUMP: &str = "pg_dump";
/// Nome do executável de restauração (`FurLabConstants.PgRestoreExecutable`).
pub const PG_RESTORE: &str = "pg_restore";
/// Nome do cliente interativo (`FurLabConstants.PsqlExecutable`).
pub const PSQL: &str = "psql";

/// Raízes de instalação procuradas no Windows.
const WINDOWS_ROOTS: [&str; 2] = [r"C:\Program Files\PostgreSQL", r"C:\PostgreSQL"];

/// Diretórios procurados no Unix.
const UNIX_DIRS: [&str; 3] = ["/usr/bin", "/usr/local/bin", "/usr/local/pgsql/bin"];

/// Procura os executáveis do PostgreSQL, na ordem do legado.
#[derive(Debug, Clone)]
pub struct PostgresBinaryLocator {
    path_env: Option<OsString>,
    windows_roots: Vec<PathBuf>,
    unix_dirs: Vec<PathBuf>,
}

impl Default for PostgresBinaryLocator {
    fn default() -> Self {
        Self {
            path_env: std::env::var_os("PATH"),
            windows_roots: WINDOWS_ROOTS.iter().map(PathBuf::from).collect(),
            unix_dirs: UNIX_DIRS.iter().map(PathBuf::from).collect(),
        }
    }
}

impl PostgresBinaryLocator {
    /// Localizador padrão do SO.
    pub fn new() -> Self {
        Self::default()
    }

    /// Localizador com diretórios de busca explícitos (testes e harness).
    pub fn with_search_paths(
        path_env: Option<OsString>,
        windows_roots: Vec<PathBuf>,
        unix_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            path_env,
            windows_roots,
            unix_dirs,
        }
    }

    /// Caminho do `pg_dump`, se existir.
    pub fn find_pg_dump(&self) -> Option<PathBuf> {
        self.find(PG_DUMP)
    }

    /// Caminho do `pg_restore`, se existir.
    pub fn find_pg_restore(&self) -> Option<PathBuf> {
        self.find(PG_RESTORE)
    }

    /// Caminho do `psql`, se existir.
    pub fn find_psql(&self) -> Option<PathBuf> {
        self.find(PSQL)
    }

    /// Primeiro candidato existente, na ordem de prioridade.
    pub fn find(&self, executable: &str) -> Option<PathBuf> {
        self.candidates(executable)
            .into_iter()
            .find(|path| path.is_file())
    }

    /// Caminhos candidatos, na ordem em que o legado os testa.
    pub fn candidates(&self, executable: &str) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        for dir in self.path_entries() {
            candidates.push(dir.join(format!("{executable}.exe")));
            candidates.push(dir.join(executable));
        }

        for root in &self.windows_roots {
            for version_dir in version_directories(root) {
                candidates.push(version_dir.join("bin").join(format!("{executable}.exe")));
            }
        }

        for dir in &self.unix_dirs {
            candidates.push(dir.join(executable));
        }

        candidates
    }

    /// Entradas do `PATH`, na ordem em que aparecem.
    fn path_entries(&self) -> Vec<PathBuf> {
        let Some(path) = &self.path_env else {
            return Vec::new();
        };

        std::env::split_paths(path).collect()
    }
}

/// Subpastas de um diretório de instalação (as versões), em ordem alfabética.
fn version_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut directories: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    directories
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cria um arquivo executável de mentira e devolve o diretório.
    fn fake_binary(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).expect("cria diretório");
        std::fs::write(dir.join(name), b"nao sou um executavel de verdade").expect("cria arquivo");
    }

    fn locator(
        path: Option<&Path>,
        windows_roots: &[&Path],
        unix_dirs: &[&Path],
    ) -> PostgresBinaryLocator {
        PostgresBinaryLocator::with_search_paths(
            path.map(|dir| dir.as_os_str().to_os_string()),
            windows_roots.iter().map(PathBuf::from).collect(),
            unix_dirs.iter().map(PathBuf::from).collect(),
        )
    }

    #[test]
    fn finds_the_executable_in_the_path() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        fake_binary(dir.path(), PG_DUMP);

        let found = locator(Some(dir.path()), &[], &[])
            .find_pg_dump()
            .expect("acha no PATH");

        assert_eq!(found, dir.path().join(PG_DUMP));
    }

    #[test]
    fn prefers_the_exe_name_in_the_path() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        fake_binary(dir.path(), PG_RESTORE);
        fake_binary(dir.path(), &format!("{PG_RESTORE}.exe"));

        let found = locator(Some(dir.path()), &[], &[])
            .find_pg_restore()
            .expect("acha no PATH");

        assert_eq!(found, dir.path().join(format!("{PG_RESTORE}.exe")));
    }

    #[test]
    fn falls_back_to_the_version_directories() {
        let root = tempfile::TempDir::new().expect("tempdir");
        // Fora de ordem de propósito: a busca visita em ordem alfabética.
        fake_binary(
            &root.path().join("16").join("bin"),
            &format!("{PG_DUMP}.exe"),
        );
        fake_binary(
            &root.path().join("15").join("bin"),
            &format!("{PG_DUMP}.exe"),
        );

        let found = locator(None, &[root.path()], &[])
            .find_pg_dump()
            .expect("acha na pasta de versão");

        assert_eq!(
            found,
            root.path()
                .join("15")
                .join("bin")
                .join(format!("{PG_DUMP}.exe"))
        );
    }

    #[test]
    fn falls_back_to_the_unix_directories() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        fake_binary(dir.path(), PSQL);

        let found = locator(None, &[], &[dir.path()])
            .find_psql()
            .expect("acha no diretório do Unix");

        assert_eq!(found, dir.path().join(PSQL));
    }

    #[test]
    fn priority_is_path_then_version_then_unix() {
        let path_dir = tempfile::TempDir::new().expect("tempdir");
        let root = tempfile::TempDir::new().expect("tempdir");
        let unix_dir = tempfile::TempDir::new().expect("tempdir");
        fake_binary(path_dir.path(), PG_DUMP);
        fake_binary(
            &root.path().join("16").join("bin"),
            &format!("{PG_DUMP}.exe"),
        );
        fake_binary(unix_dir.path(), PG_DUMP);
        let roots = [root.path()];
        let unix_dirs = [unix_dir.path()];

        // Com PATH, o PATH vence.
        assert_eq!(
            locator(Some(path_dir.path()), &roots, &unix_dirs).find_pg_dump(),
            Some(path_dir.path().join(PG_DUMP))
        );

        // Sem PATH, a pasta de versão do Windows vence o diretório do Unix.
        assert_eq!(
            locator(None, &roots, &unix_dirs).find_pg_dump(),
            Some(
                root.path()
                    .join("16")
                    .join("bin")
                    .join(format!("{PG_DUMP}.exe"))
            )
        );

        // Sem as duas anteriores, sobra o diretório do Unix.
        assert_eq!(
            locator(None, &[], &unix_dirs).find_pg_dump(),
            Some(unix_dir.path().join(PG_DUMP))
        );
    }

    #[test]
    fn returns_none_when_nothing_exists() {
        let empty = tempfile::TempDir::new().expect("tempdir");

        assert!(
            locator(Some(empty.path()), &[], &[])
                .find_pg_dump()
                .is_none()
        );
        assert!(locator(None, &[], &[]).find_psql().is_none());
    }

    #[test]
    fn a_missing_search_directory_is_not_an_error() {
        let missing = PathBuf::from("mustel-pasta-que-nao-existe");

        let locator =
            PostgresBinaryLocator::with_search_paths(None, vec![missing.clone()], vec![missing]);

        assert!(locator.find_pg_dump().is_none());
    }

    #[test]
    fn the_default_locator_uses_the_system_path_and_the_legacy_directories() {
        let default = PostgresBinaryLocator::new();

        assert!(
            default.path_env.is_some(),
            "o PATH do processo deve ser lido"
        );
        assert_eq!(default.windows_roots.len(), WINDOWS_ROOTS.len());
        assert_eq!(default.unix_dirs.len(), UNIX_DIRS.len());
    }
}
