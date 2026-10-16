//! `mustel settings db-servers rm` — remove servidor(es).
//!
//! Paridade observada (T9) e no código legado:
//! - sem servidores configurados → mensagem `No servers configured to remove.` e exit **0**;
//! - com `-n` inexistente → `Server 'x' not found.` → exit **41**;
//! - com `-n` válido → `Server 'x' removed successfully.` e exit **0**;
//! - sem `-n` → multi-seleção; nada escolhido → `No servers selected. Operation cancelled.` (0);
//!   cada escolhido imprime `Server 'x' removed.` (texto **diferente** do caminho com `-n`).
//!
//! Adição do Mustel: a remoção também apaga a credencial do cofre (D6). O legado não precisava
//! disso porque o blob ficava no próprio arquivo de configuração.

use mustel_core::config::ConfigStore;
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;

use crate::cli::DbServersRemoveArgs;
use crate::ui::prompts;

/// Executa o comando.
pub fn run(args: DbServersRemoveArgs) -> Result<i32, CoreError> {
    let store = ConfigStore::with_default_path()?;
    let servers = store.get_servers()?;

    if servers.is_empty() {
        println!("No servers configured to remove.");
        return Ok(0);
    }

    if let Some(name) = args.name.as_deref().filter(|name| !name.trim().is_empty()) {
        if !store.remove_server(name)? {
            return Err(CoreError::InvalidArgument(format!(
                "Server '{name}' not found."
            )));
        }
        forget_password(name);
        println!("Server '{name}' removed successfully.");
        return Ok(0);
    }

    let names: Vec<String> = servers.iter().map(|server| server.name.clone()).collect();
    let selected = prompts::select_many("Select servers to remove:", &names, &[])?;

    if selected.is_empty() {
        println!("No servers selected. Operation cancelled.");
        return Ok(0);
    }

    for name in &selected {
        store.remove_server(name)?;
        forget_password(name);
        println!("Server '{name}' removed.");
    }

    Ok(0)
}

/// Apaga a senha guardada do servidor, sem impedir a remoção do cadastro.
///
/// Sem cofre disponível não há o que limpar; um segredo órfão no cofre não afeta o cadastro, e
/// falhar aqui mudaria a saída de um comando que o usuário já concluiu.
fn forget_password(server: &str) {
    let _ = SystemKeyring::new().delete(server);
}
