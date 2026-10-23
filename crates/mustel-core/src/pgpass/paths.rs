//! Caminho do `pgpass.conf`.
//!
//! O legado monta o caminho com `Environment.SpecialFolder.ApplicationData` + `postgresql` +
//! `pgpass.conf` (que no Windows é `%APPDATA%\postgresql\pgpass.conf`, a mesma pasta que o `libpq`
//! usa). A resolução abaixo reproduz o .NET **em cada SO**, e não a convenção do `libpq`:
//!
//! | SO | `ApplicationData` no .NET | Aqui |
//! |----|---------------------------|-------|
//! | Windows | `%APPDATA%` (RoamingAppData) | `dirs::config_dir()` (RoamingAppData) |
//! | Linux | `$XDG_CONFIG_HOME` ou `~/.config` | idem |
//! | macOS | `$XDG_CONFIG_HOME` ou `~/.config` | idem |
//!
//! O macOS é o caso em que copiar a convenção da plataforma (`~/Library/Application Support`, que
//! é o que `dirs::config_dir()` devolve lá) **divergiria** do oráculo: o .NET trata macOS como Unix
//! puro. Como o contrato de paridade é o comportamento observado, vale o do .NET.
//!
//! `MUSTEL_PGPASS_FILE` sobrescreve o caminho inteiro — é o que permite ao harness de paridade
//! comparar o arquivo gerado sem tocar o `pgpass.conf` real do usuário (o legado **não** respeita
//! variável de ambiente: `SpecialFolder` vem do registro do Windows, não de `%APPDATA%`).

use std::path::{Path, PathBuf};

/// Nome do diretório dentro da pasta de dados de aplicação.
pub const PGPASS_DIR_NAME: &str = "postgresql";

/// Nome do arquivo de credenciais do PostgreSQL.
pub const PGPASS_FILE_NAME: &str = "pgpass.conf";

/// Variável de ambiente que sobrescreve o caminho do arquivo.
pub const PGPASS_FILE_ENV: &str = "MUSTEL_PGPASS_FILE";

/// Caminho do `pgpass.conf` no SO atual, ou `None` quando não há pasta de dados de aplicação.
pub fn pgpass_file_path() -> Option<PathBuf> {
    let override_path = std::env::var_os(PGPASS_FILE_ENV).filter(|value| !value.is_empty());
    pgpass_file_path_with(override_path.as_deref().map(Path::new))
}

/// Versão pura de [`pgpass_file_path`]: sem tocar o ambiente, para ser testável.
pub fn pgpass_file_path_with(override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        return Some(path.to_path_buf());
    }

    application_data_dir().map(|dir| dir.join(PGPASS_DIR_NAME).join(PGPASS_FILE_NAME))
}

/// Pasta de dados de aplicação do usuário, como o `SpecialFolder.ApplicationData` do .NET.
fn application_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // RoamingAppData — igual ao `%APPDATA%`, e obtido do registro (não da variável de ambiente).
        dirs::config_dir()
    }

    #[cfg(not(windows))]
    {
        xdg_config_dir()
    }
}

/// `$XDG_CONFIG_HOME` (se definida e não vazia) ou `~/.config`, como o .NET faz em Unix.
#[cfg(not(windows))]
fn xdg_config_dir() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(value));
    }

    dirs::home_dir().map(|home| home.join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-103
    fn override_path_wins_over_the_platform_directory() {
        let path = pgpass_file_path_with(Some(Path::new("/tmp/pgpass-fixture.conf")));
        assert_eq!(path, Some(PathBuf::from("/tmp/pgpass-fixture.conf")));
    }

    #[test]
    /// CONTRACT: IF-103
    fn default_path_lives_under_postgresql_in_the_application_data_dir() {
        let path = pgpass_file_path_with(None).expect("platform application data dir");
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new(PGPASS_FILE_NAME))
        );
        assert_eq!(
            path.parent().and_then(Path::file_name),
            Some(std::ffi::OsStr::new(PGPASS_DIR_NAME))
        );
    }

    #[test]
    fn empty_override_is_ignored() {
        // `MUSTEL_PGPASS_FILE=` (vazia) não pode virar um caminho válido.
        let path = pgpass_file_path_with(None);
        assert!(path.is_some());
    }
}
