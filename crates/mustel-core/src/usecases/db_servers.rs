//! `settings db-servers` como casos de uso do núcleo (T40).
//!
//! São comandos que **não têm progresso nem região viva**: cada um faz uma coisa e devolve o que
//! aconteceu (a lista, ou o erro já com o texto do contrato). Por isso a assinatura aqui é
//! `Result<T>` e não `EventSink` — o evento não teria o que reportar no meio, e a GUI (T42) quer o
//! valor, não um fluxo de mensagens.
//!
//! O que **fica no frontend**: os prompts (nome, host, porta, seleção, menu de ação) e todo o texto
//! de stdout (`Server 'x' added successfully.`, a tabela do `ls`). O que está aqui é a regra que o CLI
//! e a GUI precisam dividir: validação, recusa de nome duplicado, persistência, cofre e o teste de
//! conexão.

use crate::config::{ConfigStore, ServerConfigEntry};
use crate::credentials::{CredentialStore, SystemKeyring};
use crate::error::CoreError;
use crate::postgres::client::{connect, list_databases};
use crate::postgres::settings::ConnectionSettings;
use crate::validation::{is_valid_host, is_valid_username};

/// Servidores cadastrados, na ordem do arquivo.
pub fn list(store: &ConfigStore) -> Result<Vec<ServerConfigEntry>, CoreError> {
    Ok(store.get_servers()?)
}

/// Cadastra um servidor: valida, recusa nome repetido e grava.
///
/// As checagens e seus exit codes são os do legado: `Invalid host`/`Invalid username` saem com
/// **41**, e o nome já existente com **40** (o legado recusa em vez de atualizar, mesmo existindo
/// `add_or_update` no núcleo). O usuário só é validado quando **não** é branco — é o
/// `!string.IsNullOrWhiteSpace(Username) && !IsValidUsername(...)` do legado, que aceita um
/// servidor sem usuário definido. A **porta** não é checada aqui: quem tem o valor cru da flag (e
/// portanto pode dizer qual número foi recusado) é o frontend, que já entrega a entrada num `u16`.
pub fn add(store: &ConfigStore, server: &ServerConfigEntry) -> Result<(), CoreError> {
    if !is_valid_host(&server.host) {
        return Err(CoreError::InvalidArgument(format!(
            "Invalid host: '{}'.",
            server.host
        )));
    }
    if !server.username.trim().is_empty() && !is_valid_username(&server.username) {
        return Err(CoreError::InvalidArgument(format!(
            "Invalid username: '{}'.",
            server.username
        )));
    }

    if store.get_server(&server.name)?.is_some() {
        return Err(CoreError::InvalidState(format!(
            "Server '{}' already exists. Use a different name or remove the existing one first.",
            server.name
        )));
    }

    store.add_or_update_server(server)?;

    Ok(())
}

/// Grava um servidor vindo da tela de edição: cadastra, atualiza **ou renomeia**.
///
/// `original` é o nome que o servidor tinha quando a tela abriu o formulário (`None` = cadastro novo).
/// A distinção importa porque renomear **não** é `add` de um nome novo: o registro antigo tem de sair,
/// e a senha precisa **mudar de chave** no cofre — a chave é o nome do servidor (D6). Sem isso a
/// edição do nome duplicaria o servidor e orfanaria a senha.
///
/// As validações são as do [`add`]. Renomear para um nome já ocupado é recusado (exit 40 no CLI) em
/// vez de sobrescrever.
pub fn save(
    store: &ConfigStore,
    original: Option<&str>,
    server: &ServerConfigEntry,
) -> Result<(), CoreError> {
    if !is_valid_host(&server.host) {
        return Err(CoreError::InvalidArgument(format!(
            "Invalid host: '{}'.",
            server.host
        )));
    }
    if !server.username.trim().is_empty() && !is_valid_username(&server.username) {
        return Err(CoreError::InvalidArgument(format!(
            "Invalid username: '{}'.",
            server.username
        )));
    }

    let Some(original) = original else {
        // Cadastro novo: as regras do `add` (nome livre).
        return add(store, server);
    };

    // A edição só vale sobre um servidor que existe.
    if store.get_server(original)?.is_none() {
        return Err(CoreError::InvalidArgument(format!(
            "Server '{original}' not found."
        )));
    }

    let renamed = !original.eq_ignore_ascii_case(&server.name);
    if renamed {
        if store.get_server(&server.name)?.is_some() {
            return Err(CoreError::InvalidState(format!(
                "Server '{}' already exists. Use a different name or remove the existing one first.",
                server.name
            )));
        }

        store.remove_server(original)?;
    }

    store.add_or_update_server(server)?;

    if renamed {
        // A senha guardada precisa acompanhar o nome novo, senão fica órfã no cofre.
        move_password(original, &server.name);
    }

    Ok(())
}

