//! Plano do `query run`: dos servidores escolhidos para os bancos alvo (T40).
//!
//! Porte do `GetDatabasesForServerAsync` do legado, mais a fase de descoberta. Sai daqui a lista de
//! [`ServerPlan`] que o executor consome — com **todos** os servidores escolhidos na ordem da
//! seleção, mesmo os que ficam sem banco: o `Servers:` do resumo e a ordem do merge do consolidado
//! contam com eles.
//!
//! A conexão vem **resolvida** do frontend (senha e overrides de linha de comando já aplicados), uma
//! vez por servidor, sem banco definido; aqui só se troca o banco: o de manutenção
//! ([`MAINTENANCE_DATABASE`]) na descoberta, o do alvo em cada execução.

use std::collections::HashSet;

use crate::events::{CoreEvent, EventSink};
use crate::postgres::settings::ConnectionSettings;
use crate::query::discovery;
use crate::query::executor::{DatabaseTarget, ServerPlan};

/// Banco de manutenção onde a descoberta roda, como no
/// `BuildConnectionStringForServer(server, "postgres", null)` do legado.
pub const MAINTENANCE_DATABASE: &str = "postgres";

/// Um servidor escolhido, com a conexão resolvida e o banco ainda em aberto.
#[derive(Debug, Clone)]
pub struct ServerRun {
    /// Nome do servidor: vai na coluna `Server`, no nome do arquivo e nos avisos.
    pub name: String,
    /// `maxParallelism` do cadastro: quantos bancos **deste** servidor rodam ao mesmo tempo.
    pub max_parallelism: usize,
    /// Bancos do cadastro, usados quando o servidor não descobre.
    pub databases: Vec<String>,
    /// `excludePatterns` do cadastro: só valem na descoberta.
    pub exclude_patterns: Vec<String>,
    /// `fetchAllDatabases` do cadastro (`--all` liga em todos).
    pub fetch_all_databases: bool,
    /// Conexão resolvida, **sem** `database`: os alvos trocam o banco aqui.
    pub settings: ConnectionSettings,
}

impl ServerRun {
    /// Este servidor descobre os bancos? (`--all` passa por cima do cadastro.)
    pub fn discovers(&self, all: bool) -> bool {
        all || self.fetch_all_databases
    }
}

/// Monta o plano: um [`ServerPlan`] por servidor escolhido, na ordem, com os bancos já filtrados.
///
/// `all` liga a descoberta em todos e `exclude` são os nomes exatos do `--exclude`, aplicados no fim
/// dos dois caminhos. A fase de descoberta só existe quando **algum** servidor descobre, mas avança
/// por todos os servidores da fila, inclusive os que só leem o cadastro — é o que o legado faz com o
/// `ctx.Status` dentro do laço.
pub async fn plan(
    servers: &[ServerRun],
    all: bool,
    exclude: &HashSet<String>,
    events: &dyn EventSink,
) -> Vec<ServerPlan> {
    let discovers_any = servers.iter().any(|server| server.discovers(all));

    let mut plans = Vec::with_capacity(servers.len());
    for server in servers {
        if discovers_any {
            events.emit(CoreEvent::DiscoveringDatabases {
                server: &server.name,
            });
        }

        let databases = databases_for(server, all, exclude, events).await;

        plans.push(ServerPlan {
            name: server.name.clone(),
            max_parallelism: server.max_parallelism,
            databases: databases
                .iter()
                .map(|database| DatabaseTarget {
                    database: database.clone(),
                    settings: target_settings(&server.settings, database),
                })
                .collect(),
        });
    }

    plans
}

/// Bancos de um servidor: descobre quando o servidor pede e, se a descoberta falhar, **avisa e usa os
/// bancos do cadastro** (ou nenhum, quando o cadastro não tem banco).
async fn databases_for(
    server: &ServerRun,
    all: bool,
    exclude: &HashSet<String>,
    events: &dyn EventSink,
) -> Vec<String> {
    let databases = if server.discovers(all) {
        match discovery::discover_databases(&maintenance_settings(&server.settings)).await {
            Ok(databases) => databases,
            Err(error) => {
                // Os dois avisos são `LogWarning` no legado, que imprime desse nível para cima.
                let message = error.to_string();
                events.emit(CoreEvent::DiscoveryFailed {
                    server: &server.name,
                    error: &message,
                });

                if server.databases.is_empty() {
                    return Vec::new();
                }

                events.emit(CoreEvent::FallingBackToConfiguredDatabases {
                    server: &server.name,
                });
                server.databases.clone()
            }
        }
    } else {
        server.databases.clone()
    };

    // Os padrões do cadastro entram na descoberta; os nomes do `--exclude`, no fim (como no legado).
    discovery::filter_databases(databases, &server.exclude_patterns, exclude)
}

