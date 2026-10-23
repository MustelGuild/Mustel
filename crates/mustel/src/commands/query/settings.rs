//! Resolução da conexão do `query run`.
//!
//! Porte de `BuildConnectionStringForServer` + `CreateAdHocServer` do legado. As regras medidas no
//! T32, quando o oráculo rodou contra um Postgres real:
//!
//! - com **servidor configurado**, os overrides de linha de comando vencem o cadastro (host, porta,
//!   usuário, ssl-mode, timeout) — e `-d/--database` **não** é usado: o banco vem da lista do
//!   servidor (a descrição do legado promete que `-d` sobrescreve, mas o código nunca lê o valor
//!   nesse caminho);
//! - com **`--npgsql-connection-string`**, um servidor ad-hoc é montado com o que a string traz
//!   (host, porta, banco, usuário, ssl-mode, timeout) e os overrides entram por cima. **A senha da
//!   string é descartada** — o `CreateAdHocServer` deixa `EncryptedPassword = null` e a montagem
//!   final resolve a senha de novo, então o comando **pede a senha** se ela não vier em `-W` e não
//!   houver credencial guardada. É um comportamento do legado, medido, e reproduzido aqui de
//!   propósito;
//! - a senha segue a precedência `-W` → cofre (pelo **nome do servidor**) → prompt, com o texto
//!   `Password for '<servidor>': `;
//! - `--ssl-mode` desconhecido vira `Prefer` (o `ParseSslMode` do legado), e não é erro.
//!
//! Das opções de **pool** (`--pooling`, `--min-pool-size`, `--max-pool-size`), só o valor fica
//! guardado na conexão: o Mustel abre uma conexão por banco, então não há pool (ver
//! `docs/rust-postgres-spike.md`). O `--keepalive` **tem** efeito — vai para o socket, como no Npgsql —
//! e o `--connection-lifetime` só valeria num pool, onde não é usado ainda.

use mustel_core::config::{ConfigStore, ServerConfigEntry, UserDefaults};
use mustel_core::credentials::{CredentialStore, SystemKeyring};
use mustel_core::error::CoreError;
use mustel_core::postgres::settings::ConnectionSettings;
use mustel_core::postgres::tls::SslMode;
use mustel_core::query::plan::ServerRun;
use mustel_core::usecases::query_run;

use crate::cli::QueryRunArgs;
use crate::ui::prompts;

/// Texto do prompt de senha do `query run` (medido no oráculo).
pub const PASSWORD_PROMPT: &str = "Password for";

/// Servidores a usar: o ad-hoc da connection string ou os cadastrados (com os overrides).
///
/// Devolve `None` quando o comando já tratou o caso (config vazia) — quem chama decide a mensagem.
pub fn servers_from_settings(
    args: &QueryRunArgs,
) -> Result<Option<Vec<ServerConfigEntry>>, CoreError> {
    if let Some(connection_string) = args
        .npgsql_connection_string
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Some(vec![ad_hoc_server(connection_string, args)]));
    }

    let store = ConfigStore::with_default_path()?;
    let servers = store.get_servers()?;
    if servers.is_empty() {
        return Ok(None);
    }

    Ok(Some(servers))
}

/// Preferências do arquivo (`defaults`): diretório de saída e paralelismo padrão.
pub fn defaults() -> Result<UserDefaults, CoreError> {
    let store = ConfigStore::with_default_path()?;

    query_run::defaults(&store)
}

/// Diretório base da saída: `-o` quando informado (absolutizado, como o `Path.GetFullPath` do
/// legado), senão o default do cadastro.
pub fn output_directory(args: &QueryRunArgs, defaults: &UserDefaults) -> String {
    match args
        .output
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(output) => std::path::absolute(output)
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|_| output.to_string()),
        None => defaults.output_directory.clone(),
    }
}

