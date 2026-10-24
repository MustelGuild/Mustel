//! Quais **bases** de quais servidores vão rodar (aba Query).
//!
//! O estado é `servidor → {bases marcadas}`. Ele guarda **nomes**, não posições: por isso marcar,
//! trocar de página ou mudar a busca não perde nada (R7) — a tela só decide o que desenhar.
//!
//! O universo de bases de cada servidor vem de três lugares, nesta ordem de confiança: o
//! `databases` do cadastro, o `⟳ Discover` (`pg_database`) e o que uma query de seleção trouxe.
//! Base que casa com `excludePatterns` do **cadastro** nunca pode ser marcada (R9) — o padrão
//! sempre vale, inclusive sobre o que a query trouxe.

use std::collections::{BTreeMap, BTreeSet};

use mustel_core::config::ServerConfigEntry;
use mustel_core::query::discovery;
use mustel_core::usecases::custom_databases::ServerSelection;

/// Quantas bases cada página mostra.
///
/// O número é o mesmo das telas de 640 px de largura mínima: 25 linhas de 18 px cabem com o
/// paginador sem empurrar o `Run query` para fora.
pub const PAGE_SIZE: usize = 25;

/// Até onde o `⟳ Apply` vale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyScope {
    /// Todos os servidores do cadastro.
    All,
    /// Um servidor só, pelo nome do cadastro.
    Server(String),
}

/// O que se sabe de uma base: por que ela **não** entra, quando for o caso.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    /// `Some(padrão)` quando um `excludePatterns` do servidor barrou a base.
    pub blocked_by: Option<String>,
    /// Aviso não bloqueante (usuário da string sem senha, por exemplo).
    pub warning: Option<String>,
}

/// Uma linha da lista: o servidor e uma base dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Servidor do cadastro.
    pub server: String,
    /// Base.
    pub database: String,
    /// Por que ela não pode ser marcada, quando for o caso.
    pub info: Info,
    /// Está marcada?
    pub marked: bool,
}

/// O estado da seleção de bases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionState {
    /// Bases conhecidas por servidor.
    universe: BTreeMap<String, BTreeMap<String, Info>>,
    /// Bases marcadas por servidor.
    marked: BTreeMap<String, BTreeSet<String>>,
    /// Nome da última query de seleção aplicada.
    query: Option<String>,
    /// Texto da busca.
    search: String,
    /// Página atual (0-based).
    page: usize,
}

impl SelectionState {
    /// Nome da última query aplicada, quando houver.
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Total de bases marcadas em todos os servidores.
    pub fn marked_count(&self) -> usize {
        self.marked.values().map(BTreeSet::len).sum()
    }

    /// Quantas bases estão marcadas num servidor.
    pub fn marked_on(&self, server: &str) -> usize {
        self.marked.get(server).map_or(0, BTreeSet::len)
    }

    /// Quantas bases o servidor tem no universo.
    pub fn known_on(&self, server: &str) -> usize {
        self.universe.get(server).map_or(0, BTreeMap::len)
    }

    /// A base pode ser marcada? (base conhecida e não barrada por padrão)
    pub fn is_selectable(&self, server: &str, database: &str) -> bool {
        self.universe
            .get(server)
            .and_then(|databases| databases.get(database))
            .is_some_and(|info| info.blocked_by.is_none())
    }

    /// Marca ou desmarca uma base, preservando o resto da seleção (R1).
    ///
    /// Base barrada por `excludePatterns` **não** entra, mesmo que a tela peça: a regra é do
    /// cadastro, não da tela (R9).
    pub fn set_marked(&mut self, server: &str, database: &str, marked: bool) {
        if !self.is_selectable(server, database) {
            return;
        }
        let entry = self.marked.entry(server.to_string()).or_default();
        if marked {
            entry.insert(database.to_string());
        } else {
            entry.remove(database);
            if entry.is_empty() {
                self.marked.remove(server);
            }
        }
    }

