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
/// Escala máxima aceita; acima disso o rótulo de status mais longo disputa espaço com os combos.
pub const MAX_FONT_SCALE: f32 = 1.25;

/// Tema da janela.
///
/// `System` é o padrão e reproduz o comportamento anterior a esta preferência existir: o `egui` abre
/// seguindo o tema do sistema operacional. `Light` e `Dark` fixam a escolha do usuário.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuiTheme {
    /// Segue o tema do sistema operacional.
    #[default]
    System,
    /// Sempre claro.
    Light,
    /// Sempre escuro.
    Dark,
}

impl GuiTheme {
    /// `true` quando é o padrão — é o que **omite** a chave do arquivo, como o resto de `gui`.
    pub fn is_default(&self) -> bool {
        *self == Self::System
    }
}

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
    /// Tema da janela. Ausente = `System` (segue o sistema operacional).
    ///
    /// Só aparece no arquivo quando o usuário **não** está em `System`, pela mesma razão das outras
    /// chaves de `gui`: quem não mexeu não vê o arquivo mudar.
    #[serde(skip_serializing_if = "GuiTheme::is_default")]
    pub theme: GuiTheme,
    /// Queries salvas que a aba **Query** usa para marcar bases em massa.
    ///
    /// Fica fora do JSON enquanto a lista está vazia, pela mesma razão da seção `gui`: quem não usa
    /// a seleção por query não vê o arquivo mudar.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub selections: Vec<SelectionQuery>,
    /// Queries salvas para **uso recorrente**: escolher uma carrega o SQL no editor (aba **Query**).
    ///
    /// Conceito separado de `selections`: aqui não há servidor nem banco, e o texto vai para o
    /// editor — não é executado para descobrir bases. Ausente do JSON enquanto vazia.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub saved_queries: Vec<SavedQuery>,
    /// Como a lista de bases da aba **Query** é desenhada (paginação).
    ///
    /// Ausente = **sem paginação** (desenha todas as bases). Só aparece no arquivo quando alguém
    /// mexe, como o resto de `gui`.
    #[serde(skip_serializing_if = "SelectionListPrefs::is_default")]
    pub selection_list: SelectionListPrefs,
    /// Integração com a IA que escreve SQL (painel da aba **Query**).
    ///
    /// Ausente do arquivo enquanto não houver perfil: quem não usa a IA não vê o `mustel.jsonc`
    /// mudar. A **chave** nunca mora aqui (vai para o cofre).
    #[serde(skip_serializing_if = "AiPrefs::is_default")]
    pub ai: AiPrefs,
}

/// Temperatura padrão do modelo: baixa, para SQL estável sem travar o modelo.
pub const DEFAULT_AI_TEMPERATURE: f32 = 0.1;

/// Faixa aceita de temperatura (a mesma dos provedores OpenAI-compatible).
pub const AI_TEMPERATURE_RANGE: std::ops::RangeInclusive<f32> = 0.0..=2.0;

/// A integração de IA: a lista de perfis e qual está ativo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiPrefs {
    /// Perfis configurados (nome, endpoint, modelo, temperatura).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<AiProfile>,
    /// `id` do perfil em uso. Vazio quando nenhum foi escolhido.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub active_profile: String,
}

impl AiPrefs {
    /// É o padrão? (o que fica fora do arquivo: sem perfis).
    pub fn is_default(&self) -> bool {
        self.profiles.is_empty() && self.active_profile.is_empty()
    }

    /// O perfil ativo, se ele existir na lista.
    ///
    /// Se o `active_profile` apontar para um perfil removido à mão, cai no **primeiro** — melhor
    /// usar um perfil do que nenhum, e a tela mostra qual foi escolhido.
    pub fn active(&self) -> Option<&AiProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.id == self.active_profile)
            .or_else(|| self.profiles.first())
    }
}