/// Resolve a entrada de um servidor para o caso de uso: cadastro + overrides + senha, **uma vez**.
///
/// A conexão sai sem banco definido (o núcleo troca o banco de cada alvo e o da descoberta); é o que
/// evita resolver a senha de novo a cada banco — e por isso o prompt de senha aparece no máximo uma
/// vez por servidor (divergência 17 em `oracle/known-divergences.md`).
pub fn server_run(server: &ServerConfigEntry, args: &QueryRunArgs) -> Result<ServerRun, CoreError> {
    Ok(ServerRun {
        name: server.name.clone(),
        max_parallelism: server.max_parallelism as usize,
        databases: server.databases.clone(),
        exclude_patterns: server.exclude_patterns.clone(),
        fetch_all_databases: server.fetch_all_databases,
        settings: connection_settings(server, args)?,
    })
}

/// Monta a conexão de um servidor, aplicando os overrides e resolvendo a senha.
///
/// Sai **sem** banco definido: é o molde das conexões do servidor, e quem troca o banco é o núcleo
/// ([`mustel_core::query::plan`]), um banco por alvo e o de manutenção na descoberta.
fn connection_settings(
    server: &ServerConfigEntry,
    args: &QueryRunArgs,
) -> Result<ConnectionSettings, CoreError> {
    let password = resolve_password(server, args)?;
    let ssl_mode = resolve_ssl_mode(args, server);

    Ok(ConnectionSettings {
        host: resolve_text(args.host.as_deref(), &server.host),
        port: resolve_port(args.port.as_deref(), server.port),
        user: resolve_text(args.username.as_deref(), &server.username),
        password: Some(password),
        database: None,
        ssl_mode,
        connect_timeout: std::time::Duration::from_secs(
            // O `--timeout` do CLI é `i32`; o cadastro guarda segundos (u32).
            args.timeout
                .map(|seconds| seconds.max(1) as u64)
                .unwrap_or_else(|| u64::from(server.timeout).max(1)),
        ),
        command_timeout: std::time::Duration::from_secs(
            args.command_timeout.unwrap_or(300).max(1) as u64
        ),
        // As opções de pool entram aqui para não serem silenciosamente descartadas: o `--keepalive`
        // vai para o socket da conexão (ver `to_pg_config`) e as demais só valeriam num pool, que o
        // Mustel não tem.
        pooling: args.pooling.unwrap_or(true),
        min_pool_size: args.min_pool_size.unwrap_or(1).max(0) as u32,
        max_pool_size: args.max_pool_size.unwrap_or(100).max(0) as u32,
        keepalive: std::time::Duration::from_secs(args.keepalive.unwrap_or(0).max(0) as u64),
        connection_lifetime: std::time::Duration::from_secs(
            args.connection_lifetime.unwrap_or(0).max(0) as u64,
        ),
    })
}

/// Senha: `-W` → cofre (pelo nome do servidor) → prompt, como o `ResolvePassword` do legado.
fn resolve_password(server: &ServerConfigEntry, args: &QueryRunArgs) -> Result<String, CoreError> {
    if let Some(password) = args
        .password
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(password.to_string());
    }

    if let Ok(Some(password)) = SystemKeyring::new().get(&server.name) {
        return Ok(password);
    }

    println!(
        "No password found for server '{}'. Use 'mustel settings db-servers set-password' to save it \
         permanently.",
        server.name
    );
    prompts::password(&format!("{PASSWORD_PROMPT} '{}': ", server.name))
}

/// `--ssl-mode` vence o cadastro; valor desconhecido vira `Prefer`, como no legado.
fn resolve_ssl_mode(args: &QueryRunArgs, server: &ServerConfigEntry) -> SslMode {
    let raw = args
        .ssl_mode
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&server.ssl_mode);

    raw.parse().unwrap_or_default()
}

/// Override de texto vence o cadastro quando não é branco.
fn resolve_text(override_value: Option<&str>, config_value: &str) -> String {
    match override_value.filter(|value| !value.trim().is_empty()) {
        Some(value) => value.to_string(),
        None => config_value.to_string(),
    }
}

/// Override de porta: número válido vence; qualquer outra coisa mantém o cadastro.
fn resolve_port(override_value: Option<&str>, config_value: u16) -> u16 {
    override_value
        .and_then(|value| value.parse().ok())
        .unwrap_or(config_value)
}