    /// O `⟳ Apply`: **substitui** a seleção do escopo pedido (R1).
    ///
    /// Fora do escopo nada muda — desmarcar à mão antes de aplicar um servidor só é preservado.
    /// Base barrada por padrão entra no universo (para aparecer desabilitada, R9) mas não é marcada.
    pub fn apply(&mut self, selection: &[ServerSelection], scope: &ApplyScope, query: &str) {
        match scope {
            ApplyScope::All => self.marked.clear(),
            ApplyScope::Server(name) => {
                self.marked.remove(name);
            }
        }

        for server in selection {
            let in_scope = match scope {
                ApplyScope::All => true,
                ApplyScope::Server(name) => server.server.eq_ignore_ascii_case(name),
            };
            if !in_scope {
                continue;
            }

            let entry = self.universe.entry(server.server.clone()).or_default();
            let mut selected = BTreeSet::new();
            for database in &server.databases {
                entry.insert(
                    database.name.clone(),
                    Info {
                        blocked_by: database.blocked_by.clone(),
                        warning: database.warning.clone(),
                    },
                );
                // Barrada por padrão **e** com aviso de credencial não entram sozinhas (R9, R4b):
                // o `Apply` marca o que está pronto para rodar, e o que não está aparece na lista
                // com o motivo — em vez de virar um erro só na hora de executar.
                if database.blocked_by.is_none() && database.warning.is_none() {
                    selected.insert(database.name.clone());
                }
            }
            if !selected.is_empty() {
                self.marked.insert(server.server.clone(), selected);
            }
        }

        self.query = Some(query.to_string());
        self.page = 0;
    }

    /// O `⟳ Discover`: preenche o universo do servidor com o que `pg_database` devolveu (R6).
    ///
    /// Bases que só a query de seleção conhecia continuam marcadas — descobrir **não** desmarca.
    /// Devolve as bases que estavam marcadas e sumiram, para a tela avisar (R7).
    pub fn discover(
        &mut self,
        server: &ServerConfigEntry,
        databases: &[String],
    ) -> Vec<(String, String)> {
        let entry = self.universe.entry(server.name.clone()).or_default();

        // O padrão do cadastro vale para todo o universo, inclusive para o que veio daqui.
        for database in databases {
            let blocked_by = server
                .exclude_patterns
                .iter()
                .find(|pattern| discovery::matches_pattern(database, pattern))
                .cloned();
            entry.insert(
                database.clone(),
                Info {
                    blocked_by,
                    ..Info::default()
                },
            );
        }

        let known: BTreeSet<&String> = databases.iter().collect();
        let mut lost = Vec::new();
        if let Some(marked) = self.marked.get_mut(&server.name) {
            let gone: Vec<String> = marked
                .iter()
                .filter(|database| !known.contains(database))
                .cloned()
                .collect();
            for database in gone {
                marked.remove(&database);
                lost.push((server.name.clone(), database));
            }
            if marked.is_empty() {
                self.marked.remove(&server.name);
            }
        }

        lost
    }

    /// Junta a lista de bases do cadastro ao universo, sem marcar nada.
    ///
    /// É o que faz a lista abrir mostrando as bases que o cadastro já conhece, mesmo antes de um
    /// `⟳ Discover`.
    pub fn sync_registry(&mut self, servers: &[ServerConfigEntry]) {
        for server in servers {
            let entry = self.universe.entry(server.name.clone()).or_default();
            for database in &server.databases {
                let blocked_by = server
                    .exclude_patterns
                    .iter()
                    .find(|pattern| discovery::matches_pattern(database, pattern))
                    .cloned();
                entry.entry(database.clone()).or_insert(Info {
                    blocked_by,
                    ..Info::default()
                });
            }
        }
    }

    /// Esquece servidor que saiu do cadastro, levando a seleção dele junto (R2/T7).
    pub fn retain_servers(&mut self, servers: &[ServerConfigEntry]) -> Vec<String> {
        let alive: BTreeSet<&str> = servers.iter().map(|server| server.name.as_str()).collect();

        let mut removed = Vec::new();
        self.universe.retain(|name, _| {
            let keep = alive.contains(name.as_str());
            if !keep {
                removed.push(name.clone());
            }
            keep
        });
        self.marked.retain(|name, _| alive.contains(name.as_str()));

        // A query aplicada pode ter sido cadastrada para um servidor que não existe mais.
        removed
    }