/// A conexão de um alvo: a do servidor com o banco trocado.
pub fn target_settings(template: &ConnectionSettings, database: &str) -> ConnectionSettings {
    ConnectionSettings {
        database: Some(database.to_string()),
        ..template.clone()
    }
}

/// A conexão da descoberta: a do servidor no banco de manutenção.
pub fn maintenance_settings(template: &ConnectionSettings) -> ConnectionSettings {
    target_settings(template, MAINTENANCE_DATABASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{CoreEvent, NoopSink};
    use std::sync::Mutex;

    /// Escuta que guarda os eventos em texto, para os testes conferirem a sequência.
    struct Collector<'a>(&'a Mutex<Vec<String>>);

    impl EventSink for Collector<'_> {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.lock().expect("collector").push(format!("{event:?}"));
        }
    }

    fn server(name: &str, databases: &[&str]) -> ServerRun {
        ServerRun {
            name: name.to_string(),
            max_parallelism: 2,
            databases: databases.iter().map(|value| (*value).to_string()).collect(),
            exclude_patterns: vec!["template*".to_string()],
            fetch_all_databases: false,
            settings: ConnectionSettings {
                host: "localhost".to_string(),
                user: "postgres".to_string(),
                password: Some("segredo".to_string()),
                ..ConnectionSettings::default()
            },
        }
    }

    #[test]
    fn the_target_connection_keeps_everything_and_only_swaps_the_database() {
        let template = server("dev", &[]).settings;

        let target = target_settings(&template, "app");
        assert_eq!(target.database.as_deref(), Some("app"));
        assert_eq!(target.host, "localhost");
        assert_eq!(target.user, "postgres");
        assert_eq!(
            target.password.as_deref(),
            Some("segredo"),
            "a senha resolvida acompanha o alvo"
        );

        assert_eq!(
            maintenance_settings(&template).database.as_deref(),
            Some("postgres"),
            "a descoberta vai no banco de manutenção"
        );
    }

    #[tokio::test]
    async fn without_discovery_the_plan_uses_the_configured_databases_filtered() {
        let servers = vec![server("dev", &["app", "template_extra", "outro"])];
        let events = Mutex::new(Vec::new());

        let plans = plan(
            &servers,
            false,
            &discovery::exclude_names(Some("OUTRO")),
            &Collector(&events),
        )
        .await;

        assert_eq!(plans.len(), 1, "todo servidor escolhido entra no plano");
        assert_eq!(plans[0].name, "dev");
        assert_eq!(plans[0].max_parallelism, 2);
        assert_eq!(
            plans[0]
                .databases
                .iter()
                .map(|target| target.database.as_str())
                .collect::<Vec<_>>(),
            vec!["app"],
            "o padrão do cadastro e o nome do `--exclude` saem do plano"
        );
        assert_eq!(
            plans[0].databases[0].settings.database.as_deref(),
            Some("app")
        );
        assert!(
            events.lock().expect("collector").is_empty(),
            "sem descoberta não há fase para anunciar: {:?}",
            events.lock().expect("collector")
        );
    }

    #[tokio::test]
    async fn a_server_without_databases_still_enters_the_plan() {
        let servers = vec![server("vazio", &[]), server("dev", &["app"])];

        let plans = plan(&servers, false, &HashSet::new(), &NoopSink).await;

        assert_eq!(plans.len(), 2);
        assert!(plans[0].databases.is_empty());
        assert_eq!(plans[1].databases.len(), 1);
    }

    #[tokio::test]
    /// CONTRACT: IF-026
    /// CONTRACT: IF-042
    async fn a_failed_discovery_warns_and_falls_back_to_the_configuration() {
        // Porta fechada na máquina de teste: a descoberta falha rápido e o plano cai no cadastro.
        let mut server = server("fora-do-ar", &["app"]);
        server.fetch_all_databases = true;
        server.settings.port = 1;
        server.settings.connect_timeout = std::time::Duration::from_millis(1);
        server.settings.ssl_mode = crate::postgres::tls::SslMode::Disable;

        let events = Mutex::new(Vec::new());
        let plans = plan(&[server], false, &HashSet::new(), &Collector(&events)).await;

        assert_eq!(
            plans[0]
                .databases
                .iter()
                .map(|target| target.database.as_str())
                .collect::<Vec<_>>(),
            vec!["app"],
            "a falha não derruba o comando: fica o banco do cadastro"
        );

        let seen = events.lock().expect("collector");
        assert!(
            seen.iter()
                .any(|event| event.contains("DiscoveringDatabases")),
            "a fase é anunciada: {seen:?}"
        );
        assert!(
            seen.iter().any(|event| event.contains("DiscoveryFailed")),
            "e a falha vira aviso: {seen:?}"
        );
        assert!(
            seen.iter()
                .any(|event| event.contains("FallingBackToConfiguredDatabases")),
            "com o cadastro não vazio, cai para ele: {seen:?}"
        );
    }
}