/// Servidor ad-hoc a partir do `--npgsql-connection-string`.
///
/// Reproduz o `CreateAdHocServer`, inclusive o descarte da **senha** (que por isso volta a ser
/// resolvida pela precedência normal) e o `Name` = host.
fn ad_hoc_server(connection_string: &str, args: &QueryRunArgs) -> ServerConfigEntry {
    let parsed = parse_connection_string(connection_string);

    let host = resolve_text(
        args.host.as_deref(),
        parsed.host.as_deref().unwrap_or("localhost"),
    );
    let database = resolve_text(
        args.database.as_deref(),
        parsed.database.as_deref().unwrap_or("postgres"),
    );

    ServerConfigEntry {
        name: host.clone(),
        host,
        port: resolve_port(args.port.as_deref(), parsed.port.unwrap_or(5432)),
        username: resolve_text(
            args.username.as_deref(),
            parsed.user.as_deref().unwrap_or(""),
        ),
        databases: vec![database],
        fetch_all_databases: args.all,
        ssl_mode: parsed.ssl_mode.unwrap_or_else(|| "Prefer".to_string()),
        // O `--timeout` do CLI é `i32` e o cadastro guarda segundos; o valor da string entra no
        // meio da precedência (override → string → padrão de 30s).
        timeout: args
            .timeout
            .map(|seconds| seconds.max(1) as u32)
            .or(parsed.timeout)
            .unwrap_or(30),
        max_parallelism: 4,
        ..ServerConfigEntry::default()
    }
}

/// O que o driver tira da connection string (só o que o legado aproveita).
#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedConnectionString {
    host: Option<String>,
    port: Option<u16>,
    database: Option<String>,
    user: Option<String>,
    ssl_mode: Option<String>,
    timeout: Option<u32>,
}

