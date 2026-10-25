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

/// Subpasta onde ficam os DDLs baixados e a sessão da IA.
const AI_DIR_NAME: &str = "ai";

/// Subpasta dos dumps de schema (um arquivo por servidor × banco).
const DDL_DIR_NAME: &str = "ddl";

/// Nome do arquivo da sessão de conversa com a IA.
const AI_SESSION_FILE_NAME: &str = "session.json";

/// Diretório dos DDLs em cache (`<config>/ai/ddl`), criado se preciso.
///
/// O cache existe para não rodar o `pg_dump` a cada abertura do painel: o arquivo guarda o dump já
/// filtrado e um `.json` ao lado com a data.
pub fn ddl_cache_dir() -> Option<PathBuf> {
    config_dir().as_deref().map(ddl_cache_dir_with)
}

/// Versão pura de [`ddl_cache_dir`] (sem tocar o ambiente).
pub fn ddl_cache_dir_with(config_dir: &Path) -> PathBuf {
    let dir = ai_dir_with(config_dir).join(DDL_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Caminho do arquivo de sessão da conversa com a IA.
pub fn ai_session_path() -> Option<PathBuf> {
    config_dir().as_deref().map(ai_session_path_with)
}

/// Versão pura de [`ai_session_path`] (sem tocar o ambiente).
pub fn ai_session_path_with(config_dir: &Path) -> PathBuf {
    ai_dir_with(config_dir).join(AI_SESSION_FILE_NAME)
}

/// A pasta `ai/` dentro da configuração, criada se ainda não existir.
fn ai_dir_with(config_dir: &Path) -> PathBuf {
    let dir = config_dir.join(AI_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Nome de arquivo seguro para um par servidor × banco.
///
/// O nome do servidor é livre (o dono batiza), então `/`, `\`, `:` e espaço não podem virar
/// caminho. Tudo que não é alfanumérico, `-` ou `_` vira `_`.
pub fn ddl_file_stem(server: &str, database: &str) -> String {
    fn safe(text: &str) -> String {
        text.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
    format!("{}__{}", safe(server), safe(database))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-161
    fn override_directory_wins() {
        assert_eq!(
            config_dir_with(Some(Path::new("/tmp/mustel-tests"))),
            Some(PathBuf::from("/tmp/mustel-tests"))
        );
    }

    #[test]
    /// CONTRACT: IF-131
    fn without_override_uses_a_directory_named_mustel() {
        let dir = config_dir_with(None).expect("platform config dir must resolve");
        assert!(
            dir.ends_with(APP_DIR_NAME),
            "expected a trailing {APP_DIR_NAME} in {dir:?}"
        );
    }

    /// O cache de DDL fica em `<config>/ai/ddl` e o diretório é criado.
    #[test]
    fn the_ddl_cache_lives_under_the_ai_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dir = ddl_cache_dir_with(temp.path());

        assert_eq!(dir, temp.path().join("ai").join("ddl"));
        assert!(dir.is_dir(), "o diretório é criado: {dir:?}");
    }

    /// A sessão da IA é um arquivo ao lado do cache, não dentro dele.
    #[test]
    fn the_ai_session_lives_next_to_the_ddl_cache() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = ai_session_path_with(temp.path());

        assert_eq!(path, temp.path().join("ai").join("session.json"));
        assert_eq!(
            path.parent(),
            Some(ddl_cache_dir_with(temp.path()).parent().unwrap())
        );
    }

    /// O nome do arquivo de cache não deixa `/`, `:` nem espaço virarem caminho.
    #[test]
    fn the_ddl_file_name_is_sanitized() {
        assert_eq!(ddl_file_stem("Local", "dev"), "Local__dev");
        assert_eq!(
            ddl_file_stem("Local: 2/DEV", "meu banco"),
            "Local__2_DEV__meu_banco"
        );
    }

    #[test]
    /// CONTRACT: IF-131
    fn config_file_lives_inside_the_config_directory() {
        let path = config_file_path().expect("platform config file path");
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new(CONFIG_FILE_NAME))
        );
        assert_eq!(path.parent(), config_dir().as_deref());
    }
}
