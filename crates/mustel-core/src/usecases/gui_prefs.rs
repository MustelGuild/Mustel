//! Preferências da janela (aba **Settings**), gravadas na seção `gui` do `mustel.jsonc`.
//!
//! Caso de uso fino e sem UI: lê e grava **só** a sua seção, deixando o resto do arquivo como está.
//! É o que evita que salvar a fonte mexa nos servidores — e o inverso também vale, porque o CLI
//! reescreve o modelo inteiro e a seção `gui` viaja junto.

use crate::config::ConfigStore;
use crate::config::model::{GuiPrefs, SavedQuery};
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

/// Grava ou substitui uma **query salva** na lista, por nome.
///
/// O nome é aparado; vazio (ou só espaços) é recusado com `InvalidArgument`. A comparação é
/// **sem diferenciar maiúsculas** (`Faturamento` e `faturamento` são a mesma query) e a substituição
/// mantém a **ordem** da lista, para a posição no combo não dançar a cada salvamento.
///
pub fn upsert_saved_query(
    list: &mut Vec<SavedQuery>,
    name: &str,
    sql: &str,
) -> Result<bool, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::InvalidArgument(
            "the query name cannot be empty".to_string(),
        ));
    }

    match list
        .iter_mut()
        .find(|saved| saved.name.eq_ignore_ascii_case(name))
    {
        Some(existing) => {
            // Substitui o conteúdo e adota a grafia nova do nome (o dono pode ter corrigido a caixa).
            let changed = existing.sql != sql || existing.name != name;
            existing.name = name.to_string();
            existing.sql = sql.to_string();
            Ok(changed)
        }
        None => {
            list.push(SavedQuery {
                name: name.to_string(),
                sql: sql.to_string(),
            });
            Ok(true)
        }
    }
}

/// Remove a query salva pelo nome (sem diferenciar maiúsculas).
///
/// Devolve `true` quando removeu algo. Exclusão é **definitiva** (`D10`): não há lixeira, e a
/// confirmação acontece na tela.
pub fn remove_saved_query(list: &mut Vec<SavedQuery>, name: &str) -> bool {
    let name = name.trim();
    let before = list.len();
    list.retain(|saved| !saved.name.eq_ignore_ascii_case(name));
    list.len() != before
}

/// A query salva com aquele nome, se existir.
pub fn saved_query<'a>(list: &'a [SavedQuery], name: &str) -> Option<&'a SavedQuery> {
    list.iter()
        .find(|saved| saved.name.eq_ignore_ascii_case(name.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{GuiTheme, MAX_FONT_SCALE, SelectionQuery, ServerConfigEntry};

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
                theme: GuiTheme::Light,
                selections: Vec::new(),
                saved_queries: Vec::new(),
                selection_list: Default::default(),
                ai: Default::default(),
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

    /// O CLI grava um servidor e as queries de seleção continuam no arquivo (D11).
    ///
    /// A seção `gui` viaja no modelo inteiro: quem reescreve o arquivo tem de preservá-la.
    /// CONTRACT: IF-229
    #[test]
    fn a_cli_rewrite_keeps_the_selection_queries() {
        let (_directory, store) = store();
        let query = SelectionQuery {
            name: "Produção".to_string(),
            server: "configurador".to_string(),
            database: "configurador".to_string(),
            sql: "select host, port, database from tenants".to_string(),
        };
        save(
            &store,
            Some(GuiPrefs {
                selections: vec![query.clone()],
                ..GuiPrefs::default()
            }),
        )
        .expect("salva as queries");

        // `settings db-servers add` reescreve o arquivo inteiro.
        crate::usecases::db_servers::save(&store, None, &server("dev")).expect("salva servidor");

        let config = store.load().expect("lê");
        assert_eq!(config.servers.len(), 1);
        assert_eq!(
            config.gui.expect("seção gui").selections,
            vec![query],
            "a query sobrevive à reescrita do CLI"
        );
    }

    /// Salvar cria, e salvar de novo substitui **sem duplicar** nem mudar a ordem.
    /// CONTRACT: IF-244
    #[test]
    fn saving_a_query_creates_then_replaces_in_place() {
        let mut list = Vec::new();

        assert!(upsert_saved_query(&mut list, "Faturamento", "select 1").expect("salva"));
        assert!(upsert_saved_query(&mut list, "Auditoria", "select 2").expect("salva"));
        assert_eq!(list.len(), 2);

        assert!(upsert_saved_query(&mut list, "Faturamento", "select 3").expect("substitui"));

        assert_eq!(list.len(), 2, "substituir não duplica");
        assert_eq!(list[0].name, "Faturamento", "a posição fica onde estava");
        assert_eq!(list[0].sql, "select 3");
    }

    /// O nome é único **sem diferenciar maiúsculas** e o nome é aparado.
    /// CONTRACT: IF-244
    #[test]
    fn the_name_is_case_insensitive_and_trimmed() {
        let mut list = Vec::new();
        upsert_saved_query(&mut list, "Faturamento", "select 1").expect("salva");

        upsert_saved_query(&mut list, "  faturamento  ", "select 2").expect("substitui");

        assert_eq!(list.len(), 1, "caixa e espaços não criam outra query");
        assert_eq!(list[0].name, "faturamento", "a grafia nova vale");
        assert_eq!(list[0].sql, "select 2");
    }

    /// Nome vazio ou só de espaços é recusado.
    /// CONTRACT: IF-244
    #[test]
    fn an_empty_name_is_refused() {
        let mut list = Vec::new();

        for name in ["", "   ", "\t\n"] {
            let error = upsert_saved_query(&mut list, name, "select 1")
                .expect_err("nome vazio precisa falhar");
            assert!(
                matches!(error, CoreError::InvalidArgument(_)),
                "erro de argumento, não de outro tipo: {error:?}"
            );
        }
        assert!(list.is_empty(), "nada foi gravado");
    }

    /// Remover tira da lista; remover o que não existe não explode.
    /// CONTRACT: IF-244
    #[test]
    fn removing_a_query_is_definitive_and_idempotent() {
        let mut list = vec![SavedQuery {
            name: "Faturamento".to_string(),
            sql: "select 1".to_string(),
        }];

        assert!(
            remove_saved_query(&mut list, "faturamento"),
            "caixa não importa"
        );
        assert!(list.is_empty());
        assert!(
            !remove_saved_query(&mut list, "faturamento"),
            "já não existe"
        );
    }

    /// A lista sobrevive à reescrita do arquivo pelo CLI, ao lado das `selections`.
    /// CONTRACT: IF-244
    #[test]
    fn saved_queries_survive_a_cli_rewrite() {
        let (_directory, store) = store();
        let mut list = Vec::new();
        upsert_saved_query(&mut list, "Faturamento", "select 1").expect("salva");

        save(
            &store,
            Some(GuiPrefs {
                saved_queries: list.clone(),
                ..GuiPrefs::default()
            }),
        )
        .expect("grava");
        crate::usecases::db_servers::save(&store, None, &server("dev")).expect("salva servidor");

        let config = store.load().expect("lê");
        assert_eq!(config.servers.len(), 1);
        assert_eq!(
            config.gui.expect("seção gui").saved_queries,
            list,
            "as queries salvas sobrevivem à reescrita do CLI"
        );
    }
}
