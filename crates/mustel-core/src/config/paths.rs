//! Resolução de caminhos da configuração do usuário.

use std::path::{Path, PathBuf};

/// Diretório da aplicação dentro da pasta de configuração do sistema.
pub const APP_DIR_NAME: &str = "Mustel";

/// Nome do arquivo de configuração.
pub const CONFIG_FILE_NAME: &str = "mustel.jsonc";

/// Variável de ambiente que sobrescreve o diretório de configuração.
///
/// Existe para isolar execuções de teste e do harness de paridade (`oracle-*`), que nunca devem
/// tocar a configuração real do usuário.
pub const CONFIG_DIR_ENV: &str = "MUSTEL_CONFIG_DIR";

/// Diretório de configuração do Mustel no SO atual.
///
/// Ordem: `MUSTEL_CONFIG_DIR` (se definida e não vazia) → pasta de configuração do sistema
/// (`%APPDATA%` no Windows, `~/.config` no Linux, `~/Library/Application Support` no macOS).
pub fn config_dir() -> Option<PathBuf> {
    let override_dir = std::env::var_os(CONFIG_DIR_ENV).filter(|value| !value.is_empty());
    config_dir_with(override_dir.as_deref().map(Path::new))
}

/// Versão pura de [`config_dir`]: sem tocar o ambiente, para ser testável e reutilizável.
pub fn config_dir_with(override_dir: Option<&Path>) -> Option<PathBuf> {
    match override_dir {
        Some(dir) => Some(dir.to_path_buf()),
        None => dirs::config_dir().map(|dir| dir.join(APP_DIR_NAME)),
    }
}

/// Caminho completo do `mustel.jsonc`, ou `None` quando o SO não expõe pasta de configuração.
pub fn config_file_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_directory_wins() {
        assert_eq!(
            config_dir_with(Some(Path::new("/tmp/mustel-tests"))),
            Some(PathBuf::from("/tmp/mustel-tests"))
        );
    }

    #[test]
    fn without_override_uses_a_directory_named_mustel() {
        let dir = config_dir_with(None).expect("platform config dir must resolve");
        assert!(
            dir.ends_with(APP_DIR_NAME),
            "expected a trailing {APP_DIR_NAME} in {dir:?}"
        );
    }

    #[test]
    fn config_file_lives_inside_the_config_directory() {
        let path = config_file_path().expect("platform config file path");
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new(CONFIG_FILE_NAME))
        );
        assert_eq!(path.parent(), config_dir().as_deref());
    }
}
