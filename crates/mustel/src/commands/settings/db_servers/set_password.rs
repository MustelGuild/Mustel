//! `mustel settings db-servers set-password` — guarda a senha de um servidor no cofre.
//!
//! Paridade observada no código legado:
//! - sem `name` → seleção de servidor; **sem servidores cadastrados** → mensagem
//!   `No servers configured. Use 'mustel settings db-servers add' to add one.` e exit **0**;
//! - `name` inexistente → `Server 'x' not found.` → exit **41**;
//! - senha lida com máscara (`Password: `) e, no sucesso, `Password saved securely for 'x'.` (exit 0).
//!
//! Diferenças deliberadas (D6): o segredo vai para o **cofre do sistema**, não para um blob no
//! arquivo de configuração; e foi acrescentado `-W/--password` (mesma adição que o `add` ganhou) para
//! que o comando seja utilizável em script, onde não há terminal para prompt.

use mustel_core::config::ConfigStore;
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;

use crate::cli::DbServersSetPasswordArgs;
use crate::ui::prompts;

/// Executa o comando.
pub fn run(args: DbServersSetPasswordArgs) -> Result<i32, CoreError> {
    let store = ConfigStore::with_default_path()?;

    let name = match args.name.as_deref().filter(|name| !name.trim().is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let servers = store.get_servers()?;
            if servers.is_empty() {
                println!("No servers configured. Use 'mustel settings db-servers add' to add one.");
                return Ok(0);
            }

            let names: Vec<String> = servers.iter().map(|server| server.name.clone()).collect();
            prompts::select_one("Select a server to set the password:", &names, None)?
        }
    };

    if store.get_server(&name)?.is_none() {
        return Err(CoreError::InvalidArgument(format!(
            "Server '{name}' not found."
        )));
    }

    let password = match args.password.as_deref() {
        Some(password) => password.to_string(),
        None => prompts::password("Password: ")?,
    };

    store_password(&name, &password)?;
    println!("Password saved securely for '{name}'.");
    Ok(0)
}

/// Guarda a senha no cofre do sistema (D6).
fn store_password(server: &str, password: &str) -> Result<(), CoreError> {
    SystemKeyring::new()
        .set(server, password)
        .map_err(|error| CoreError::InvalidState(error.to_string()))
}