/// Um provedor OpenAI-compatible configurado (`/chat/completions`).
///
/// O `id` é **estável** e é a conta no cofre (`mustel.ai`): renomear o perfil na tela não perde a
/// chave. `name` é só o rótulo que aparece no combo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiProfile {
    /// Identificador estável (conta do cofre).
    pub id: String,
    /// Rótulo mostrado na janela.
    pub name: String,
    /// Base do endpoint, sem `/chat/completions`.
    pub base_url: String,
    /// Modelo pedido no corpo.
    pub model: String,
    /// Temperatura do pedido (D26). Ausente do arquivo quando é o padrão.
    #[serde(skip_serializing_if = "is_default_ai_temperature")]
    pub temperature: f32,
}

impl Default for AiProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: String::new(),
            temperature: DEFAULT_AI_TEMPERATURE,
        }
    }
}

impl AiProfile {
    /// A temperatura, cortada na faixa aceita.
    ///
    /// Arquivo editado à mão (ou `NaN`, se alguém escreveu lixo) não pode virar pedido inválido:
    /// acima de `2.0` corta em `2.0`, abaixo de `0.0` corta em `0.0`, `NaN` volta ao padrão.
    pub fn clamped_temperature(&self) -> f32 {
        if self.temperature.is_nan() {
            return DEFAULT_AI_TEMPERATURE;
        }
        self.temperature
            .clamp(*AI_TEMPERATURE_RANGE.start(), *AI_TEMPERATURE_RANGE.end())
    }
}

/// `true` quando a temperatura é o padrão (é o que omite a chave do arquivo).
fn is_default_ai_temperature(value: &f32) -> bool {
    (*value - DEFAULT_AI_TEMPERATURE).abs() < f32::EPSILON
}

/// Paginação da lista de bases (aba **Query**).
///
/// Desligada por padrão: com os filtros da tela, a lista completa é utilizável, e paginar sem
/// necessidade esconde informação. Ligada, cada servidor/todos são revelados em pedaços de
/// `page_size` com um botão no fim da lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SelectionListPrefs {
    /// A lista é paginada?
    pub paginate: bool,
    /// Quantas bases por vez quando paginada.
    pub page_size: usize,
}

/// Tamanho de página padrão.
pub const DEFAULT_PAGE_SIZE: usize = 50;
/// Faixa aceita para o tamanho da página.
pub const MIN_PAGE_SIZE: usize = 10;
/// Teto do tamanho da página.
pub const MAX_PAGE_SIZE: usize = 500;

