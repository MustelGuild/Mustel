//! Modelos do arquivo de configuração do usuário (`mustel.jsonc`).

use serde::{Deserialize, Serialize};

/// Padrões de exclusão usados quando o servidor não define os seus (mesmo default do FurLab).
pub const DEFAULT_EXCLUDE_PATTERNS: [&str; 2] = ["template*", "postgres"];

/// Conteúdo completo do `mustel.jsonc`.
///
/// Sem `Eq` porque `GuiPrefs::font_scale` é `f32`: comparação de igualdade exata fica em `PartialEq`
/// e é o que os testes de ida-e-volta do arquivo precisam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserConfig {
    /// Servidores PostgreSQL configurados.
    pub servers: Vec<ServerConfigEntry>,
    /// Preferências aplicadas quando a CLI não sobrescreve.
    pub defaults: Option<UserDefaults>,
    /// Preferências da janela (aba **Settings**).
    ///
    /// Fica **fora do JSON** enquanto ninguém configura nada (`skip_serializing_if`): o arquivo
    /// continua byte a byte igual ao de antes desta seção existir. Quando existe, o CLI a preserva —
    /// `settings db-servers add` reescreve o arquivo inteiro com este modelo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gui: Option<GuiPrefs>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            defaults: Some(UserDefaults::default()),
            gui: None,
        }
    }
}

/// Escala de fonte padrão (1.0 = tamanhos do egui).
pub const DEFAULT_FONT_SCALE: f32 = 1.0;

/// Faixa aceita para a escala de fonte da janela.
///
/// O teto não é estético: é medido. Com nomes típicos, a linha mais larga da tela de consulta ocupa
/// ~332 px a 1.0 na fonte embutida e ~413 px na fonte de sistema mais larga testada (Lucida Console);
/// 1.25 ainda cabe na janela mínima de 640 px com folga. Acima disso o rótulo de estado mais longo
/// passa a disputar espaço com os combos.
pub const MIN_FONT_SCALE: f32 = 0.8;
pub const MAX_FONT_SCALE: f32 = 1.25;

/// Preferências de aparência da janela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuiPrefs {
    /// Família da **interface** (texto comum). `None` = a embutida (Ubuntu-Light).
    pub interface_font: Option<String>,
    /// Família do **editor de SQL** (monoespaçada). `None` = a embutida (Hack).
    pub editor_font: Option<String>,
    /// Escala aplicada a todos os tamanhos de texto da janela.
    pub font_scale: f32,
    /// Queries salvas que a aba **Query** usa para marcar bases em massa.
    ///
    /// Fica fora do JSON enquanto a lista está vazia, pela mesma razão da seção `gui`: quem não usa
    /// a seleção por query não vê o arquivo mudar.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub selections: Vec<SelectionQuery>,
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            interface_font: None,
            editor_font: None,
            font_scale: DEFAULT_FONT_SCALE,
            selections: Vec::new(),
        }
    }
}

/// Uma query salva que decide **quais bases** marcar (aba **Query**).
///
/// Não guarda servidor de destino: o resultado da query é casado com o **cadastro** de servidores
/// (`settings db-servers`), e host fora dele é erro. `server`/`database` dizem onde o SQL roda —
/// também um servidor do cadastro, com a senha vinda do cofre.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SelectionQuery {
    /// Nome curto, único sem diferenciar maiúsculas. É o rótulo do combo.
    pub name: String,
    /// Servidor **do cadastro** onde o SQL roda.
    pub server: String,
    /// Banco desse servidor onde o SQL roda.
    pub database: String,
    /// O SQL; devolve as bases por connection string ou por colunas.
    pub sql: String,
}

