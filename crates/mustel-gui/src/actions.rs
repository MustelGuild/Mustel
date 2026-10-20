//! Execução dos pedidos da tela, **sem** tokio, canal ou janela (T42/T43).
//!
//! É a costura testável: cada função aqui recebe o [`ConfigStore`] (e o pedido já resolvido) e
//! devolve o [`JobResult`] que a tela espera. O [`App`](crate::app::App) só a chama dentro do runtime
//! de background e manda o resultado pelo canal; assim o fluxo (cadastrar, editar, remover, senha)
//! pode ser exercitado num teste, com o cadastro num diretório temporário — sem mexer em variável de
//! ambiente do processo, que é global e quebraria os testes que rodam em paralelo.

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::usecases::db_servers;

use crate::request::JobResult;

/// Lê o cadastro do disco.
pub fn load_servers(store: &ConfigStore) -> JobResult {
    match db_servers::list(store) {
        Ok(servers) => JobResult::Servers(servers),
        Err(error) => JobResult::Failed(error.to_string()),
    }
}

/// Grava um servidor: `update` quando ele já existe, `add` quando é novo.
///
/// A escolha entre os dois é do **frontend** por um motivo de contrato: o `add` recusa nome repetido
/// (exit 40 no CLI, que é o comportamento do legado) e o `update` é a edição da GUI, que preserva a
/// senha do cofre. Quem sabe qual dos dois o usuário pediu é a tela — ela manda o `Save` com o
/// servidor completo.
pub fn save_server(store: &ConfigStore, server: &ServerConfigEntry) -> JobResult {
    let outcome = db_servers::list(store).and_then(|current| {
        let exists = current
            .iter()
            .any(|entry| entry.name.eq_ignore_ascii_case(&server.name));
        if exists {
            db_servers::update(store, server)
        } else {
            db_servers::add(store, server)
        }
    });

    match outcome {
        Ok(()) => JobResult::Saved(server.name.clone()),
        Err(error) => JobResult::Failed(error.to_string()),
    }
}

/// Remove um servidor do cadastro (e do cofre).
pub fn remove_server(store: &ConfigStore, name: &str) -> JobResult {
    match db_servers::remove(store, name) {
        Ok(()) => JobResult::Removed(name.to_string()),
        Err(error) => JobResult::Failed(error.to_string()),
    }
}

/// Guarda a senha de um servidor no cofre.
pub fn save_password(store: &ConfigStore, name: &str, password: &str) -> JobResult {
    match db_servers::set_password(store, name, password) {
        Ok(()) => JobResult::PasswordSaved(name.to_string()),
        Err(error) => JobResult::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn saving_a_new_server_adds_it_and_saving_it_again_updates_it() {
        let (_directory, store) = store();

        assert_eq!(
            save_server(&store, &server("novo")),
            JobResult::Saved("novo".to_string())
        );

        // Segundo `Save` com o mesmo nome: é edição, não erro de duplicata.
        let edited = ServerConfigEntry {
            port: 6543,
            ..server("novo")
        };
        assert_eq!(
            save_server(&store, &edited),
            JobResult::Saved("novo".to_string()),
            "editar não pode cair no `add`, que recusaria o nome repetido"
        );

        let JobResult::Servers(servers) = load_servers(&store) else {
            panic!("esperava a lista");
        };
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].port, 6543, "a edição foi gravada");

        assert_eq!(
            remove_server(&store, "novo"),
            JobResult::Removed("novo".to_string())
        );
        assert_eq!(
            remove_server(&store, "fantasma"),
            JobResult::Failed("Server 'fantasma' not found.".to_string())
        );
    }

    #[test]
    fn a_save_with_an_invalid_host_reports_the_core_message() {
        let (_directory, store) = store();
        let invalid = ServerConfigEntry {
            host: String::new(),
            ..server("x")
        };

        assert_eq!(
            save_server(&store, &invalid),
            JobResult::Failed("Invalid host: ''.".to_string())
        );
    }
}