/// Lê uma connection string no estilo Npgsql (`Chave=Valor;Chave=Valor`).
///
/// Chaves desconhecidas (senha, pooling, keepalive, ...) são ignoradas — a senha porque o legado a
/// descarta, as demais porque não há pool ainda (T34).
fn parse_connection_string(connection_string: &str) -> ParsedConnectionString {
    let mut parsed = ParsedConnectionString::default();

    for pair in connection_string.split(';') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "host" | "server" | "data source" => parsed.host = Some(value.to_string()),
            "port" => parsed.port = value.parse().ok(),
            "database" | "initial catalog" => parsed.database = Some(value.to_string()),
            "username" | "user id" | "userid" | "user" => parsed.user = Some(value.to_string()),
            "sslmode" | "ssl mode" => parsed.ssl_mode = Some(value.to_string()),
            "timeout" => parsed.timeout = value.parse().ok(),
            _ => {}
        }
    }

    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> QueryRunArgs {
        QueryRunArgs {
            input: None,
            command: None,
            output: None,
            npgsql_connection_string: None,
            host: None,
            port: None,
            database: None,
            username: None,
            password: Some("senha".to_string()),
            ssl_mode: None,
            timeout: None,
            command_timeout: None,
            pooling: None,
            min_pool_size: None,
            max_pool_size: None,
            keepalive: None,
            connection_lifetime: None,
            all: false,
            exclude: None,
            no_confirm: false,
        }
    }

    fn server() -> ServerConfigEntry {
        ServerConfigEntry {
            name: "dev".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            databases: vec!["mydb".to_string()],
            ssl_mode: "Prefer".to_string(),
            timeout: 30,
            ..ServerConfigEntry::default()
        }
    }

    #[test]
    /// CONTRACT: IF-016
    fn the_connection_string_is_parsed_like_npgsql() {
        let parsed = parse_connection_string(
            "Host=127.0.0.1;Port=55432;Database=mustel_test;Username=postgres;Password=dev;\
             SslMode=Disable;Timeout=15;Pooling=false",
        );

        assert_eq!(
            parsed,
            ParsedConnectionString {
                host: Some("127.0.0.1".to_string()),
                port: Some(55432),
                database: Some("mustel_test".to_string()),
                user: Some("postgres".to_string()),
                ssl_mode: Some("Disable".to_string()),
                timeout: Some(15),
            }
        );
    }

    #[test]
    /// CONTRACT: IF-017
    fn the_ad_hoc_server_takes_the_name_from_the_host_and_drops_the_password() {
        let command = args();
        let ad_hoc = ad_hoc_server(
            "Host=127.0.0.1;Port=55432;Database=mustel_test;Username=postgres;Password=dev",
            &command,
        );

        assert_eq!(ad_hoc.name, "127.0.0.1", "o `Name` do ad-hoc é o host");
        assert_eq!(ad_hoc.host, "127.0.0.1");
        assert_eq!(ad_hoc.port, 55432);
        assert_eq!(ad_hoc.username, "postgres");
        assert_eq!(ad_hoc.databases, vec!["mustel_test".to_string()]);
        assert_eq!(
            ad_hoc.timeout, 30,
            "sem `--timeout` nem `Timeout=` vale o padrão de 30s"
        );

        // A senha da string **não** entra no cadastro (o `CreateAdHocServer` a descarta).
        let settings = connection_settings(&ad_hoc, &command).expect("resolve");
        assert_eq!(settings.password.as_deref(), Some("senha"), "vem do `-W`");
    }

    #[test]
    /// CONTRACT: IF-020
    fn command_line_overrides_win_over_the_connection_string() {
        let mut command = args();
        command.host = Some("outro.host".to_string());
        command.port = Some("6000".to_string());
        command.username = Some("admin".to_string());
        command.database = Some("outro_banco".to_string());

        let ad_hoc = ad_hoc_server("Host=127.0.0.1;Port=55432;Database=mustel_test", &command);

        assert_eq!(ad_hoc.name, "outro.host");
        assert_eq!(ad_hoc.port, 6000);
        assert_eq!(ad_hoc.username, "admin");
        assert_eq!(ad_hoc.databases, vec!["outro_banco".to_string()]);
    }

    #[test]
    /// CONTRACT: IF-019
    fn command_line_overrides_win_over_the_configured_server() {
        let mut command = args();
        command.host = Some("10.0.0.9".to_string());
        command.port = Some("6543".to_string());
        command.username = Some("app".to_string());
        command.ssl_mode = Some("Disable".to_string());
        command.timeout = Some(5);

        let settings = connection_settings(&server(), &command).expect("resolve");

        assert_eq!(settings.host, "10.0.0.9");
        assert_eq!(settings.port, 6543);
        assert_eq!(settings.user, "app");
        assert_eq!(settings.ssl_mode, SslMode::Disable);
        assert_eq!(settings.connect_timeout, std::time::Duration::from_secs(5));
    }

    #[test]
    fn without_overrides_the_server_config_is_used() {
        let settings = connection_settings(&server(), &args()).expect("resolve");

        assert_eq!(settings.host, "localhost");
        assert_eq!(settings.port, 5432);
        assert_eq!(settings.user, "postgres");
        assert_eq!(settings.ssl_mode, SslMode::Prefer);
        assert_eq!(settings.connect_timeout, std::time::Duration::from_secs(30));
    }

    #[test]
    /// CONTRACT: IF-022
    fn an_unknown_ssl_mode_falls_back_to_prefer() {
        let mut command = args();
        command.ssl_mode = Some("Inventado".to_string());

        assert_eq!(
            connection_settings(&server(), &command)
                .expect("resolve")
                .ssl_mode,
            SslMode::Prefer
        );
    }

    #[test]
    fn the_template_has_no_database_because_the_core_picks_one_per_target() {
        let template = connection_settings(&server(), &args()).expect("resolve");

        assert_eq!(
            template.database, None,
            "o banco entra no núcleo: um por alvo, e `postgres` na descoberta"
        );
    }

    #[test]
    /// CONTRACT: IF-023
    fn an_invalid_port_override_keeps_the_configured_port() {
        let mut command = args();
        command.port = Some("nao-e-porta".to_string());

        assert_eq!(
            connection_settings(&server(), &command)
                .expect("resolve")
                .port,
            5432
        );
    }
}
