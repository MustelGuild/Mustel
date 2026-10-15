//! Senhas de servidor no cofre do sistema (D6).
//!
//! Regras que valem para todo o projeto:
//! - senha **nunca** vai para `mustel.jsonc`, log ou mensagem de erro;
//! - o legado guardava um blob DPAPI no arquivo de config (`encryptedPassword`) e essa migração
//!   **não** é feita: quem tinha senha salva informa de novo (`set-password` ou `-W`);
//! - sem cofre disponível (Linux sem Secret Service, por exemplo) a operação falha com mensagem
//!   clara em vez de degradar para arquivo — ver `docs/pt-BR/credenciais.md`.

use std::collections::HashMap;
use std::sync::Mutex;

/// Serviço usado no cofre do sistema (o "usuário" do segredo é o nome do servidor).
pub const SERVICE_NAME: &str = "mustel";

/// Erros do cofre de credenciais.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    /// O cofre do sistema não está disponível neste ambiente.
    #[error("credential store unavailable for service '{service}': {reason}")]
    Unavailable {
        /// Serviço consultado.
        service: String,
        /// Motivo devolvido pelo backend do cofre.
        reason: String,
    },
    /// O cofre recusou a operação.
    #[error("credential store error for '{service}/{server}': {reason}")]
    Store {
        /// Serviço consultado.
        service: String,
        /// Servidor (chave) da operação.
        server: String,
        /// Motivo devolvido pelo backend.
        reason: String,
    },
}

/// Resultado das operações de credencial.
pub type Result<T> = std::result::Result<T, CredentialError>;

/// Contrato de armazenamento de senhas por servidor.
pub trait CredentialStore: Send + Sync {
    /// Grava (ou substitui) a senha do servidor.
    fn set(&self, server: &str, password: &str) -> Result<()>;
    /// Recupera a senha do servidor, ou `None` quando não há segredo guardado.
    fn get(&self, server: &str) -> Result<Option<String>>;
    /// Remove a senha do servidor; devolve `false` quando não havia nada.
    fn delete(&self, server: &str) -> Result<bool>;
}

/// Implementação real, sobre o cofre do sistema operacional.
///
/// Windows Credential Manager, macOS Keychain e Secret Service (Linux) são usados via `keyring`.
pub struct SystemKeyring {
    service: String,
}

impl SystemKeyring {
    /// Cria o store padrão do Mustel.
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.to_string(),
        }
    }

    /// Cria o store com um nome de serviço diferente (usado em teste).
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// Nome do serviço em uso.
    pub fn service(&self) -> &str {
        &self.service
    }

    fn entry(&self, server: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, server).map_err(|error| self.map_error(server, error))
    }

    fn map_error(&self, server: &str, error: keyring::Error) -> CredentialError {
        match error {
            keyring::Error::NoStorageAccess(inner) => CredentialError::Unavailable {
                service: self.service.clone(),
                reason: inner.to_string(),
            },
            keyring::Error::PlatformFailure(inner) => CredentialError::Unavailable {
                service: self.service.clone(),
                reason: inner.to_string(),
            },
            other => CredentialError::Store {
                service: self.service.clone(),
                server: server.to_string(),
                reason: other.to_string(),
            },
        }
    }
}

impl Default for SystemKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for SystemKeyring {
    fn set(&self, server: &str, password: &str) -> Result<()> {
        self.entry(server)?
            .set_password(password)
            .map_err(|error| self.map_error(server, error))
    }

    fn get(&self, server: &str) -> Result<Option<String>> {
        match self.entry(server)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(self.map_error(server, error)),
        }
    }

    fn delete(&self, server: &str) -> Result<bool> {
        match self.entry(server)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(self.map_error(server, error)),
        }
    }
}

/// Store em memória — usado em teste e por frontends que precisam de um dublé.
#[derive(Debug, Default)]
pub struct InMemoryCredentialStore {
    entries: Mutex<HashMap<String, String>>,
}

impl InMemoryCredentialStore {
    /// Cria um store vazio.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn set(&self, server: &str, password: &str) -> Result<()> {
        let mut entries = self.entries.lock().expect("credential store lock");
        entries.insert(server.to_string(), password.to_string());
        Ok(())
    }

    fn get(&self, server: &str) -> Result<Option<String>> {
        let entries = self.entries.lock().expect("credential store lock");
        Ok(entries.get(server).cloned())
    }

    fn delete(&self, server: &str) -> Result<bool> {
        let mut entries = self.entries.lock().expect("credential store lock");
        Ok(entries.remove(server).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_and_deletes() {
        let store = InMemoryCredentialStore::new();

        assert_eq!(store.get("dev").expect("get"), None);
        store.set("dev", "s3cret").expect("set");
        assert_eq!(store.get("dev").expect("get"), Some("s3cret".to_string()));

        store.set("dev", "novo").expect("overwrite");
        assert_eq!(store.get("dev").expect("get"), Some("novo".to_string()));

        assert!(store.delete("dev").expect("delete"));
        assert!(!store.delete("dev").expect("delete again"));
        assert_eq!(store.get("dev").expect("get"), None);
    }

    #[test]
    fn service_name_is_stable() {
        assert_eq!(SERVICE_NAME, "mustel");
        assert_eq!(SystemKeyring::new().service(), SERVICE_NAME);
    }

    /// Round-trip contra o cofre real do sistema.
    ///
    /// Ignorado por padrão: escreve no cofre do usuário. Rodar conscientemente com
    /// `MUSTEL_KEYRING_TESTS=1 cargo test -p mustel-core -- --ignored`. O segredo é apagado no fim.
    #[test]
    fn system_keyring_round_trips_when_enabled() {
        if std::env::var("MUSTEL_KEYRING_TESTS").as_deref() != Ok("1") {
            eprintln!("skipped: set MUSTEL_KEYRING_TESTS=1 to exercise the real keyring");
            return;
        }

        let store = SystemKeyring::with_service("mustel-selftest");
        let server = "mustel-test-server";

        assert_eq!(store.get(server).expect("get before"), None);
        store.set(server, "round-trip-check").expect("set");
        assert_eq!(
            store.get(server).expect("get"),
            Some("round-trip-check".to_string())
        );
        assert!(store.delete(server).expect("delete"));
        assert_eq!(store.get(server).expect("get after"), None);
    }
}