/// Leva a senha guardada de um nome para outro, quando existe.
///
/// Falha de cofre não impede a edição: o cadastro já está gravado, e o pior caso é a senha ficar sob o
/// nome antigo (recuperável com `set-password`).
fn move_password(from: &str, to: &str) {
    let keyring = SystemKeyring::new();
    if let Ok(Some(password)) = keyring.get(from)
        && keyring.set(to, &password).is_ok()
    {
        let _ = keyring.delete(from);
    }
}

/// Remove um servidor e esquece a senha dele.
///
/// Nome inexistente é **erro** (41), com o texto do legado. Apagar a credencial do cofre é adição
/// do Mustel (D6): o legado guardava o blob no próprio arquivo, que ia embora junto.
pub fn remove(store: &ConfigStore, name: &str) -> Result<(), CoreError> {
    if !store.remove_server(name)? {
        return Err(CoreError::InvalidArgument(format!(
            "Server '{name}' not found."
        )));
    }

    forget_password(name);

    Ok(())
}

/// Guarda a senha de um servidor no cofre do sistema (D6).
///
/// O servidor precisa existir (41 quando não existe), como no legado.
pub fn set_password(store: &ConfigStore, name: &str, password: &str) -> Result<(), CoreError> {
    if store.get_server(name)?.is_none() {
        return Err(CoreError::InvalidArgument(format!(
            "Server '{name}' not found."
        )));
    }

    store_password(name, password)
}

/// O que o teste de conexão descobriu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReport {
    /// Bancos acessíveis, como o `list_databases` devolve.
    pub databases: Vec<String>,
}

/// Testa a conexão de um servidor cadastrado, com a senha já resolvida pelo frontend.
///
/// O legado embrulha qualquer falha em `InvalidOperationException` — exit **40**, com prefixo
/// `Authentication failed:` quando a mensagem cheira a senha e `Connection failed:` nos demais casos.
/// Esse embrulho é regra, não formatação: é o que define o exit code.
pub async fn test_connection(
    server: &ServerConfigEntry,
    password: &str,
) -> Result<TestReport, CoreError> {
    let settings = ConnectionSettings {
        host: server.host.clone(),
        port: server.port,
        user: server.username.clone(),
        password: Some(password.to_string()),
        database: Some("postgres".to_string()),
        ssl_mode: server.ssl_mode.parse().unwrap_or_default(),
        connect_timeout: std::time::Duration::from_secs(u64::from(server.timeout)),
        ..ConnectionSettings::default()
    };

    let client = connect(&settings)
        .await
        .map_err(|error| wrap_connection_error(&error))?;
    let databases = list_databases(&client)
        .await
        .map_err(|error| wrap_connection_error(&error))?;

    Ok(TestReport { databases })
}

/// Aplica o embrulho do legado: `Authentication failed:` vs `Connection failed:` → exit 40.
fn wrap_connection_error(error: &CoreError) -> CoreError {
    let message = error.to_string();
    let lowercase = message.to_ascii_lowercase();
    let looks_like_auth = lowercase.contains("authentication") || lowercase.contains("password");

    let prefix = if looks_like_auth {
        "Authentication failed"
    } else {
        "Connection failed"
    };

    CoreError::InvalidState(format!("{prefix}: {message}"))
}

