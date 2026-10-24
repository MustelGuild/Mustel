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
    /// Servidor do filtro (`None` = todos).
    server_filter: Option<String>,
    /// Quantas linhas da lista plana já foram reveladas quando a paginação está ligada.
    revealed: usize,
}

/// O que o `Select all` fez, para a tela poder contar o que ficou de fora.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkSummary {
    /// Bases marcadas.
    pub marked: usize,
    /// Bases barradas por `excludePatterns` (não entram nunca).
    pub excluded: usize,
    /// Bases com aviso de credencial (R4b: não entram sozinhas).
    pub no_password: usize,
}

impl MarkSummary {
    /// Texto do status: o que entrou e, quando houver, o que foi pulado e por quê.
    pub fn describe(&self) -> String {
        let mut text = format!("{} marked", self.marked);
        if self.excluded > 0 {
            text.push_str(&format!(
                " · {} skipped (excluded by excludePatterns)",
                self.excluded
            ));
        }
        if self.no_password > 0 {
            text.push_str(&format!(" · {} skipped (no password)", self.no_password));
        }
        text
    }
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
        // A carga muda o universo: o "carregar mais" volta ao começo para o resultado aparecer.
        self.revealed = 0;
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

    /// A lista **plana** de bases, na ordem que a tela desenha.
    ///
    /// Sem agrupar por servidor: o servidor é um **filtro** ([`Self::set_server_filter`]) e cada
    /// linha carrega o nome dele (o mesmo nome de base pode existir em mais de um servidor). A ordem
    /// é alfabética pela base, com o servidor desempatando — é o que deixa a lista previsível.
    ///
    /// Durante a busca, o resultado é **completo**: o pedaço da paginação não vale, senão achar a
    /// base procurada vira loteria.
    pub fn rows(&self, servers: &[ServerConfigEntry]) -> Vec<Row> {
        let needle = self.search.trim().to_lowercase();
        let mut rows = Vec::new();

        for server in servers {
            if let Some(filter) = &self.server_filter
                && !server.name.eq_ignore_ascii_case(filter)
            {
                continue;
            }
            let Some(databases) = self.universe.get(&server.name) else {
                continue;
            };
            let marked = self.marked.get(&server.name);
            let server_matches = !needle.is_empty() && server.name.to_lowercase().contains(&needle);

            for (database, info) in databases {
                if !needle.is_empty()
                    && !server_matches
                    && !database.to_lowercase().contains(&needle)
                {
                    continue;
                }
                rows.push(Row {
                    server: server.name.clone(),
                    database: database.clone(),
                    info: info.clone(),
                    marked: marked.is_some_and(|marked| marked.contains(database)),
                });
            }
        }

        rows.sort_by(|left, right| {
            left.database
                .to_lowercase()
                .cmp(&right.database.to_lowercase())
                .then_with(|| left.server.to_lowercase().cmp(&right.server.to_lowercase()))
        });
        rows
    }

    /// As linhas que a tela desenha, respeitando a paginação.
    ///
    /// `page_size = None` (o padrão) desenha tudo. Com busca ativa, também desenha tudo.
    pub fn visible_rows(
        &self,
        servers: &[ServerConfigEntry],
        page_size: Option<usize>,
    ) -> Vec<Row> {
        let rows = self.rows(servers);
        match page_size {
            Some(size) if self.search.trim().is_empty() => {
                let limit = self.revealed.max(size);
                rows.into_iter().take(limit).collect()
            }
            _ => rows,
        }
    }

    /// Quantas bases ainda não foram reveladas (o que o "carregar mais" traz).
    pub fn hidden(&self, servers: &[ServerConfigEntry], page_size: Option<usize>) -> usize {
        let Some(size) = page_size else {
            return 0;
        };
        if !self.search.trim().is_empty() {
            return 0;
        }
        let total = self.rows(servers).len();
        total.saturating_sub(self.revealed.max(size))
    }

    /// Revela mais um pedaço da lista plana.
    ///
    /// O ponto de partida é o que já está **visível** (o primeiro pedaço é a página), não o contador
    /// — senão o primeiro "carregar mais" não acrescentaria nada.
    pub fn reveal_more(&mut self, page_size: usize, servers: &[ServerConfigEntry]) {
        self.revealed = self.visible_rows(servers, Some(page_size)).len() + page_size;
    }

    /// Servidor do filtro (`None` = todos).
    pub fn server_filter(&self) -> Option<&str> {
        self.server_filter.as_deref()
    }

    /// Troca o filtro de servidor. Um filtro novo volta a lista ao primeiro pedaço.
    pub fn set_server_filter(&mut self, server: Option<String>) {
        if self.server_filter != server {
            self.server_filter = server;
            self.revealed = 0;
        }
    }

