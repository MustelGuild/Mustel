use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use colored::Colorize;

use crate::config::models::{ServerConfigEntry, UserConfig, UserDefaults};
use crate::error::{MustelError, Result};

pub struct UserConfigService {
    config_file_path: PathBuf,
}

impl UserConfigService {
    pub fn new() -> Self {
        let config_file_path = Self::resolve_config_path();
        Self { config_file_path }
    }

    /// Resolves the configuration path. Checks for %LocalAppData%/Mustel/mustel.jsonc,
    /// falling back to %LocalAppData%/FurLab/furlab.jsonc if present, otherwise creating Mustel dir.
    fn resolve_config_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "Mustel") {
            let mustel_path = proj_dirs.config_dir().join("mustel.jsonc");
            if mustel_path.exists() {
                return mustel_path;
            }

            // Check legacy FurLab path
            if let Some(furlab_dirs) = ProjectDirs::from("", "", "FurLab") {
                let furlab_path = furlab_dirs.config_dir().join("furlab.jsonc");
                if furlab_path.exists() {
                    return furlab_path;
                }
            }

            return mustel_path;
        }

        // Fallback to local ./mustel.jsonc if OS paths fail
        PathBuf::from("mustel.jsonc")
    }

    #[allow(dead_code)]
    pub fn get_config_file_path(&self) -> &Path {
        &self.config_file_path
    }

    pub fn config_file_exists(&self) -> bool {
        self.config_file_path.exists()
    }

    /// SECURITY: Checks if the config file has insecure permissions (world-readable or group-readable).
    /// Only warns on Unix systems; Windows uses ACLs which are harder to check.
    fn check_config_permissions(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Ok(metadata) = fs::metadata(&self.config_file_path) {
                let mode = metadata.permissions().mode();
                // Check if world-readable (04) or group-readable (02)
                if mode & 0o047 != 0 {
                    eprintln!(
                        "{} {}",
                        "WARNING:".yellow().bold(),
                        format!(
                            "Config file '{}' has insecure permissions ({:o}). \
                             Credentials may be readable by other users. \
                             Run: chmod 600 '{}'",
                            self.config_file_path.display(),
                            mode & 0o777,
                            self.config_file_path.display()
                        ).yellow()
                    );
                }
            }
        }
        Ok(())
    }

    /// SECURITY: Sets restrictive permissions on the config file (user read/write only).
    fn secure_config_permissions(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::Permissions::mode(0o600);
            fs::set_permissions(&self.config_file_path, perms)?;
        }
        Ok(())
    }

    pub fn load_config(&self) -> Result<UserConfig> {
        if !self.config_file_exists() {
            let default_config = UserConfig::default();
            self.save_config(&default_config)?;
            return Ok(default_config);
        }

        // SECURITY: Check permissions before loading
        self.check_config_permissions()?;

        let content = fs::read_to_string(&self.config_file_path)?;
        let config: UserConfig = json5::from_str(&content)
            .map_err(|e| MustelError::Json(format!("Failed to parse config at {:?}: {}", self.config_file_path, e)))?;

        Ok(config)
    }

    pub fn save_config(&self, config: &UserConfig) -> Result<()> {
        if let Some(parent) = self.config_file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json_content = serde_json::to_string_pretty(config)
            .map_err(|e| MustelError::Json(format!("Failed to serialize config: {}", e)))?;

        // Write to temp file first, then rename (atomic write)
        let temp_path = self.config_file_path.with_extension("tmp");
        fs::write(&temp_path, &json_content)?;

        // SECURITY: Set restrictive permissions before renaming
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::Permissions::mode(0o600);
            fs::set_permissions(&temp_path, perms)?;
        }

        fs::rename(&temp_path, &self.config_file_path)?;

        // Final permission check
        self.check_config_permissions()?;

        Ok(())
    }

    pub fn get_servers(&self) -> Result<Vec<ServerConfigEntry>> {
        let config = self.load_config()?;
        Ok(config.servers)
    }

    pub fn get_server(&self, name: &str) -> Result<Option<ServerConfigEntry>> {
        let config = self.load_config()?;
        Ok(config.servers.into_iter().find(|s| s.name.eq_ignore_ascii_case(name)))
    }

    pub fn add_or_update_server(&self, server: ServerConfigEntry) -> Result<()> {
        let mut config = self.load_config()?;
        if let Some(pos) = config.servers.iter().position(|s| s.name.eq_ignore_ascii_case(&server.name)) {
            config.servers[pos] = server;
        } else {
            config.servers.push(server);
        }
        self.save_config(&config)
    }

    pub fn remove_server(&self, name: &str) -> Result<bool> {
        let mut config = self.load_config()?;
        let initial_len = config.servers.len();
        config.servers.retain(|s| !s.name.eq_ignore_ascii_case(name));
        let removed = config.servers.len() < initial_len;
        if removed {
            self.save_config(&config)?;
        }
        Ok(removed)
    }

    pub fn set_encrypted_password(&self, server_name: &str, encrypted_password: String) -> Result<()> {
        let mut config = self.load_config()?;
        if let Some(server) = config.servers.iter_mut().find(|s| s.name.eq_ignore_ascii_case(server_name)) {
            server.encrypted_password = Some(encrypted_password);
            server.password = None; // Clear plaintext password if encrypted password is set
            self.save_config(&config)?;
            Ok(())
        } else {
            Err(MustelError::Config(format!("Server '{}' not found", server_name)))
        }
    }

    pub fn get_defaults(&self) -> Result<UserDefaults> {
        let config = self.load_config()?;
        Ok(config.defaults)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_parsing_error() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let config_path = temp_file.path().to_path_buf();

        fs::write(&config_path, "invalid json content { [").unwrap();

        let service = UserConfigService {
            config_file_path: config_path.clone(),
        };

        let result = service.load_config();

        assert!(result.is_err());
        match result.unwrap_err() {
            MustelError::Json(msg) => {
                assert!(msg.contains("Failed to parse config at"));
            },
            _ => panic!("Expected MustelError::Json"),
        }
    }
}