impl GuiPrefs {
    /// Escala dentro da faixa aceita.
    ///
    /// O arquivo é editável à mão, então o valor lido não é confiável: um `5.0` salvo por engano
    /// deixaria a janela inutilizável antes de alguém abrir a aba Settings para corrigir.
    pub fn clamped_scale(&self) -> f32 {
        if !self.font_scale.is_finite() {
            return DEFAULT_FONT_SCALE;
        }
        self.font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
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

    /// Enquanto ninguém mexe na janela, o arquivo não ganha a seção `gui`.
    ///
    /// É o que mantém o contrato do CLI intacto: `settings db-servers add` reescreve o arquivo
    /// inteiro, então uma seção que existisse sempre mudaria o JSON de todo mundo.
    /// CONTRACT: IF-218
    #[test]
    fn a_config_without_gui_preferences_serializes_without_the_section() {
        let config = UserConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("serializes");

        assert!(
            !json.contains("\"gui\""),
            "sem preferências, nem a chave aparece: {json}"
        );

        // E o texto continua sendo o mesmo de antes da seção existir.
        let expected = serde_json::to_string_pretty(&UserConfig {
            servers: Vec::new(),
            defaults: Some(UserDefaults::default()),
            gui: None,
        })
        .expect("serializes");
        assert_eq!(json, expected);
    }

    /// Preferências da janela sobrevivem à reescrita do arquivo pelo CLI.
    ///
    /// CONTRACT: IF-218
    #[test]
    fn gui_preferences_survive_a_cli_rewrite() {
        let config = UserConfig {
            gui: Some(GuiPrefs {
                interface_font: Some("Segoe UI".to_string()),
                editor_font: Some("Fira Code".to_string()),
                font_scale: 1.15,
                selections: Vec::new(),
            }),
            ..UserConfig::default()
        };

        // O CLI grava o modelo inteiro: salvar e reler tem de trazer o mesmo conteúdo.
        let json = serde_json::to_string_pretty(&config).expect("serializes");
        let back: UserConfig = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(back, config);
        let gui = back.gui.expect("a seção sobreviveu");
        assert_eq!(gui.interface_font.as_deref(), Some("Segoe UI"));
        assert_eq!(gui.editor_font.as_deref(), Some("Fira Code"));
        assert!((gui.font_scale - 1.15).abs() < f32::EPSILON);
    }

    /// Escala fora da faixa é **lida** com limite, sem reescrever o arquivo.
    ///
    /// O arquivo é editável à mão: um `5.0` por engano deixaria a janela inutilizável antes de
    /// alguém conseguir abrir a aba Settings para corrigir.
    /// CONTRACT: IF-218
    #[test]
    fn the_font_scale_is_clamped_when_read() {
        let scale = |value: f32| {
            GuiPrefs {
                font_scale: value,
                ..GuiPrefs::default()
            }
            .clamped_scale()
        };

        assert_eq!(scale(1.0), 1.0);
        assert_eq!(scale(5.0), MAX_FONT_SCALE, "acima do teto, corta no teto");
        assert_eq!(scale(0.1), MIN_FONT_SCALE, "abaixo do piso, corta no piso");
        assert_eq!(
            scale(f32::NAN),
            DEFAULT_FONT_SCALE,
            "valor corrompido volta ao padrão"
        );
    }

    /// Quem só usa a fonte não vê a chave `selections` no arquivo (D11).
    ///
    /// CONTRACT: IF-229
    #[test]
    fn empty_selections_do_not_appear_in_the_file() {
        let prefs = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            ..GuiPrefs::default()
        };
        let json = serde_json::to_string_pretty(&prefs).expect("serializes");
        assert!(
            !json.contains("selections"),
            "lista vazia não é gravada: {json}"
        );
    }

    /// As queries de seleção sobrevivem à ida e volta, com a ordem preservada.
    ///
    /// CONTRACT: IF-229
    #[test]
    fn selection_queries_survive_the_round_trip() {
        let selections = vec![
            SelectionQuery {
                name: "Produção".to_string(),
                server: "configurador".to_string(),
                database: "configurador".to_string(),
                sql: "select host, port, database from tenants".to_string(),
            },
            SelectionQuery {
                name: "Homolog".to_string(),
                server: "Local".to_string(),
                database: "postgres".to_string(),
                sql: "select * from tenants where ambiente = 'homolog'".to_string(),
            },
        ];
        let prefs = GuiPrefs {
            selections: selections.clone(),
            ..GuiPrefs::default()
        };

        let json = serde_json::to_string_pretty(&prefs).expect("serializes");
        assert!(json.contains("\"selections\""), "a lista entra: {json}");

        let back: GuiPrefs = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.selections, selections);
    }

    /// Arquivo antigo (sem a chave) continua legível: a lista nasce vazia.
    ///
    /// CONTRACT: IF-229
    #[test]
    fn an_old_file_without_selections_still_loads() {
        let json = r#"{ "interfaceFont": "Segoe UI", "fontScale": 1.1 }"#;
        let prefs: GuiPrefs = serde_json::from_str(json).expect("deserializes");
        assert!(prefs.selections.is_empty());
        assert_eq!(prefs.interface_font.as_deref(), Some("Segoe UI"));
    }
}