    /// As bases **visíveis e marcáveis** (nem barradas, nem com aviso de senha).
    ///
    /// É o alvo do `Select all` / `Clear`: o que está na tela, ou seja, filtro × busca.
    fn selectable_visible(&self, servers: &[ServerConfigEntry]) -> Vec<(String, String)> {
        self.rows(servers)
            .into_iter()
            .filter(|row| row.info.blocked_by.is_none() && row.info.warning.is_none())
            .map(|row| (row.server, row.database))
            .collect()
    }

    /// Quantas bases o cadastro conhece (o "N" do filtro "All servers").
    pub fn known_count(&self, servers: &[ServerConfigEntry]) -> usize {
        servers
            .iter()
            .map(|server| self.known_on(&server.name))
            .sum()
    }

    /// Quantas bases o `Select all` marcaria agora (o número do botão).
    pub fn selectable_count(&self, servers: &[ServerConfigEntry]) -> usize {
        self.selectable_visible(servers).len()
    }

    /// Tudo o que está visível já está marcado? (estado do `☑ Select all`)
    pub fn all_visible_marked(&self, servers: &[ServerConfigEntry]) -> bool {
        let visible = self.selectable_visible(servers);
        !visible.is_empty()
            && visible.iter().all(|(server, database)| {
                self.marked
                    .get(server)
                    .is_some_and(|marked| marked.contains(database))
            })
    }

    /// `Select all`: marca **tudo o que está visível** (filtro × busca).
    ///
    /// Barradas por `excludePatterns` e com aviso de senha ficam de fora, e a contagem do que ficou
    /// de fora volta para a tela não esconder o que não entrou.
    pub fn select_all_visible(&mut self, servers: &[ServerConfigEntry]) -> MarkSummary {
        let rows = self.rows(servers);
        let mut summary = MarkSummary::default();
        for row in &rows {
            if row.info.blocked_by.is_some() {
                summary.excluded += 1;
                continue;
            }
            if row.info.warning.is_some() {
                summary.no_password += 1;
                continue;
            }
            self.set_marked(&row.server, &row.database, true);
            summary.marked += 1;
        }
        summary
    }

    /// `Clear`: desmarca tudo o que está visível.
    pub fn clear_visible(&mut self, servers: &[ServerConfigEntry]) -> usize {
        let rows = self.rows(servers);
        let mut cleared = 0;
        for row in &rows {
            self.set_marked(&row.server, &row.database, false);
            cleared += 1;
        }
        cleared
    }

    /// Texto da busca (mudar a busca volta a lista ao primeiro pedaço).
    pub fn set_search(&mut self, search: String) {
        if self.search != search {
            self.search = search;
            self.revealed = 0;
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

    /// A seleção sobrevive a paginar, filtrar e buscar (R7).
    /// CONTRACT: IF-227
    #[test]
    fn the_selection_survives_paging_filtering_and_searching() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..80).map(|index| format!("base_{index:02}")).collect();
        state.discover(&servers[0], &many);

        state.set_marked("Local", "base_00", true);
        state.set_marked("Local", "base_70", true);
        assert_eq!(state.marked_count(), 2);

        state.reveal_more(50, &servers);
        state.set_server_filter(Some("Local".to_string()));
        assert_eq!(state.marked_count(), 2, "filtrar não desmarca");

        state.set_search("base_7".to_string());
        assert_eq!(state.marked_count(), 2, "buscar não desmarca");
        assert_eq!(state.rows(&servers).len(), 10, "base_70..base_79");
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
        let rows = state.rows(&servers);
        let row = rows
            .iter()
            .find(|row| row.database == "postgres")
            .expect("a barrada aparece na lista");
        assert_eq!(
            row.info.blocked_by.as_deref(),
            Some("postgres"),
            "com o padrão que a barrou"
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

    /// A lista é **plana e alfabética**, com o servidor em cada linha; o servidor é filtro.
    /// CONTRACT: IF-227
    #[test]
    fn the_list_is_flat_alphabetical_and_filtered_by_server() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], &["dev".to_string(), "alpha".to_string()]);
        state.discover(&servers[1], &["dev".to_string(), "zulu".to_string()]);

        let rows = state.rows(&servers);
        let names: Vec<&str> = rows.iter().map(|row| row.database.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha", "dev", "dev", "zulu"],
            "alfabetica, com o mesmo nome uma vez por servidor"
        );
        assert_eq!(rows[1].server, "Local", "empate desempatado pelo servidor");
        assert_eq!(rows[2].server, "Produção");

