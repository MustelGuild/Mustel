//! Configuração do usuário: leitura, escrita e consultas do `mustel.jsonc`.
//!
//! Comportamento espelha o `UserConfigService` do FurLab (defaults, validações, busca sem
//! diferenciar maiúsculas, criação na primeira execução), com duas diferenças deliberadas:
//! a escrita é atômica (temp + rename) e não existe campo de senha (credenciais vão para o
//! `keyring`, D6).

pub mod jsonc;
pub mod model;
pub mod paths;

use std::fs;
use std::path::{Path, PathBuf};

pub use model::{ServerConfigEntry, UserConfig, UserDefaults};
pub use paths::{CONFIG_DIR_ENV, CONFIG_FILE_NAME, config_dir, config_file_path};

/// Erros de leitura/escrita da configuração.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// O SO não expõe uma pasta de configuração e `MUSTEL_CONFIG_DIR` não foi definida.
    #[error("could not determine the configuration directory for this platform")]
    NoConfigDir,
    /// Falha ao ler o arquivo.
    #[error("failed to read {path}: {source}")]
    Read {
        /// Caminho do arquivo.
        path: PathBuf,
        /// Erro de I/O original.
        source: std::io::Error,
    },
    /// Falha ao escrever o arquivo.
    #[error("failed to write {path}: {source}")]
    Write {
        /// Caminho do arquivo.
        path: PathBuf,
        /// Erro de I/O original.
        source: std::io::Error,
    },
    /// O conteúdo não é JSON/JSONC válido.
    #[error("invalid JSON: {message}")]
    Parse {
        /// Mensagem do parser.
        message: String,
    },
    /// Nome de servidor vazio em `add`/`set-password`.
    #[error("server name cannot be empty")]
    EmptyServerName,
    /// Servidor referenciado mas inexistente na configuração.
    #[error("Server '{0}' not found.")]
    ServerNotFound(String),
}

/// Resultado das operações de configuração.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Acesso ao arquivo `mustel.jsonc`.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    /// Cria um store para um caminho explícito (usado por testes e pelo harness).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Cria um store no caminho padrão do SO.
    pub fn with_default_path() -> Result<Self> {
        config_file_path()
            .map(Self::new)
            .ok_or(ConfigError::NoConfigDir)
    }

    /// Caminho do arquivo gerenciado.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Indica se o arquivo já existe.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Lê a configuração, criando uma vazia na primeira execução (Q8).
    pub fn load(&self) -> Result<UserConfig> {
        if !self.exists() {
            let config = UserConfig::new_empty();
            self.save(&config)?;
            return Ok(config);
        }

        let raw = fs::read_to_string(&self.path).map_err(|source| ConfigError::Read {
            path: self.path.clone(),
            source,
        })?;

        let stripped = jsonc::strip_comments(&raw);
        let mut config: UserConfig =
            serde_json::from_str(&stripped).map_err(|error| ConfigError::Parse {
                message: error.to_string(),
            })?;
        config.apply_defaults();
        Ok(config)
    }

    /// Grava a configuração de forma atômica (arquivo temporário + rename).
    pub fn save(&self, config: &UserConfig) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|source| ConfigError::Write {
                path: self.path.clone(),
                source,
            })?;
        }

        let json = serde_json::to_string_pretty(config).map_err(|error| ConfigError::Parse {
            message: error.to_string(),
        })?;

        let temp_path = temporary_path(&self.path);
        fs::write(&temp_path, native_newlines(json)).map_err(|source| ConfigError::Write {
            path: temp_path.clone(),
            source,
        })?;
        fs::rename(&temp_path, &self.path).map_err(|source| ConfigError::Write {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }

    /// Lista os servidores configurados.
    pub fn get_servers(&self) -> Result<Vec<ServerConfigEntry>> {
        Ok(self.load()?.servers)
    }

    /// Busca um servidor pelo nome (sem diferenciar maiúsculas/minúsculas).
    pub fn get_server(&self, name: &str) -> Result<Option<ServerConfigEntry>> {
        Ok(self
            .load()?
            .servers
            .into_iter()
            .find(|server| server.name.eq_ignore_ascii_case(name)))
    }

    /// Adiciona ou substitui um servidor.
    pub fn add_or_update_server(&self, server: &ServerConfigEntry) -> Result<()> {
        if server.name.trim().is_empty() {
            return Err(ConfigError::EmptyServerName);
        }

        let mut config = self.load()?;
        match config
            .servers
            .iter_mut()
            .find(|existing| existing.name.eq_ignore_ascii_case(&server.name))
        {
            Some(existing) => *existing = server.clone(),
            None => config.servers.push(server.clone()),
        }
        self.save(&config)
    }

    /// Remove um servidor; devolve `false` quando ele não existia.
    pub fn remove_server(&self, name: &str) -> Result<bool> {
        let mut config = self.load()?;
        let previous_len = config.servers.len();
        config
            .servers
            .retain(|server| !server.name.eq_ignore_ascii_case(name));
        if config.servers.len() == previous_len {
            return Ok(false);
        }
        self.save(&config)?;
        Ok(true)
    }
}

/// Ajusta as quebras de linha ao SO, como o arquivo escrito pelo legado.
///
/// Observado na captura do oráculo (T9): no Windows o `furlab.jsonc` sai com CRLF. Manter o mesmo
/// padrão evita uma diferença de bytes puramente cosmética nos artefatos comparados.
fn native_newlines(json: String) -> String {
    if cfg!(windows) {
        json.replace('\n', "\r\n")
    } else {
        json
    }
}

/// Caminho temporário usado na escrita atômica (mesmo diretório, para o rename ser barato).
fn temporary_path(path: &Path) -> PathBuf {
    let mut temp = path.to_path_buf();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| CONFIG_FILE_NAME.to_string());
    temp.set_file_name(format!("{file_name}.tmp"));
    temp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-111
    fn native_newlines_follow_the_platform() {
        let converted = native_newlines("a\nb".to_string());
        if cfg!(windows) {
            assert_eq!(converted, "a\r\nb");
        } else {
            assert_eq!(converted, "a\nb");
        }
    }

    #[test]
    fn temporary_path_keeps_the_same_directory() {
        let temp = temporary_path(Path::new("/tmp/mustel/mustel.jsonc"));
        assert_eq!(temp, PathBuf::from("/tmp/mustel/mustel.jsonc.tmp"));
    }
}
