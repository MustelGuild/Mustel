//! Sandbox da configuração do legado.
//!
//! O FurLab grava em `%LOCALAPPDATA%\FurLab\furlab.jsonc` (via `SpecialFolder.LocalApplicationData`,
//! que **não** respeita variável de ambiente no Windows). Para capturar comandos que escrevem sem
//! tocar a configuração real do usuário, o harness salva o arquivo, semeia o fixture, roda e
//! restaura no fim (inclusive em caso de pânico, via `Drop`).

use std::path::{Path, PathBuf};

/// Configuração de exemplo usada nas capturas.
pub const SAMPLE_CONFIG: &str = r#"{
  "servers": [
    {
      "name": "dev",
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "databases": ["mydb", "app_dev"],
      "sslMode": "Prefer",
      "timeout": 30,
      "maxParallelism": 4
    },
    {
      "name": "offline",
      "host": "127.0.0.1",
      "port": 1,
      "username": "postgres",
      "sslMode": "Disable",
      "timeout": 1
    }
  ],
  "defaults": {
    "outputDirectory": "./results",
    "requireConfirmation": true,
    "maxParallelism": 4
  }
}
"#;

/// Sandbox que preserva a configuração real do legado durante a captura.
pub struct LegacyConfigSandbox {
    path: PathBuf,
    backup: Option<Vec<u8>>,
    existed: bool,
}

impl LegacyConfigSandbox {
    /// Caminho da configuração do legado no SO atual.
    pub fn config_path() -> PathBuf {
        dirs::data_local_dir()
            .expect("local app data dir")
            .join("FurLab")
            .join("furlab.jsonc")
    }

    /// Assume o controle da configuração do legado, guardando o conteúdo atual.
    pub fn take() -> std::io::Result<Self> {
        let path = Self::config_path();
        let existed = path.is_file();
        let backup = if existed {
            Some(std::fs::read(&path)?)
        } else {
            None
        };
        Ok(Self {
            path,
            backup,
            existed,
        })
    }

    /// Caminho gerenciado.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Escreve o fixture (ou remove o arquivo quando `content` é `None`).
    pub fn seed(&self, content: Option<&str>) -> std::io::Result<()> {
        match content {
            Some(text) => {
                if let Some(parent) = self.path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&self.path, text)
            }
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        }
    }

    /// Restaura o estado original da configuração.
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

impl Drop for LegacyConfigSandbox {
    fn drop(&mut self) {
        if self.backup.is_some() {
            let _ = self.restore();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_config_is_valid_json_with_two_servers() {
        let parsed: serde_json::Value =
            serde_json::from_str(SAMPLE_CONFIG).expect("sample config must parse");
        let servers = parsed["servers"].as_array().expect("servers array");
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0]["name"], "dev");
        assert_eq!(servers[1]["name"], "offline");
    }

    #[test]
    fn legacy_config_path_ends_with_furlab_jsonc() {
        let path = LegacyConfigSandbox::config_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("furlab.jsonc")
        );
        assert!(path.to_string_lossy().contains("FurLab"));
    }
}