        state.set_server_filter(Some("produção".to_string()));
        let rows = state.rows(&servers);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.server == "Produção"));
        assert_eq!(state.server_filter(), Some("produção"));
    }

    /// Busca casa nome da base **e** nome do servidor, sem diferenciar maiúsculas.
    /// CONTRACT: IF-227
    #[test]
    fn searching_matches_uppercase_and_server_name() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], &["dev".to_string(), "test2".to_string()]);

        state.set_search("LOCAL".to_string());
        assert_eq!(state.rows(&servers).len(), 2, "nome do servidor casa");

        state.set_search("DEV".to_string());
        assert_eq!(state.rows(&servers).len(), 1, "sem diferenciar maiusculas");
    }

    /// `Select all` marca **o que esta visivel** (filtro x busca) e conta o que pulou.
    /// CONTRACT: IF-227
    #[test]
    fn select_all_marks_what_is_visible_and_reports_what_it_skipped() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(
            &servers[0],
            &[
                "dev".to_string(),
                "test2".to_string(),
                "postgres".to_string(),
                "template0".to_string(),
            ],
        );
        state.discover(&servers[1], &["dev".to_string()]);

        assert_eq!(state.selectable_count(&servers), 3);
        let summary = state.select_all_visible(&servers);
        assert_eq!(summary.marked, 3);
        assert_eq!(
            summary.excluded, 2,
            "postgres e template0 casam com os padroes"
        );
        assert_eq!(summary.no_password, 0);
        assert!(
            summary
                .describe()
                .contains("2 skipped (excluded by excludePatterns)")
        );
        assert!(state.all_visible_marked(&servers));

        state.clear_visible(&servers);
        assert_eq!(state.marked_count(), 0);
        state.set_server_filter(Some("Produção".to_string()));
        let summary = state.select_all_visible(&servers);
        assert_eq!(summary.marked, 1, "so o dev do Producao");
        assert_eq!(
            state.marked_on("Local"),
            0,
            "o outro servidor nao foi tocado"
        );

        state.set_server_filter(None);
        state.clear_visible(&servers);
        state.set_search("test".to_string());
        let summary = state.select_all_visible(&servers);
        assert_eq!(summary.marked, 1, "so test2");
        assert_eq!(state.marked_on("Local"), 1);
    }

    /// Base com aviso de senha **nao** entra no `Select all` (R4b), mas da para marcar a mao.
    /// CONTRACT: IF-226
    #[test]
    fn a_warned_database_is_not_marked_by_select_all() {
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
                    warning: Some("user 'ana' has no password in the query".to_string()),
                },
            ],
        };
        state.apply(&[selection], &ApplyScope::All, "Produção");

        assert_eq!(state.marked_count(), 1);
        let summary = state.select_all_visible(&servers);
        assert_eq!(summary.marked, 1, "dev ja estava; a com aviso fica de fora");
        assert_eq!(summary.no_password, 1);
        assert!(summary.describe().contains("1 skipped (no password)"));

        state.set_marked("Local", "test2", true);
        assert_eq!(state.marked_on("Local"), 2);
    }

    /// A paginacao e opcional: desligada desenha tudo, ligada revela em pedacos.
    /// CONTRACT: IF-227
    #[test]
    fn pagination_is_optional_and_reveals_in_chunks() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..120).map(|index| format!("base_{index:03}")).collect();
        state.discover(&servers[0], &many);

        assert_eq!(state.visible_rows(&servers, None).len(), 120);
        assert_eq!(state.hidden(&servers, None), 0);

        assert_eq!(state.visible_rows(&servers, Some(50)).len(), 50);
        assert_eq!(state.hidden(&servers, Some(50)), 70);

        state.reveal_more(50, &servers);
        assert_eq!(state.visible_rows(&servers, Some(50)).len(), 100);
        state.reveal_more(50, &servers);
        assert_eq!(state.visible_rows(&servers, Some(50)).len(), 120);
        assert_eq!(state.hidden(&servers, Some(50)), 0);

        state.set_search("base_119".to_string());
        assert_eq!(state.visible_rows(&servers, Some(50)).len(), 1);
        assert_eq!(state.hidden(&servers, Some(50)), 0);
    }

    /// **Estresse**: 1000 bases num servidor continuam baratas de desenhar.
    ///
    /// O custo de montar a lista é o que a tela paga por quadro, com paginação ou sem. Este teste
    /// guarda o teto: se alguém trocar o algoritmo por algo quadrático, ele cai aqui.
    /// CONTRACT: IF-227
    #[test]
    fn a_thousand_databases_stay_cheap_to_render() {
        let servers = servers();
        let mut state = SelectionState::default();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..1000).map(|index| format!("db_{index:04}")).collect();
        state.discover(&servers[0], &many);

        let started = std::time::Instant::now();
        for _ in 0..100 {
            let rows = state.rows(&servers);
            assert_eq!(rows.len(), 1000, "sem paginação, a lista é completa");
        }
        let per_frame = started.elapsed() / 100;

        assert!(
            per_frame < std::time::Duration::from_millis(4),
            "montar a lista de 1000 bases custou {per_frame:?} por quadro"
        );
    }
}