    /// As linhas da página atual, já filtradas pela busca.
    ///
    /// A ordem é a do cadastro (servidor) e a alfabética dentro dele — estável entre quadros, que
    /// é o que faz a paginação não "pular" sozinha.
    ///
    /// Servidor do cadastro **sem base conhecida** também vira linha (com nome de base vazio): sem
    /// isso ele não apareceria na lista e o `⟳ Discover` da própria linha seria inalcançável (R2).
    pub fn rows(&self, servers: &[ServerConfigEntry]) -> Vec<Row> {
        let needle = self.search.trim().to_lowercase();
        let mut rows = Vec::new();

        for server in servers {
            let server_matches = needle.is_empty() || server.name.to_lowercase().contains(&needle);
            let mut emitted = 0usize;

            if let Some(databases) = self.universe.get(&server.name) {
                for (database, info) in databases {
                    let matches = needle.is_empty()
                        || database.to_lowercase().contains(&needle)
                        || server.name.to_lowercase().contains(&needle);
                    if !matches {
                        continue;
                    }
                    rows.push(Row {
                        server: server.name.clone(),
                        database: database.clone(),
                        info: info.clone(),
                        marked: self
                            .marked
                            .get(&server.name)
                            .is_some_and(|marked| marked.contains(database)),
                    });
                    emitted += 1;
                }
            }

            if emitted == 0 && server_matches {
                rows.push(Row {
                    server: server.name.clone(),
                    database: String::new(),
                    info: Info::default(),
                    marked: false,
                });
            }
        }

        rows
    }

    /// Quantas páginas a lista tem com a busca atual.
    pub fn page_count(&self, servers: &[ServerConfigEntry]) -> usize {
        self.rows(servers).len().div_ceil(PAGE_SIZE).max(1)
    }

    /// A página atual, já presa ao intervalo válido.
    pub fn page(&self, servers: &[ServerConfigEntry]) -> usize {
        self.page.min(self.page_count(servers).saturating_sub(1))
    }

    /// As linhas da página atual.
    pub fn page_rows(&self, servers: &[ServerConfigEntry]) -> Vec<Row> {
        let rows = self.rows(servers);
        let page = self.page(servers);
        rows.into_iter()
            .skip(page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .collect()
    }

    /// Página seguinte/anterior (não passa dos limites).
    pub fn turn_page(&mut self, servers: &[ServerConfigEntry], forward: bool) {
        let last = self.page_count(servers).saturating_sub(1);
        self.page = if forward {
            (self.page(servers) + 1).min(last)
        } else {
            self.page(servers).saturating_sub(1)
        };
    }

    /// Texto da busca (a página volta ao começo quando ele muda).
    pub fn set_search(&mut self, search: String) {
        if self.search != search {
            self.search = search;
            self.page = 0;
        }
    }

    /// A busca atual.
    pub fn search(&self) -> &str {
        &self.search
    }

    /// Os alvos do `Run query`: `(servidor, bases marcadas)`, na ordem do cadastro (R10).
    ///
    /// Servidor sem base marcada **não entra**, e base barrada por padrão nunca chega aqui — o
    /// filtro é do cadastro e vale antes do plano.
    pub fn targets(&self, servers: &[ServerConfigEntry]) -> Vec<(String, Vec<String>)> {
        servers
            .iter()
            .filter_map(|server| {
                let databases: Vec<String> = self
                    .marked
                    .get(&server.name)?
                    .iter()
                    .filter(|database| self.is_selectable(&server.name, database))
                    .cloned()
                    .collect();
                if databases.is_empty() {
                    None
                } else {
                    Some((server.name.clone(), databases))
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustel_core::usecases::custom_databases::DatabaseChoice;

    fn server(name: &str) -> ServerConfigEntry {
        ServerConfigEntry {
            name: name.to_string(),
            host: "127.0.0.1".to_string(),
            username: "postgres".to_string(),
            exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
            ..ServerConfigEntry::default()
        }
    }

    fn servers() -> Vec<ServerConfigEntry> {
        vec![server("Local"), server("Produção")]
    }

    fn choice(name: &str) -> DatabaseChoice {
        DatabaseChoice {
            name: name.to_string(),
            blocked_by: None,
            warning: None,
        }
    }

    fn selection(server: &str, databases: &[&str]) -> ServerSelection {
        ServerSelection {
            server: server.to_string(),
            databases: databases.iter().map(|name| choice(name)).collect(),
        }
    }

    /// Marcar e desmarcar preserva o resto da seleção.
    /// CONTRACT: IF-227
    #[test]
    fn marking_and_unmarking_keeps_the_rest() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state
            .universe
            .entry("Local".to_string())
            .or_default()
            .extend([
                ("dev".to_string(), Info::default()),
                ("prod_a".to_string(), Info::default()),
                ("test_a".to_string(), Info::default()),
            ]);

        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "prod_a", true);
        assert_eq!(state.marked_count(), 2);

        state.set_marked("Local", "dev", false);
        assert_eq!(state.marked_count(), 1, "só o desmarcado saiu");
        assert!(
            state
                .targets(&servers)
                .first()
                .is_some_and(|(_, databases)| databases == &["prod_a".to_string()])
        );
    }

    /// `⟳ Apply` **substitui** o escopo: todos, ou um servidor só (R1).
    /// CONTRACT: IF-227
    #[test]
    fn apply_replaces_only_the_requested_scope() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state
            .universe
            .entry("Local".to_string())
            .or_default()
            .extend([
                ("dev".to_string(), Info::default()),
                ("velho".to_string(), Info::default()),
            ]);
        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "velho", true);

        // Só o Local: o Produção não é tocado (ele nem tem nada marcado ainda).
        state.apply(
            &[selection("Local", &["dev", "test_a"])],
            &ApplyScope::Server("Local".to_string()),
            "Produção",
        );
        assert_eq!(
            state.marked_on("Local"),
            2,
            "`velho` saiu, `dev` e `test_a` ficaram"
        );
        assert!(
            !state
                .marked
                .get("Local")
                .is_some_and(|marked| marked.contains("velho")),
            "o Apply substitui, não soma"
        );

        // Todos: agora o Produção entra e o Local é recalculado do que a query trouxe.
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::All,
            "Produção",
        );
        assert_eq!(state.marked_on("Local"), 1);
        assert_eq!(state.marked_on("Produção"), 1);
        assert_eq!(state.query(), Some("Produção"));
    }