impl Default for SelectionListPrefs {
    fn default() -> Self {
        Self {
            paginate: false,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl SelectionListPrefs {
    /// É o padrão? (o que fica fora do arquivo)
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Tamanho de página dentro da faixa aceita.
    ///
    /// O arquivo é editável à mão: um `0` faria a lista não mostrar nada, e um valor absurdo
    /// desfaria a razão de existir a paginação.
    pub fn clamped_page_size(&self) -> usize {
        self.page_size.clamp(MIN_PAGE_SIZE, MAX_PAGE_SIZE)
    }
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            interface_font: None,
            editor_font: None,
            font_scale: DEFAULT_FONT_SCALE,
            theme: GuiTheme::System,
            selections: Vec::new(),
            saved_queries: Vec::new(),
            selection_list: SelectionListPrefs::default(),
            ai: AiPrefs::default(),
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

/// Uma query salva para **uso recorrente** na aba **Query**.
///
/// Guarda **só o SQL** (decisão `D7`): servidor, banco e diretório de saída são escolhidos na tela a
/// cada execução. Escolher a query no combo **copia o texto para o editor**, onde ele continua
/// editável — é um atalho de digitação, não uma execução pré-configurada.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedQuery {
    /// Nome curto, único **sem diferenciar maiúsculas**. É o rótulo do combo.
    pub name: String,
    /// O SQL que vai para o editor.
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
                theme: GuiTheme::Dark,
                selections: Vec::new(),
                saved_queries: Vec::new(),
                selection_list: Default::default(),
                ai: Default::default(),
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

    /// Sem a chave `theme`, o tema é `System` — o comportamento de antes da preferência existir.
    /// CONTRACT: IF-243
    #[test]
    fn a_config_without_theme_reads_as_system() {
        let json = r#"{ "servers": [], "gui": { "fontScale": 1.1 } }"#;
        let config: UserConfig = serde_json::from_str(json).expect("deserializes");
        let gui = config.gui.expect("a seção existe");

        assert_eq!(gui.theme, GuiTheme::System);
    }

    /// Escolher claro ou escuro sobrevive à ida e volta do arquivo.
    /// CONTRACT: IF-243
    #[test]
    fn the_chosen_theme_round_trips() {
        for chosen in [GuiTheme::Light, GuiTheme::Dark] {
            let config = UserConfig {
                gui: Some(GuiPrefs {
                    theme: chosen,
                    ..GuiPrefs::default()
                }),
                ..UserConfig::default()
            };
            let json = serde_json::to_string_pretty(&config).expect("serializes");
            let back: UserConfig = serde_json::from_str(&json).expect("deserializes");

            assert_eq!(back.gui.expect("a seção existe").theme, chosen, "{json}");
        }
    }

    /// `System` é o padrão e **não** entra no arquivo, como o resto de `gui`.
    /// CONTRACT: IF-243
    #[test]
    fn the_system_theme_stays_out_of_the_file() {
        let prefs = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            ..GuiPrefs::default()
        };
        let json = serde_json::to_string_pretty(&prefs).expect("serializes");

        assert!(
            !json.contains("theme"),
            "System não vai para o arquivo: {json}"
        );
    }

    /// Sem a chave `savedQueries`, a lista começa vazia.
    /// CONTRACT: IF-244
    #[test]
    fn a_config_without_saved_queries_reads_as_an_empty_list() {
        let json = r#"{ "servers": [], "gui": { "fontScale": 1.0 } }"#;
        let config: UserConfig = serde_json::from_str(json).expect("deserializes");

        assert!(config.gui.expect("a seção existe").saved_queries.is_empty());
    }

    /// As queries salvas sobrevivem à ida e volta do arquivo, ao lado das de seleção.
    /// CONTRACT: IF-244
    #[test]
    fn saved_queries_round_trip_next_to_selections() {
        let prefs = GuiPrefs {
            saved_queries: vec![SavedQuery {
                name: "Faturamento por mês".to_string(),
                sql: "select 1".to_string(),
            }],
            selections: vec![SelectionQuery {
                name: "Tenants".to_string(),
                server: "local".to_string(),
                database: "dev".to_string(),
                sql: "select host from tenants".to_string(),
            }],
            ..GuiPrefs::default()
        };

        let json = serde_json::to_string_pretty(&prefs).expect("serializes");
        let back: GuiPrefs = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(back, prefs, "as duas listas convivem no arquivo: {json}");
    }

    /// Lista vazia de queries salvas não aparece no arquivo.
    /// CONTRACT: IF-244
    #[test]
    fn empty_saved_queries_stay_out_of_the_file() {
        let prefs = GuiPrefs {
            interface_font: Some("Segoe UI".to_string()),
            ..GuiPrefs::default()
        };
        let json = serde_json::to_string_pretty(&prefs).expect("serializes");

        assert!(
            !json.contains("savedQueries"),
            "lista vazia não vai para o arquivo: {json}"
        );
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

    /// A seção `ai` só existe no arquivo quando há perfil, e a ida-e-volta preserva tudo.
    /// CONTRACT: IF-247
    #[test]
    fn the_ai_section_round_trips_and_stays_out_when_empty() {
        let empty = GuiPrefs::default();
        let json = serde_json::to_string(&empty).expect("serializa");
        assert!(
            !json.contains("\"ai\""),
            "sem perfil, a seção não aparece: {json}"
        );

        let prefs = GuiPrefs {
            ai: AiPrefs {
                profiles: vec![AiProfile {
                    id: "rapido".to_string(),
                    name: "Rápido".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    temperature: 0.7,
                }],
                active_profile: "rapido".to_string(),
            },
            ..GuiPrefs::default()
        };
        let json = serde_json::to_string(&prefs).expect("serializa");
        assert!(json.contains("\"activeProfile\":\"rapido\""), "{json}");
        assert!(
            json.contains("\"baseUrl\":\"https://api.openai.com/v1\""),
            "{json}"
        );
        assert!(json.contains("\"temperature\":0.7"), "{json}");

        let parsed: GuiPrefs = serde_json::from_str(&json).expect("lê de volta");
        assert_eq!(parsed, prefs);
        assert_eq!(
            parsed.ai.active().map(|profile| profile.id.as_str()),
            Some("rapido")
        );
    }

    /// `temperature` no padrão **não** aparece no arquivo; ausente lê como `0.1`.
    #[test]
    fn the_default_temperature_stays_out_of_the_file() {
        let prefs = GuiPrefs {
            ai: AiPrefs {
                profiles: vec![AiProfile {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    model: "m".to_string(),
                    ..AiProfile::default()
                }],
                active_profile: "a".to_string(),
            },
            ..GuiPrefs::default()
        };
        let json = serde_json::to_string(&prefs).expect("serializa");
        assert!(!json.contains("temperature"), "padrão fica fora: {json}");

        // Ausente lê como o padrão.
        let parsed: GuiPrefs = serde_json::from_str(&json).expect("lê");
        assert_eq!(parsed.ai.profiles[0].temperature, DEFAULT_AI_TEMPERATURE);

        // E um arquivo **sem** a seção inteira continua legível.
        let old = r#"{"fontScale":1.0,"theme":"dark"}"#;
        let parsed: GuiPrefs = serde_json::from_str(old).expect("arquivo antigo");
        assert!(parsed.ai.profiles.is_empty());
    }

    /// Temperatura fora da faixa é cortada na leitura; `NaN` volta ao padrão.
    #[test]
    fn the_temperature_is_clamped_when_read() {
        let temperature = |value: f32| {
            AiProfile {
                temperature: value,
                ..AiProfile::default()
            }
            .clamped_temperature()
        };

        assert_eq!(temperature(0.7), 0.7);
        assert_eq!(temperature(5.0), 2.0, "acima do teto, corta no teto");
        assert_eq!(temperature(-1.0), 0.0, "abaixo do piso, corta no piso");
        assert_eq!(temperature(f32::NAN), DEFAULT_AI_TEMPERATURE);
    }

    /// O perfil ativo que sumiu da lista cai no primeiro (não deixa a IA sem perfil).
    #[test]
    fn a_missing_active_profile_falls_back_to_the_first() {
        let prefs = AiPrefs {
            profiles: vec![
                AiProfile {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    ..AiProfile::default()
                },
                AiProfile {
                    id: "b".to_string(),
                    name: "B".to_string(),
                    ..AiProfile::default()
                },
            ],
            active_profile: "sumiu".to_string(),
        };
        assert_eq!(prefs.active().map(|profile| profile.id.as_str()), Some("a"));

        let none = AiPrefs::default();
        assert!(none.active().is_none());
    }

    /// A seção `selectionList` é lida do arquivo (ida e volta), e ausente = desligada.
    /// CONTRACT: IF-234
    #[test]
    fn the_selection_list_prefs_round_trip() {
        let json = r#"{
            "selections": [],
            "selectionList": { "paginate": true, "pageSize": 80 }
        }"#;
        let prefs: GuiPrefs = serde_json::from_str(json).expect("deserializes");
        assert!(prefs.selection_list.paginate, "leu o `paginate`");
        assert_eq!(prefs.selection_list.page_size, 80, "leu o `pageSize`");

        // Ausente = desligado com o padrão.
        let prefs: GuiPrefs = serde_json::from_str("{}").expect("deserializes");
        assert!(!prefs.selection_list.paginate);
        assert_eq!(prefs.selection_list.page_size, DEFAULT_PAGE_SIZE);

        // Padrão não é gravado; fora do padrão é.
        let plain = serde_json::to_string(&GuiPrefs::default()).expect("serializes");
        assert!(
            !plain.contains("selectionList"),
            "padrão fica fora: {plain}"
        );
        let custom = serde_json::to_string(&GuiPrefs {
            selection_list: SelectionListPrefs {
                paginate: true,
                page_size: 80,
            },
            ..GuiPrefs::default()
        })
        .expect("serializes");
        assert!(custom.contains("\"selectionList\""), "{custom}");
        assert!(custom.contains("\"pageSize\":80"), "{custom}");
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
