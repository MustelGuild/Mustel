//! `mustel settings db-servers test` — testa a conexão de um servidor cadastrado.
//!
//! Paridade conferida no código legado:
//! - sem `name` → seleção de servidor; sem servidores cadastrados → mensagem e exit **0**;
//! - `name` inexistente → `Server 'x' not found.` → exit **41**;
//! - imprime `Testing connection to {nome} ({host}:{porta})...` antes de conectar;
//! - sucesso → `Connection to {host}:{porta} successful`, `Authenticated as {usuário}` e
//!   `Databases found: a, b, c` (ou `No accessible databases found`); exit **0**;
//! - falha de conexão → o legado **embrulha** o erro em `InvalidOperationException`, então o exit é
//!   **40** (não 10/11 como eu havia suposto antes de ler o código), com prefixo `Authentication
//!   failed:` quando a mensagem cheira a senha e `Connection failed:` nos demais casos.
//!
//! **Divergência deliberada (documentada):** o legado lista bancos executando o **`psql` externo**
//! (`SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname;`), exigindo o
//! binário instalado — sem ele, o comando falha com "psql executable not found". O Mustel usa o
//! próprio driver (D12) com a **mesma query**, o que dispensa o `psql` no caminho do `test`.

use mustel_core::config::ConfigStore;
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;
use mustel_core::usecases::db_servers;

use crate::cli::DbServersTestArgs;
use crate::ui::prompts;

/// Executa o comando.
pub async fn run(args: DbServersTestArgs) -> Result<i32, CoreError> {
    let store = ConfigStore::with_default_path()?;

    let name = match args.name.as_deref().filter(|name| !name.trim().is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let servers = db_servers::list(&store)?;
            if servers.is_empty() {
                println!("No servers configured. Use 'mustel settings db-servers add' to add one.");
                return Ok(0);
            }

            let names: Vec<String> = servers.iter().map(|server| server.name.clone()).collect();
            prompts::select_one("Select a server to test:", &names, None)?
        }
    };

    let server = store
        .get_server(&name)?
        .ok_or_else(|| CoreError::InvalidArgument(format!("Server '{name}' not found.")))?;

    println!(
        "Testing connection to {} ({}:{})...",
        server.name, server.host, server.port
    );

    let password = match SystemKeyring::new().get(&name) {
        Ok(Some(password)) => password,
        _ => {
            println!(
                "No password found for '{}'. Type to test (will not be saved):",
                server.name
            );
            prompts::password("Password: ")?
        }
    };

    let report = db_servers::test_connection(&server, &password).await?;

    println!("Connection to {}:{} successful", server.host, server.port);
    println!("Authenticated as {}", server.username);
    if report.databases.is_empty() {
        println!("No accessible databases found");
    } else {
        println!("Databases found: {}", report.databases.join(", "));
    }

    Ok(0)
}
