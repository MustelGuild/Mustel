//! De onde vem a senha do `pg_dump`/`pg_restore`/`psql`.
//!
//! Porte do `DatabaseBackupCommand`/`DatabaseRestoreCommand` do legado (a classe
//! `PostgresPasswordHandler` em si só sabia ler do console). A precedência observada é:
//!
//! 1. `-W/--password` da linha de comando;
//! 2. senha **guardada** do servidor configurado que casa com `host:porta` (o legado lia o blob
//!    DPAPI do próprio arquivo de configuração; no Mustel é o cofre do sistema, D6);
//! 3. **prompt** interativo — que é UI e por isso vive no frontend, não aqui (D9).
//!
//! Quem chama passa o prompt como closure, e ele só é chamado quando as duas fontes anteriores não
//! têm valor: é a mesma ordem, com a decisão testável sem tocar terminal.

use crate::config::{ConfigStore, ServerConfigEntry};
use crate::credentials::CredentialStore;
use crate::error::CoreError;

/// Texto do prompt do backup/restore.
///
/// É **diferente** do `"Password: "` do `pgpass add` e do `settings db-servers set-password`; o
/// legado usa `"Enter password: "` só aqui.
pub const PASSWORD_PROMPT: &str = "Enter password: ";

/// Servidor configurado que casa com `host:porta`.
///
/// Regras do legado: o **host** compara sem diferenciar maiúsculas e a **porta** é comparada como
/// texto (`Port.ToString() == port`), ou seja, `"5432"` casa com `5432`. O **nome** do servidor não
/// participa — dois servidores no mesmo host e porta são indistinguíveis aqui.
pub fn find_server<'a>(
    servers: &'a [ServerConfigEntry],
    host: &str,
    port: &str,
) -> Option<&'a ServerConfigEntry> {
    servers
        .iter()
        .find(|server| server.host.eq_ignore_ascii_case(host) && server.port.to_string() == port)
}

/// Senha guardada do servidor que casa com `host:porta`, se houver.
///
/// Ausência de configuração, de servidor correspondente ou de credencial no cofre é `None` — o
/// legado embrulhava tudo num `try`/`catch` vazio pelo mesmo motivo: quem decide se dá para seguir é
/// o prompt, depois.
pub fn stored_password(
    config: &ConfigStore,
    credentials: &dyn CredentialStore,
    host: &str,
    port: &str,
) -> Option<String> {
    let servers = config.get_servers().ok()?;
    let server = find_server(&servers, host, port)?;
    credentials.get(&server.name).ok().flatten()
}

/// Resolve a senha na ordem do legado: flag → valor guardado → prompt.
///
/// O legado trata espaço em branco como ausência (`IsNullOrWhiteSpace`) nas duas primeiras fontes;
/// o que vem do prompt é usado **como veio**, sem checagem (uma senha vazia digitada vira
/// `PGPASSWORD` vazio, e o erro aparece na autenticação do PostgreSQL).
pub fn resolve_password(
    flag: Option<&str>,
    stored: Option<&str>,
    prompt: impl FnOnce() -> Result<String, CoreError>,
) -> Result<String, CoreError> {
    let usable = |value: &str| !value.trim().is_empty();

    if let Some(password) = flag.filter(|value| usable(value)) {
        return Ok(password.to_string());
    }
    if let Some(password) = stored.filter(|value| usable(value)) {
        return Ok(password.to_string());
    }

    prompt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::InMemoryCredentialStore;

    fn server(name: &str, host: &str, port: u16) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: host.to_string(),
            port,
            ..ServerConfigEntry::default()
        }
    }

    fn never() -> Result<String, CoreError> {
        panic!("o prompt não deveria ser chamado")
    }

    #[test]
    fn the_prompt_text_is_the_one_from_backup_and_restore() {
        assert_eq!(PASSWORD_PROMPT, "Enter password: ");
        assert_ne!(PASSWORD_PROMPT, "Password: ");
    }

    #[test]
    /// CONTRACT: IF-018
    fn the_flag_wins() {
        let resolved = resolve_password(Some("da-flag"), Some("guardada"), never).expect("senha");
        assert_eq!(resolved, "da-flag");
    }

    #[test]
    /// CONTRACT: IF-018
    fn the_stored_password_is_used_when_there_is_no_flag() {
        let resolved = resolve_password(None, Some("guardada"), never).expect("senha");
        assert_eq!(resolved, "guardada");
    }

    #[test]
    /// CONTRACT: IF-018
    fn the_prompt_is_the_last_resort_and_its_value_is_used_verbatim() {
        assert_eq!(
            resolve_password(None, None, || Ok("digitada".to_string())).expect("senha"),
            "digitada"
        );
        // O legado não checa o que vem do prompt: uma senha vazia segue adiante.
        assert_eq!(
            resolve_password(None, None, || Ok(String::new())).expect("senha"),
            ""
        );
    }

    #[test]
    fn blank_values_count_as_absent_on_both_sources() {
        for blank in [None, Some(""), Some("   ")] {
            assert_eq!(
                resolve_password(blank, blank, || Ok("digitada".to_string())).expect("senha"),
                "digitada",
                "valor em branco {blank:?} não é senha"
            );
        }
    }

    #[test]
    fn the_server_is_matched_by_host_and_port_ignoring_case() {
        let servers = vec![
            server("dev", "localhost", 5432),
            server("prod", "DB.Empresa.com", 5433),
        ];

        assert_eq!(
            find_server(&servers, "LOCALHOST", "5432").map(|s| s.name.as_str()),
            Some("dev")
        );
        assert_eq!(
            find_server(&servers, "db.empresa.com", "5433").map(|s| s.name.as_str()),
            Some("prod")
        );

        // A porta entra como texto: é assim que o legado compara (`Port.ToString()`).
        assert!(find_server(&servers, "localhost", "5433").is_none());
        assert!(find_server(&servers, "outro.host", "5432").is_none());
    }

    #[test]
    fn the_stored_password_comes_from_the_keyring_by_server_name() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let config = ConfigStore::new(dir.path().join("mustel.jsonc"));
        let mut stored = crate::config::UserConfig::new_empty();
        stored.servers.push(server("dev", "localhost", 5432));
        config.save(&stored).expect("grava config");

        let credentials = InMemoryCredentialStore::new();
        credentials.set("dev", "do-cofre").expect("guarda");

        assert_eq!(
            stored_password(&config, &credentials, "localhost", "5432").as_deref(),
            Some("do-cofre")
        );
        // Host/porta sem servidor correspondente, ou servidor sem credencial: `None`.
        assert!(stored_password(&config, &credentials, "localhost", "5433").is_none());
        assert!(stored_password(&config, &credentials, "outro", "5432").is_none());
    }

    #[test]
    fn a_missing_config_file_is_not_an_error() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let config = ConfigStore::new(dir.path().join("nao-existe.jsonc"));
        let credentials = InMemoryCredentialStore::new();

        assert!(stored_password(&config, &credentials, "localhost", "5432").is_none());
    }
}
