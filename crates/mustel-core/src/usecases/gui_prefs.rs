//! Preferências da janela (aba **Settings**), gravadas na seção `gui` do `mustel.jsonc`.
//!
//! Caso de uso fino e sem UI: lê e grava **só** a sua seção, deixando o resto do arquivo como está.
//! É o que evita que salvar a fonte mexa nos servidores — e o inverso também vale, porque o CLI
//! reescreve o modelo inteiro e a seção `gui` viaja junto.

use crate::config::ConfigStore;
use crate::config::model::GuiPrefs;
use crate::error::CoreError;

/// Lê as preferências gravadas (`None` = nunca configuradas).
pub fn load(store: &ConfigStore) -> Result<Option<GuiPrefs>, CoreError> {
    let config = store.load()?;
    Ok(config.gui)
}

/// Grava as preferências preservando o resto do arquivo.
///
/// `None` remove a seção: é o que o botão "Restaurar padrão" usa para o arquivo voltar a ser
/// exatamente o que era antes de alguém mexer na fonte.
pub fn save(store: &ConfigStore, prefs: Option<GuiPrefs>) -> Result<(), CoreError> {
    let mut config = store.load()?;
    config.gui = prefs;
    store.save(&config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{MAX_FONT_SCALE, ServerConfigEntry};

    fn store() -> (tempfile::TempDir, ConfigStore) {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(directory.path().join("mustel.jsonc"));
        (directory, store)
    }

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: "localhost".to_string(),
            username: "postgres".to_string(),
            ..ServerConfigEntry::default()
        }
    }

    /// Salvar a fonte **não** mexe nos servidores; salvar um servidor **preserva** a fonte.
    ///
    /// As duas direções importam: a janela grava a seção `gui` e o CLI reescreve o arquivo inteiro.
    /// CONTRACT: IF-218
    #[test]
    fn saving_preferences_keeps_the_servers_and_vice_versa() {
        let (_directory, store) = store();
        crate::usecases::db_servers::save(&store, None, &server("dev")).expect("salva servidor");

        save(
            &store,
            Some(GuiPrefs {
                interface_font: Some("Segoe UI".to_string()),
                editor_font: Some("Fira Code".to_string()),
                font_scale: 1.1,
            }),
        )
        .expect("salva preferências");

        let config = store.load().expect("lê");
        assert_eq!(config.servers.len(), 1, "o servidor continua lá");
        assert_eq!(
            config
                .gui
                .expect("preferências gravadas")
                .interface_font
                .as_deref(),
            Some("Segoe UI")
        );

        // Agora o CLI grava um servidor novo: a seção `gui` tem de sobreviver.
        crate::usecases::db_servers::save(&store, None, &server("outro")).expect("salva outro");
        let config = store.load().expect("lê de novo");
        assert_eq!(config.servers.len(), 2);
        assert!(
            config.gui.is_some(),
            "o CLI preserva as preferências da janela"
        );
    }

    /// Gravar `None` remove a seção — o arquivo volta ao estado de antes.
    ///
    /// CONTRACT: IF-218
    #[test]
    fn clearing_the_preferences_removes_the_section() {
        let (_directory, store) = store();
        save(
            &store,
            Some(GuiPrefs {
                font_scale: MAX_FONT_SCALE,
                ..GuiPrefs::default()
            }),
        )
        .expect("salva");
        assert!(store.load().expect("lê").gui.is_some());

        save(&store, None).expect("limpa");
        assert!(
            store.load().expect("lê").gui.is_none(),
            "sem preferências, sem seção"
        );

        let text = std::fs::read_to_string(store.path()).expect("lê arquivo");
        assert!(!text.contains("\"gui\""), "nem a chave aparece: {text}");
    }
}
