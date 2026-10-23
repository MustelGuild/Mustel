//! Modelos do arquivo de configuração do usuário (`mustel.jsonc`).

use serde::{Deserialize, Serialize};

/// Padrões de exclusão usados quando o servidor não define os seus (mesmo default do FurLab).
pub const DEFAULT_EXCLUDE_PATTERNS: [&str; 2] = ["template*", "postgres"];

/// Conteúdo completo do `mustel.jsonc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserConfig {
    /// Servidores PostgreSQL configurados.
    pub servers: Vec<ServerConfigEntry>,
    /// Preferências aplicadas quando a CLI não sobrescreve.
    pub defaults: Option<UserDefaults>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            defaults: Some(UserDefaults::default()),
        }
    }
}

impl UserConfig {
    /// Configuração vazia usada no primeiro uso (Q8: sem import do `furlab.jsonc`).
    pub fn new_empty() -> Self {
        Self::default()
    }

    /// Aplica os defaults e validações do FurLab (`ValidateAndApplyDefaults`).
    pub fn apply_defaults(&mut self) {
        if self.defaults.is_none() {
            self.defaults = Some(UserDefaults::default());
        }
        for server in &mut self.servers {
            server.apply_defaults();
        }
    }
}

/// Um servidor PostgreSQL configurado.
///
/// Não guarda senha: credenciais vivem no cofre do sistema (`keyring`, D6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfigEntry {
    /// Identificador único do servidor (comparado sem diferenciar maiúsculas).
    pub name: String,
    /// Host do PostgreSQL.
    pub host: String,
    /// Porta do PostgreSQL (default 5432).
    pub port: u16,
    /// Usuário da conexão.
    pub username: String,
    /// Bancos específicos usados quando `fetchAllDatabases` é falso.
    pub databases: Vec<String>,
    /// Descobre todos os bancos do servidor automaticamente.
    pub fetch_all_databases: bool,
    /// Padrões excluídos na descoberta automática.
    pub exclude_patterns: Vec<String>,
    /// Modo SSL (`Disable`, `Allow`, `Prefer`, `Require`, `VerifyCA`, `VerifyFull`).
    pub ssl_mode: String,
    /// Timeout de conexão em segundos (default 30).
    pub timeout: u32,
    /// Paralelismo máximo por servidor (default 4).
    pub max_parallelism: u32,
}

impl Default for ServerConfigEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 5432,
            username: String::new(),
            databases: Vec::new(),
            fetch_all_databases: false,
            exclude_patterns: DEFAULT_EXCLUDE_PATTERNS
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect(),
            ssl_mode: "Prefer".to_string(),
            timeout: 30,
            max_parallelism: 4,
        }
    }
}

impl ServerConfigEntry {
    /// Normaliza campos que o arquivo trouxe explicitamente zerados ou vazios.
    pub fn apply_defaults(&mut self) {
        if self.port == 0 {
            self.port = 5432;
        }
        if self.ssl_mode.trim().is_empty() {
            self.ssl_mode = "Prefer".to_string();
        }
        if self.timeout == 0 {
            self.timeout = 30;
        }
        if self.max_parallelism == 0 {
            self.max_parallelism = 4;
        }
        if self.exclude_patterns.is_empty() {
            self.exclude_patterns = DEFAULT_EXCLUDE_PATTERNS
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect();
        }
    }
}

/// Preferências padrão do arquivo (`defaults`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserDefaults {
    /// Diretório padrão dos CSV gerados por `query run`.
    pub output_directory: String,
    /// Exige confirmação antes de queries destrutivas.
    pub require_confirmation: bool,
    /// Paralelismo máximo padrão.
    pub max_parallelism: u32,
}

impl Default for UserDefaults {
    fn default() -> Self {
        Self {
            output_directory: "./results".to_string(),
            require_confirmation: true,
            max_parallelism: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_legacy_fur_lab_values() {
        let server = ServerConfigEntry::default();
        assert_eq!(server.port, 5432);
        assert_eq!(server.ssl_mode, "Prefer");
        assert_eq!(server.timeout, 30);
        assert_eq!(server.max_parallelism, 4);
        assert_eq!(server.exclude_patterns, vec!["template*", "postgres"]);

        let defaults = UserDefaults::default();
        assert_eq!(defaults.output_directory, "./results");
        assert!(defaults.require_confirmation);
        assert_eq!(defaults.max_parallelism, 4);
    }

    #[test]
    fn apply_defaults_fixes_zeroed_and_empty_fields() {
        let mut server = ServerConfigEntry {
            name: "dev".to_string(),
            port: 0,
            ssl_mode: "   ".to_string(),
            timeout: 0,
            max_parallelism: 0,
            exclude_patterns: Vec::new(),
            ..ServerConfigEntry::default()
        };
        server.apply_defaults();
        assert_eq!(server.port, 5432);
        assert_eq!(server.ssl_mode, "Prefer");
        assert_eq!(server.timeout, 30);
        assert_eq!(server.max_parallelism, 4);
        assert_eq!(server.exclude_patterns, vec!["template*", "postgres"]);
    }

    #[test]
    /// CONTRACT: IF-114
    fn serializes_with_camel_case_keys_in_legacy_order() {
        let config = UserConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("serializes");
        assert_eq!(
            json,
            "{\n  \"servers\": [],\n  \"defaults\": {\n    \"outputDirectory\": \"./results\",\n    \"requireConfirmation\": true,\n    \"maxParallelism\": 4\n  }\n}"
        );
    }

    #[test]
    /// CONTRACT: IF-129
    fn tolerates_unknown_fields_from_hand_edited_files() {
        let json = r#"{
          "servers": [
            {
              "name": "dev",
              "host": "localhost",
              "password": "legacy-field",
              "commandTimeout": 300
            }
          ],
          "defaults": { "outputFormat": "csv", "fetchAllDatabases": false }
        }"#;
        let config: UserConfig = serde_json::from_str(json).expect("deserializes");
        assert_eq!(config.servers.len(), 1);
        assert_eq!(
            config.servers[0].port, 5432,
            "missing port falls back to default"
        );
    }
}