/// Guarda a senha no cofre do sistema (D6).
fn store_password(server: &str, password: &str) -> Result<(), CoreError> {
    SystemKeyring::new()
        .set(server, password)
        .map_err(|error| CoreError::InvalidState(error.to_string()))
}

/// Apaga a senha guardada, sem impedir a remoção do cadastro.
///
/// Sem cofre disponível não há o que limpar; um segredo órfão não afeta o cadastro, e falhar aqui
/// mudaria a saída de um comando que o usuário já concluiu.
fn forget_password(server: &str) {
    let _ = SystemKeyring::new().delete(server);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::exit_code;

    fn store() -> (tempfile::TempDir, ConfigStore) {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(directory.path().join("mustel.jsonc"));

        (directory, store)
    }

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            databases: vec!["mydb".to_string()],
            ..ServerConfigEntry::default()
        }
    }

    #[test]
    fn adding_persists_the_server_and_then_lists_it() {
        let (_directory, store) = store();

        add(&store, &server("dev")).expect("cadastra");

        let servers = list(&store).expect("lista");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "dev");
    }

    #[test]
    /// CONTRACT: IF-116
    fn a_repeated_name_is_refused_with_exit_40() {
        let (_directory, store) = store();
        add(&store, &server("dev")).expect("cadastra");

        let error = add(&store, &server("DEV")).expect_err("recusa");
        assert_eq!(
            error.to_string(),
            "Server 'DEV' already exists. Use a different name or remove the existing one first."
        );
        assert_eq!(error.exit_code(), exit_code::INVALID_STATE);
    }

    #[test]
    fn the_validation_texts_and_codes_are_the_legacy_ones() {
        let (_directory, store) = store();

        let invalid_host = ServerConfigEntry {
            host: "".to_string(),
            ..server("dev")
        };
        let error = add(&store, &invalid_host).expect_err("recusa host vazio");
        assert_eq!(error.to_string(), "Invalid host: ''.");
        assert_eq!(error.exit_code(), exit_code::INVALID_ARGUMENT);

        let invalid_username = ServerConfigEntry {
            username: "nome com espaço".to_string(),
            ..server("dev")
        };
        let error = add(&store, &invalid_username).expect_err("recusa usuário inválido");
        assert_eq!(error.to_string(), "Invalid username: 'nome com espaço'.");
        assert_eq!(error.exit_code(), exit_code::INVALID_ARGUMENT);
    }

    #[test]
    /// CONTRACT: IF-118
    fn a_server_without_username_is_accepted_like_in_the_legacy() {
        let (_directory, store) = store();
        let anonymous = ServerConfigEntry {
            username: String::new(),
            ..server("sem-usuario")
        };

        add(&store, &anonymous).expect("o legado só valida usuário quando ele não é branco");
        assert_eq!(list(&store).expect("lista")[0].username, "");
    }

    #[test]
    fn editing_in_place_keeps_the_same_entry() {
        let (_directory, store) = store();
        add(&store, &server("dev")).expect("cadastra");
        add(&store, &server("outro")).expect("cadastra");

        let edited = ServerConfigEntry {
            port: 6543,
            host: "db.interno".to_string(),
            ..server("dev")
        };
        save(&store, Some("dev"), &edited).expect("atualiza");

        let servers = list(&store).expect("lista");
        assert_eq!(servers.len(), 2, "os outros servidores ficam");
        let dev = servers.iter().find(|s| s.name == "dev").expect("dev");
        assert_eq!(dev.port, 6543);
        assert_eq!(dev.host, "db.interno");

        let error = save(&store, Some("fantasma"), &server("fantasma")).expect_err("recusa");
        assert_eq!(error.to_string(), "Server 'fantasma' not found.");
    }

    #[test]
    fn renaming_moves_the_entry_instead_of_creating_another() {
        let (_directory, store) = store();
        add(&store, &server("antes")).expect("cadastra");

        let renamed = ServerConfigEntry {
            name: "depois".to_string(),
            ..server("antes")
        };
        save(&store, Some("antes"), &renamed).expect("renomeia");

        let servers = list(&store).expect("lista");
        assert_eq!(
            servers.len(),
            1,
            "renomear substitui o registro, não cria outro: {servers:?}"
        );
        assert_eq!(servers[0].name, "depois");
    }

    #[test]
    fn renaming_onto_an_existing_name_is_refused() {
        let (_directory, store) = store();
        add(&store, &server("um")).expect("cadastra");
        add(&store, &server("dois")).expect("cadastra");

        let colliding = ServerConfigEntry {
            name: "dois".to_string(),
            ..server("um")
        };
        let error = save(&store, Some("um"), &colliding).expect_err("recusa");

        assert_eq!(
            error.to_string(),
            "Server 'dois' already exists. Use a different name or remove the existing one first."
        );
        assert_eq!(list(&store).expect("lista").len(), 2, "nada foi tocado");
    }

    #[test]
    fn creating_with_a_taken_name_is_refused_like_the_add() {
        let (_directory, store) = store();
        add(&store, &server("novo")).expect("cadastra");

        let error = save(&store, None, &server("novo")).expect_err("recusa");

        assert_eq!(
            error.exit_code(),
            exit_code::INVALID_STATE,
            "cadastro novo continua com a regra do `add` (exit 40)"
        );
    }

    /// O rename mexe no cofre de verdade (a chave é o nome): só roda com `MUSTEL_KEYRING_TESTS=1`.
    ///
    /// É a parte que mais pode dar errado em silêncio — se o segredo não migrar, a edição do nome deixa
    /// a senha órfã sob o nome antigo, e o `Test` volta a pedir senha.
    #[test]
    fn renaming_moves_the_stored_password_to_the_new_name() {
        use crate::credentials::{CredentialStore, SystemKeyring};

        if std::env::var("MUSTEL_KEYRING_TESTS").as_deref() != Ok("1") {
            eprintln!("skipped: set MUSTEL_KEYRING_TESTS=1 to exercise the real keyring");
            return;
        }

        let (_directory, store) = store();
        let keyring = SystemKeyring::new();
        let before = "mustel-test-rename-before";
        let after = "mustel-test-rename-after";
        let _ = keyring.delete(before);
        let _ = keyring.delete(after);

        add(&store, &server(before)).expect("cadastra");
        keyring.set(before, "segredo-do-teste").expect("guarda");

        let renamed = ServerConfigEntry {
            name: after.to_string(),
            ..server(before)
        };
        save(&store, Some(before), &renamed).expect("renomeia");

        assert_eq!(
            keyring.get(after).expect("get depois"),
            Some("segredo-do-teste".to_string()),
            "a senha acompanha o nome novo"
        );
        assert_eq!(
            keyring.get(before).expect("get antes"),
            None,
            "e não fica órfã sob o nome antigo"
        );

        let _ = keyring.delete(before);
        let _ = keyring.delete(after);
    }

    #[test]
    fn removing_a_server_that_is_not_there_is_an_error() {
        let (_directory, store) = store();

        let error = remove(&store, "nao-existe").expect_err("recusa");
        assert_eq!(error.to_string(), "Server 'nao-existe' not found.");
        assert_eq!(error.exit_code(), exit_code::INVALID_ARGUMENT);
    }

    #[test]
    fn setting_the_password_of_a_missing_server_is_an_error_before_touching_the_vault() {
        let (_directory, store) = store();

        let error = set_password(&store, "nao-existe", "segredo").expect_err("recusa");
        assert_eq!(error.to_string(), "Server 'nao-existe' not found.");
    }

    #[test]
    fn error_wrapping_matches_the_legacy_prefixes() {
        let auth = CoreError::Postgres {
            sqlstate: Some("28P01".to_string()),
            message: "password authentication failed for user \"postgres\"".to_string(),
        };
        let wrapped = wrap_connection_error(&auth);
        assert!(wrapped.to_string().starts_with("Authentication failed:"));
        assert_eq!(wrapped.exit_code(), exit_code::INVALID_STATE);

        let refused = CoreError::Postgres {
            sqlstate: None,
            message: "connection refused".to_string(),
        };
        let wrapped = wrap_connection_error(&refused);
        assert!(wrapped.to_string().starts_with("Connection failed:"));
    }
}