    /// A seleção sobrevive a mudar página e busca (R7).
    /// CONTRACT: IF-227
    #[test]
    fn the_selection_survives_paging_and_searching() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..80).map(|index| format!("base_{index:02}")).collect();
        state.discover(&servers[0], &many);

        state.set_marked("Local", "base_00", true);
        state.set_marked("Local", "base_70", true);
        assert_eq!(state.marked_count(), 2);

        state.turn_page(&servers, true);
        state.turn_page(&servers, true);
        assert_eq!(state.marked_count(), 2, "virar página não desmarca");

        state.set_search("base_7".to_string());
        assert_eq!(state.page(&servers), 0, "mudar a busca volta ao começo");
        assert_eq!(state.marked_count(), 2, "buscar não desmarca");
        assert_eq!(state.rows(&servers).len(), 10, "base_70..base_79");

        state.set_search(String::new());
        assert_eq!(state.marked_count(), 2);
    }

    /// Servidor removido do cadastro leva a seleção dele junto.
    /// CONTRACT: IF-227
    #[test]
    fn a_removed_server_takes_its_selection_along() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::All,
            "Produção",
        );
        assert_eq!(state.marked_count(), 2);

        let remaining = vec![server("Local")];
        state.retain_servers(&remaining);

        assert_eq!(state.marked_count(), 1, "a do servidor removido saiu");
        assert_eq!(state.marked_on("Local"), 1);
        assert_eq!(state.marked_on("Produção"), 0);
        assert!(
            state
                .targets(&remaining)
                .iter()
                .all(|(name, _)| name == "Local"),
            "não sobra alvo fantasma"
        );
    }

    /// Descobrir que uma base marcada sumiu devolve a lista de sumidas (R7).
    /// CONTRACT: IF-227
    #[test]
    fn a_reload_that_loses_a_marked_database_reports_it() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], &["dev".to_string(), "prod_a".to_string()]);
        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "prod_a", true);

        // A base `prod_a` sumiu do servidor.
        let lost = state.discover(&servers[0], &["dev".to_string()]);
        assert_eq!(
            lost,
            vec![("Local".to_string(), "prod_a".to_string())],
            "a tela precisa saber o que sumiu"
        );
        assert_eq!(state.marked_on("Local"), 1, "a que ficou continua marcada");
    }

    /// Base barrada por `excludePatterns` **nunca** é marcável (R9).
    /// CONTRACT: IF-225
    #[test]
    fn a_blocked_database_can_never_be_marked() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(
            &servers[0],
            &[
                "dev".to_string(),
                "postgres".to_string(),
                "template_zero".to_string(),
            ],
        );

        assert!(!state.is_selectable("Local", "postgres"));
        assert!(!state.is_selectable("Local", "template_zero"));
        assert!(state.is_selectable("Local", "dev"));

        // Mesmo que a tela peça, não entra.
        state.set_marked("Local", "postgres", true);
        assert_eq!(state.marked_count(), 0);

        // E o Apply da query também não a marca.
        let blocked = ServerSelection {
            server: "Local".to_string(),
            databases: vec![
                choice("dev"),
                DatabaseChoice {
                    name: "postgres".to_string(),
                    blocked_by: Some("postgres".to_string()),
                    warning: None,
                },
            ],
        };
        state.apply(&[blocked], &ApplyScope::All, "Produção");
        assert_eq!(state.marked_count(), 1, "só a `dev` entrou");
        assert_eq!(
            state.rows(&servers)[1].info.blocked_by.as_deref(),
            Some("postgres"),
            "a barrada aparece na lista, com o padrão"
        );
    }

    /// O `Run query` leva só servidor com base marcada (R10).
    /// CONTRACT: IF-227
    #[test]
    fn the_targets_skip_servers_without_a_marked_database() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.apply(
            &[selection("Local", &["dev"]), selection("Produção", &[])],
            &ApplyScope::All,
            "Produção",
        );

        let targets = state.targets(&servers);
        assert_eq!(targets.len(), 1, "o servidor vazio fica de fora");
        assert_eq!(targets[0].0, "Local");
        assert_eq!(targets[0].1, vec!["dev".to_string()]);
    }

    /// A lista pagina e a busca filtra por base e por servidor.
    /// CONTRACT: IF-227
    #[test]
    fn the_list_pages_and_searches() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..PAGE_SIZE + 5)
            .map(|index| format!("base_{index:02}"))
            .collect();
        state.discover(&servers[0], &many);

        assert_eq!(state.page_count(&servers), 2);
        assert_eq!(state.page_rows(&servers).len(), PAGE_SIZE);
        assert!(
            state
                .page_rows(&servers)
                .iter()
                .all(|row| row.server == "Local")
        );

        state.turn_page(&servers, true);
        let second = state.page_rows(&servers);
        assert_eq!(second.len(), 6, "5 bases + o servidor sem base descoberta");
        assert_eq!(
            second.last().map(|row| row.database.as_str()),
            Some(""),
            "servidor sem base vira a linha de dica, para o ⟳ Discover da linha existir"
        );

        // Não passa do fim nem volta antes do começo.
        state.turn_page(&servers, true);
        assert_eq!(state.page(&servers), 1);
        state.turn_page(&servers, false);
        state.turn_page(&servers, false);
        assert_eq!(state.page(&servers), 0);

        // Busca pelo nome do servidor traz tudo dele.
        state.set_search("produção".to_string());
        let rows = state.rows(&servers);
        assert_eq!(rows.len(), 1, "só o servidor sem bases conhecidas");
        assert_eq!(rows[0].server, "Produção");
        assert_eq!(rows[0].database, "", "linha de dica, sem checkbox");
    }

    /// Uma base com aviso de credencial **não** é marcada pelo `Apply` (R4b).
    ///
    /// Ela continua na lista, com o aviso, e o dono decide marcá-la à mão. Marcar sozinha seria
    /// prometer uma execução que não tem senha para o usuário pedido.
    /// CONTRACT: IF-226
    #[test]
    fn a_warned_database_is_not_marked_by_apply() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        let selection = ServerSelection {
            server: "Local".to_string(),
            databases: vec![
                choice("dev"),
                DatabaseChoice {
                    name: "test2".to_string(),
                    blocked_by: None,
                    warning: Some(
                        "user 'ana' has no password in the query; the vault only holds 'postgres'"
                            .to_string(),
                    ),
                },
            ],
        };
        state.apply(&[selection], &ApplyScope::All, "Produção");

        assert_eq!(state.marked_count(), 1, "só a linha pronta para rodar");
        assert!(
            state.rows(&servers)[1].info.warning.is_some(),
            "a com aviso continua na lista, com o motivo"
        );

        // E não é barrada de vez: marcar à mão continua possível.
        state.set_marked("Local", "test2", true);
        assert_eq!(state.marked_count(), 2);
    }

    /// Servidor do cadastro sem base conhecida **aparece** na lista (R2).
    ///
    /// Sem a linha ele nem seria desenhado, e o `⟳ Discover` do cabeçalho ficaria inalcançável.
    /// CONTRACT: IF-227
    #[test]
    fn a_server_without_known_databases_still_shows_up() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        let rows = state.rows(&servers);
        assert_eq!(rows.len(), 2, "os dois servidores do cadastro");
        assert!(rows.iter().all(|row| row.database.is_empty()));
        assert_eq!(state.page_count(&servers), 1);
    }
}
