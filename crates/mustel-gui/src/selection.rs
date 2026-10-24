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
use mustel_core::config::model::SelectionQuery;
use mustel_core::query::discovery;
use mustel_core::usecases::custom_databases::ServerSelection;
use mustel_core::usecases::db_servers::CatalogDatabase;

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
    /// O **próprio servidor** disse que ela não serve como alvo (`template` ou
    /// `datallowconn = false`), com o motivo pronto para o tooltip.
    ///
    /// Não é padrão do cadastro: é um fato do catálogo (`pg_database`). Fica separado para o motivo
    /// mostrado ser o certo, e para um seletor que nomeie a base não "desbloquear" o que o
    /// servidor já recusou.
    pub unusable: Option<String>,
    /// Aviso não bloqueante (usuário da string sem senha, por exemplo).
    pub warning: Option<String>,
}

impl Info {
    /// A base está barrada (padrão do cadastro **ou** recusa do servidor)?
    pub fn is_blocked(&self) -> bool {
        self.blocked_by.is_some() || self.unusable.is_some()
    }

    /// O motivo a mostrar no tooltip da linha desabilitada.
    pub fn reason(&self) -> Option<&str> {
        match (&self.blocked_by, &self.unusable) {
            (Some(pattern), _) => Some(pattern),
            (None, Some(unusable)) => Some(unusable),
            (None, None) => None,
        }
    }
}

/// Converte nomes em catálogo **utilizável** (não é template, aceita conexão).
///
/// Atalho para quem só tem nomes — teste e tela falam de "bases", não de flags do catálogo.
pub fn catalog<const N: usize>(names: [&str; N]) -> Vec<CatalogDatabase> {
    names
        .iter()
        .map(|name| CatalogDatabase::usable(*name))
        .collect()
}

/// O servidor de onde a query **lê** não está mais no cadastro?
///
/// O seletor é global e continua válido como cadastro — mas não roda sem a fonte. Quem detecta isso
/// **antes** de disparar o job é a tela: mandar e receber erro só para dizer o que já se sabe
/// gastaria uma conexão e assustaria o dono com um erro de rede.
pub fn missing_source(query: &SelectionQuery, servers: &[ServerConfigEntry]) -> Option<String> {
    let known = servers
        .iter()
        .any(|server| server.name.eq_ignore_ascii_case(&query.server));
    (!known).then(|| query.server.clone())
}

/// O motivo, pronto para o tooltip, de a base não servir como alvo — ou `None` se servir.
///
/// Vem do catálogo, não de heurística de nome: `template0` não aceita conexão (`datallowconn =
/// false`) e `template1` é template; rodar ou restaurar neles não é o que se quer.
fn unusable_reason(database: &CatalogDatabase) -> Option<String> {
    match (database.template, database.allow_connections) {
        (false, true) => None,
        (true, false) => Some("template database (datallowconn = false)".to_string()),
        (true, true) => Some("template database".to_string()),
        (false, false) => {
            Some("database does not allow connections (datallowconn = false)".to_string())
        }
    }
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
    /// Qual seletor produziu a seleção de cada servidor (`servidor -> nome da query`).
    ///
    /// É o que permite **seletores diferentes por servidor**: filtrar um servidor e escolher outra
    /// query aplica só nele. A memória é do rótulo ("de onde veio"), não uma promessa de que a
    /// seleção atual saiu inteira dele — marcar à mão depois não apaga o rótulo.
    server_selectors: BTreeMap<String, String>,
    /// O que o seletor de cada servidor marcou quando foi aplicado (servidor -> bases).
    ///
    /// É o alvo do `Reset`: desfazer o que foi marcado à mão é voltar a este conjunto. Guardar o
    /// resultado (em vez de reexecutar a query no clique) deixa o botão instantâneo e sem modo de
    /// falha — para trazer o que mudou no banco, aplica-se o seletor de novo.
    selector_marks: BTreeMap<String, BTreeSet<String>>,
    /// Texto da busca.
    search: String,
    /// Servidor do filtro (`None` = todos).
    server_filter: Option<String>,
    /// Quantas linhas da lista plana já foram reveladas quando a paginação está ligada.
    revealed: usize,
    /// A lista mostra **só as marcadas**? (visão, não ação: `Select all`/`Clear` não mudam)
    show_only_marked: bool,
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
        if let Some(skips) = self.describe_skips() {
            text.push_str(&format!(" · {skips}"));
        }
        text
    }

    /// Só o que ficou **de fora**, para quando o "quantas marcadas" já está na tela.
    ///
    /// A sidebar mostra uma linha derivada (`103 marked · from 'Producao'`) que é sempre a verdade
    /// atual; repetir a contagem no recado da última ação só duplicava o mesmo número.
    pub fn describe_skips(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.excluded > 0 {
            parts.push(format!(
                "{} skipped (excluded by excludePatterns)",
                self.excluded
            ));
        }
        if self.no_password > 0 {
            parts.push(format!("{} skipped (no password)", self.no_password));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
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

    /// Quantas das bases **listadas** (filtro × busca, e só as marcadas quando o toggle está
    /// ligado) estão marcadas.
    ///
    /// É o "N de M" do cabeçalho da lista, e por isso os dois números saem do mesmo conjunto: o
    /// marcado do escopo inteiro sobre um total filtrado produzia `3 of 1` assim que uma busca
    /// deixava uma linha só. Com o `Show only marked` ligado, é o que a lista mostra (tudo marcado).
    pub fn marked_in_filter(&self, servers: &[ServerConfigEntry]) -> usize {
        self.display_rows(servers)
            .iter()
            .filter(|row| row.marked)
            .count()
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
            .is_some_and(|info| !info.is_blocked())
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
    ///
    /// Devolve o que aconteceu (marcadas e puladas), para a tela contar sem recalcular.
    pub fn apply(
        &mut self,
        selection: &[ServerSelection],
        scope: &ApplyScope,
        query: &str,
    ) -> MarkSummary {
        match scope {
            ApplyScope::All => self.marked.clear(),
            ApplyScope::Server(name) => {
                self.marked.remove(name);
            }
        }

        let mut summary = MarkSummary::default();
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
                // A recusa do **servidor** (template, `datallowconn = false`) é um fato do catálogo,
                // e a query não sabe dele: preservar o que a descoberta estabeleceu impede que um
                // seletor que nomeia `template0` o torne selecionável.
                let unusable = entry
                    .get(&database.name)
                    .and_then(|info| info.unusable.clone());
                entry.insert(
                    database.name.clone(),
                    Info {
                        blocked_by: database.blocked_by.clone(),
                        unusable,
                        warning: database.warning.clone(),
                    },
                );
                // Barrada por padrão **ou** recusada pelo servidor, e com aviso de credencial, não
                // entram sozinhas (R9, R4b): o `Apply` marca o que está pronto para rodar, e o que
                // não está aparece na lista com o motivo — em vez de virar um erro só na hora de
                // executar. Olhar o `Info` **depois** de montar é o que impede o `template0` de ser
                // marcado (a query não sabe da recusa do servidor, mas o universo sabe).
                let info = entry.get(&database.name).cloned().unwrap_or_default();
                if info.is_blocked() {
                    summary.excluded += 1;
                } else if info.warning.is_some() {
                    summary.no_password += 1;
                } else {
                    selected.insert(database.name.clone());
                }
            }
            if !selected.is_empty() {
                summary.marked += selected.len();
                // O `Reset` volta a este conjunto: é o que o seletor marcou, antes de qualquer
                // marcação à mão.
                self.selector_marks
                    .insert(server.server.clone(), selected.clone());
                self.marked.insert(server.server.clone(), selected);
                // O servidor passou a usar **este** seletor.
                self.server_selectors
                    .insert(server.server.clone(), query.to_string());
            }
        }

        self.query = Some(query.to_string());
        // A carga muda o universo: o "carregar mais" volta ao começo para o resultado aparecer.
        self.revealed = 0;
        summary
    }

    /// O `⟳ Discover`: preenche o universo do servidor com o que o catálogo devolveu (R6).
    ///
    /// Entra **tudo**, inclusive `template0` e bases com `datallowconn = false`: elas aparecem
    /// apagadas com o motivo, e é isso que faz a contagem do servidor ser o que ele tem — se a
    /// descoberta escondesse o `template0`, um seletor que o nomeasse mudaria a contagem de 104 para
    /// 105 (bug relatado pelo dono).
    ///
    /// Bases que só a query de seleção conhecia continuam marcadas — descobrir **não** desmarca.
    /// Devolve as bases que estavam marcadas e sumiram, para a tela avisar (R7).
    pub fn discover(
        &mut self,
        server: &ServerConfigEntry,
        catalog: impl AsRef<[CatalogDatabase]>,
    ) -> Vec<(String, String)> {
        let catalog = catalog.as_ref();
        let entry = self.universe.entry(server.name.clone()).or_default();

        // O padrão do cadastro vale para todo o universo, inclusive para o que veio daqui. Também
        // vale o que o servidor diz de si mesmo (template/`datallowconn`).
        for database in catalog {
            let blocked_by = server
                .exclude_patterns
                .iter()
                .find(|pattern| discovery::matches_pattern(&database.name, pattern))
                .cloned();
            entry.insert(
                database.name.clone(),
                Info {
                    blocked_by,
                    unusable: unusable_reason(database),
                    ..Info::default()
                },
            );
        }

        let known: BTreeSet<&str> = catalog
            .iter()
            .map(|database| database.name.as_str())
            .collect();
        let mut lost = Vec::new();
        if let Some(marked) = self.marked.get_mut(&server.name) {
            let gone: Vec<String> = marked
                .iter()
                .filter(|database| !known.contains(database.as_str()))
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
        self.server_selectors
            .retain(|name, _| alive.contains(name.as_str()));
        self.selector_marks
            .retain(|name, _| alive.contains(name.as_str()));

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

    /// As linhas que a tela **desenha**: filtro × busca, e só as marcadas quando o
    /// `Show only marked` está ligado.
    ///
    /// A distinção importa: o `Select all`/`Clear` continuam agindo sobre [`rows`](Self::rows) — a
    /// visão é um recorte da tela, não uma mudança de significado dos botões. Se o toggle mexesse na
    /// ação, "Select all" com o filtro ligado só remarcaria o que já estava marcado.
    fn display_rows(&self, servers: &[ServerConfigEntry]) -> Vec<Row> {
        let rows = self.rows(servers);
        if self.show_only_marked {
            rows.into_iter().filter(|row| row.marked).collect()
        } else {
            rows
        }
    }

    /// As linhas que a tela desenha, respeitando a paginação.
    ///
    /// `page_size = None` (o padrão) desenha tudo. Com busca ativa, também desenha tudo.
    pub fn visible_rows(
        &self,
        servers: &[ServerConfigEntry],
        page_size: Option<usize>,
    ) -> Vec<Row> {
        let rows = self.display_rows(servers);
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
        let total = self.display_rows(servers).len();
        total.saturating_sub(self.revealed.max(size))
    }

    /// Revela mais um pedaço da lista plana.
    ///
    /// O ponto de partida é o que já está **visível** (o primeiro pedaço é a página), não o contador
    /// — senão o primeiro "carregar mais" não acrescentaria nada.
    pub fn reveal_more(&mut self, page_size: usize, servers: &[ServerConfigEntry]) {
        self.revealed = self.visible_rows(servers, Some(page_size)).len() + page_size;
    }

    /// A lista mostra só as marcadas?
    pub fn show_only_marked(&self) -> bool {
        self.show_only_marked
    }

    /// Liga/desliga o "só as marcadas". Troca de visão volta ao primeiro pedaço da paginação.
    pub fn set_show_only_marked(&mut self, value: bool) {
        if self.show_only_marked != value {
            self.show_only_marked = value;
            self.revealed = 0;
        }
    }

    /// Esquece o seletor de **todos** os servidores (a vassoura do combo).
    ///
    /// Não desmarca nada: o que a query marcou continua marcado, só deixa de ser atribuído a ela.
    /// Independe do filtro — a vassoura é sobre "todos", não sobre o escopo da tela.
    pub fn forget_all_selectors(&mut self) {
        self.server_selectors.clear();
        // Sem rótulo não há "o que o seletor marcou": o `Reset` volta a ser `Clear`.
        self.selector_marks.clear();
    }

    /// Quantos servidores têm seletor guardado (para o botão só aparecer quando há o que limpar).
    pub fn selectors_count(&self) -> usize {
        self.server_selectors.len()
    }

    /// Esquece o seletor do **escopo** (a linha "nenhum" do combo).
    ///
    /// Nao desmarca nada: so para de atribuir a selecao atual a uma query.
    pub fn forget_scope_selector(&mut self, servers: &[ServerConfigEntry]) {
        match self.server_filter.clone() {
            Some(name) => {
                self.server_selectors.remove(&name);
                self.selector_marks.remove(&name);
            }
            None => {
                for server in servers {
                    self.server_selectors.remove(&server.name);
                    self.selector_marks.remove(&server.name);
                }
            }
        }
    }

    /// Qual seletor está aplicado num servidor (`None` = nenhum).
    pub fn selector_for(&self, server: &str) -> Option<&str> {
        self.server_selectors.get(server).map(String::as_str)
    }

    /// Qual seletor o **escopo** está usando.
    ///
    /// Com filtro num servidor é o dele. Com "todos os servidores" é o de
    /// [`selector_for_all`](Self::selector_for_all).
    pub fn selector_for_scope(&self, servers: &[ServerConfigEntry]) -> Option<String> {
        match &self.server_filter {
            Some(name) => self.selector_for(name).map(str::to_string),
            None => self.selector_for_all(servers),
        }
    }

    /// Qual seletor vale para **todos** os servidores do cadastro (independente do filtro).
    ///
    /// Só há resposta quando `todos` têm o **mesmo** seletor — e "não tem nenhum" conta como um
    /// valor: com filtro em `Local` e `Producao` aplicado só nele, o escopo "todos" **não** está em
    /// `Producao`. Ignorar os servidores sem seletor (como era antes) fazia o combo e a linha
    /// "All servers" afirmarem um seletor que só um servidor tinha.
    pub fn selector_for_all(&self, servers: &[ServerConfigEntry]) -> Option<String> {
        // O primeiro servidor do cadastro já decide: se ele não tem seletor, o escopo também não
        // tem (nem todos concordariam em ter nenhum, nem há um nome para mostrar).
        let first = self.selector_for(&servers.first()?.name)?;
        servers
            .iter()
            .all(|server| {
                self.selector_for(&server.name)
                    .is_some_and(|selector| selector.eq_ignore_ascii_case(first))
            })
            .then(|| first.to_string())
    }

    /// De onde vem a seleção do escopo, para o status (`from 'Producao'`).
    ///
    /// Com seletores diferentes no escopo não existe "o" nome: dizer *vários* é mais honesto que
    /// omitir a origem ou eleger um deles.
    pub fn scope_origin(&self, servers: &[ServerConfigEntry]) -> Option<String> {
        if let Some(name) = self.selector_for_scope(servers) {
            return Some(name);
        }
        let in_scope = |server: &ServerConfigEntry| match &self.server_filter {
            Some(filter) => server.name.eq_ignore_ascii_case(filter),
            None => true,
        };
        let distinct: BTreeSet<&str> = servers
            .iter()
            .filter(|server| in_scope(server))
            .filter_map(|server| self.selector_for(&server.name))
            .collect();
        (distinct.len() > 1).then(|| "several selectors".to_string())
    }

    /// Esquece os seletores que saíram do cadastro (a query foi removida em Settings).
    ///
    /// Sem isto o combo de um servidor continuava mostrando o nome de uma query extinta — o mesmo
    /// problema que o `retain_selection_queries` resolvia para a escolha única.
    pub fn retain_selectors(&mut self, known: &[String]) {
        self.server_selectors
            .retain(|_, selector| known.iter().any(|name| name.eq_ignore_ascii_case(selector)));
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
            .filter(|row| !row.info.is_blocked() && row.info.warning.is_none())
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
            if row.info.is_blocked() {
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

    /// `Clear`/`Reset`: devolve o que está visível (filtro × busca) ao que o **seletor** marcou.
    ///
    /// Sem seletor o alvo é o vazio, então o efeito é o `Clear` de sempre. Com seletor, o que foi
    /// marcado à mão **sai** e o que foi desmarcado à mão **volta** — é desfazer a bagunça e ficar
    /// com a decisão da query outra vez. O escopo não muda: o mesmo filtro × busca de antes.
    ///
    /// Devolve `(desmarcadas, remarcadas)` para a tela contar o que aconteceu.
    pub fn reset_visible(&mut self, servers: &[ServerConfigEntry]) -> (usize, usize) {
        let rows = self.rows(servers);
        let mut cleared = 0;
        let mut restored = 0;
        for row in &rows {
            let target = self
                .selector_marks
                .get(&row.server)
                .is_some_and(|marks| marks.contains(&row.database));
            if row.marked == target {
                continue;
            }
            self.set_marked(&row.server, &row.database, target);
            if target {
                restored += 1;
            } else {
                cleared += 1;
            }
        }
        (cleared, restored)
    }

    /// O escopo tem algum seletor aplicado? (o botão vira `Reset` e passa a ter o que restaurar)
    pub fn has_selector_in_scope(&self, servers: &[ServerConfigEntry]) -> bool {
        match &self.server_filter {
            Some(name) => self.selector_for(name).is_some(),
            None => servers
                .iter()
                .any(|server| self.selector_for(&server.name).is_some()),
        }
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

    /// Converte uma lista de nomes já montada (dinâmica) em catálogo utilizável.
    fn catalog_from(names: &[String]) -> Vec<CatalogDatabase> {
        names
            .iter()
            .map(|name| CatalogDatabase::usable(name.clone()))
            .collect()
    }

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

    /// Cada servidor lembra **o próprio** seletor: trocar de escopo não apaga o do outro.
    ///
    /// É a regra central do combo que aplica: filtrar um servidor e escolher age só nele; com
    /// "todos" a aplicação vale para todos (e passa a valer).
    /// CONTRACT: IF-237
    #[test]
    fn each_server_remembers_its_own_selector() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        // Filtrando o Local, aplico `Producao` — só nele.
        state.set_server_filter(Some("Local".to_string()));
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::Server("Local".to_string()),
            "Producao",
        );
        assert_eq!(state.selector_for("Local"), Some("Producao"));
        assert_eq!(state.selector_for("Produção"), None);
        assert_eq!(
            state.selector_for_scope(&servers).as_deref(),
            Some("Producao"),
            "o escopo é o Local: o combo mostra o seletor dele"
        );

        // Troco para o Produção e aplico `Homologacao`: cada um fica com o seu.
        state.set_server_filter(Some("Produção".to_string()));
        state.apply(
            &[selection("Produção", &["homem_a"])],
            &ApplyScope::Server("Produção".to_string()),
            "Homologacao",
        );
        assert_eq!(state.selector_for("Local"), Some("Producao"));
        assert_eq!(state.selector_for("Produção"), Some("Homologacao"));

        // Sem filtro os dois aparecem, mas eles **discordam**: o combo fica vazio.
        state.set_server_filter(None);
        assert_eq!(state.selector_for_scope(&servers), None);
        assert_eq!(
            state.scope_origin(&servers).as_deref(),
            Some("several selectors"),
            "o status não elege um dos dois"
        );

        // Aplicar em "todos" passa a valer para todos: o combo volta a ter um nome só.
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::All,
            "Comum",
        );
        assert_eq!(state.selector_for_scope(&servers).as_deref(), Some("Comum"));
        assert_eq!(state.scope_origin(&servers).as_deref(), Some("Comum"));
    }

    /// Base que **só o seletor** conhece entra no universo: aparece (barrada, se casar com o padrão)
    /// e **conta** no total do servidor, mas nunca é marcada.
    ///
    /// Foi o caso real do dono: a `todas` devolvia `template0`, que o `pg_database` daquele servidor
    /// não tem; ao aplicar, o `Local` foi de 104 para 105. Decisão dele: manter visível, apagado e
    /// contando — o que o seletor nomeou não pode desaparecer em silêncio.
    /// CONTRACT: IF-237
    #[test]
    fn a_database_only_the_selector_knows_is_listed_blocked_and_counted() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev"]));
        assert_eq!(state.known_on("Local"), 1);

        let summary = state.apply(
            &[ServerSelection {
                server: "Local".to_string(),
                databases: vec![
                    choice("dev"),
                    DatabaseChoice {
                        // Nome que a descoberta não viu e que casa com `template*`.
                        name: "template0".to_string(),
                        blocked_by: Some("template*".to_string()),
                        warning: None,
                    },
                ],
            }],
            &ApplyScope::All,
            "todas",
        );

        assert_eq!(
            state.known_on("Local"),
            2,
            "o nome que só o seletor conhece entra na contagem"
        );
        assert_eq!(summary.marked, 1, "só o `dev` foi marcado");
        assert_eq!(
            summary.excluded, 1,
            "o `template0` foi contado como barrado"
        );
        assert_eq!(state.marked_on("Local"), 1);
        assert!(
            !state.is_selectable("Local", "template0"),
            "aparece barrado: não pode ser selecionado"
        );
        state.set_marked("Local", "template0", true);
        assert_eq!(
            state.marked_on("Local"),
            1,
            "marcar à mão também é recusado"
        );

        // E está na lista, com o padrão que o barrou (para o tooltip da tela).
        let row = state
            .rows(&servers)
            .into_iter()
            .find(|row| row.database == "template0")
            .expect("a linha existe");
        assert_eq!(row.info.blocked_by.as_deref(), Some("template*"));
        assert!(!row.marked);
    }

    /// O "N de M" do cabeçalho da lista sai **do mesmo conjunto** (filtro × busca).
    ///
    /// Bug visto na janela: marcadas 3 no escopo e uma busca que deixa uma linha mostravam
    /// `3 of 1`.
    /// CONTRACT: IF-237
    #[test]
    fn the_list_header_counts_marked_inside_the_filter() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a", "test_a"]));
        state.discover(&servers[1], catalog(["dev"]));
        state.select_all_visible(&servers);
        assert_eq!(state.marked_count(), 4);
        assert_eq!(state.marked_in_filter(&servers), 4);

        // A busca deixa uma linha: o cabeçalho passa a falar de uma linha só.
        state.set_search("prod_".to_string());
        assert_eq!(state.marked_in_filter(&servers), 1);

        // Desmarcando pela busca, o cabeçalho acompanha.
        state.reset_visible(&servers);
        assert_eq!(state.marked_in_filter(&servers), 0);
        assert_eq!(state.marked_count(), 3, "o resto continua marcado");
    }

    /// A descoberta traz o **catálogo inteiro**: template entra na conta, apagado, e aplicar um
    /// seletor que o nomeia **não** mexe no total.
    ///
    /// É o bug relatado pelo dono pela segunda vez: a descoberta escondia o `template0`, então a
    /// `todas` que o nomeava fazia o `Local` ir de 104 para 105. Agora ele já está na lista desde a
    /// descoberta, com o motivo do **próprio servidor** (não é `excludePatterns`), e a aplicação
    /// preserva essa recusa.
    /// CONTRACT: IF-236
    #[test]
    fn the_catalog_brings_templates_and_the_count_does_not_move() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        // Descoberta: `dev` + `postgres` (barrado pelo padrão) + `template0` (recusa do servidor).
        state.discover(
            &servers[0],
            vec![
                CatalogDatabase::usable("dev"),
                CatalogDatabase::usable("postgres"),
                CatalogDatabase {
                    name: "template0".to_string(),
                    template: true,
                    allow_connections: false,
                },
            ],
        );
        assert_eq!(
            state.known_on("Local"),
            3,
            "o template conta como base do servidor"
        );
        assert!(
            !state.is_selectable("Local", "template0"),
            "template não pode ser selecionado"
        );

        // Aplicar um seletor que nomeia o template: nem muda a conta, nem o desbloqueia.
        let summary = state.apply(
            &[ServerSelection {
                server: "Local".to_string(),
                databases: vec![
                    mustel_core::usecases::custom_databases::DatabaseChoice {
                        name: "dev".to_string(),
                        blocked_by: None,
                        warning: None,
                    },
                    mustel_core::usecases::custom_databases::DatabaseChoice {
                        name: "template0".to_string(),
                        // A query **não** sabe da recusa do servidor: vem sem bloqueio.
                        blocked_by: None,
                        warning: None,
                    },
                ],
            }],
            &ApplyScope::All,
            "todas",
        );

        assert_eq!(
            state.known_on("Local"),
            3,
            "a contagem do servidor não muda ao aplicar"
        );
        assert!(
            !state.is_selectable("Local", "template0"),
            "a recusa do servidor sobrevive à aplicação"
        );
        assert!(!state.is_selectable("Local", "postgres"));
        assert_eq!(summary.marked, 1, "só o `dev`");
        assert_eq!(
            summary.excluded, 1,
            "o `template0` é contado como barrado, mesmo sem padrão"
        );
    }

    /// A vassoura esquece o seletor de **todos** os servidores — e não desmarca nada.
    /// CONTRACT: IF-240
    #[test]
    fn the_broom_forgets_every_selector_without_unmarking() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        for name in ["Local", "Produção"] {
            state.apply(
                &[selection(name, &["dev"])],
                &ApplyScope::Server(name.to_string()),
                "todas",
            );
        }
        assert_eq!(state.selectors_count(), 2);
        assert_eq!(state.marked_count(), 2);

        // Mesmo com filtro num servidor, a vassoura é sobre **todos**.
        state.set_server_filter(Some("Local".to_string()));
        state.forget_all_selectors();

        assert_eq!(
            state.selectors_count(),
            0,
            "nenhum servidor fica com seletor"
        );
        assert_eq!(state.selector_for("Local"), None);
        assert_eq!(state.selector_for("Produção"), None);
        assert_eq!(state.selector_for_all(&servers), None);
        assert_eq!(state.marked_count(), 2, "a seleção continua marcada");
    }

    /// `Show only marked` é **visão**: esconde as não marcadas sem mudar o alvo dos botões.
    /// CONTRACT: IF-239
    #[test]
    fn showing_only_marked_hides_rows_but_does_not_change_the_actions() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a", "test_a"]));
        state.discover(&servers[1], catalog(["dev"]));
        state.set_marked("Local", "dev", true);

        assert_eq!(state.visible_rows(&servers, None).len(), 4);
        assert_eq!(state.marked_in_filter(&servers), 1);

        state.set_show_only_marked(true);
        let rows = state.visible_rows(&servers, None);
        assert_eq!(rows.len(), 1, "só a marcada aparece");
        assert_eq!(rows[0].database, "dev");
        assert_eq!(
            state.marked_in_filter(&servers),
            1,
            "o cabeçalho fala do que a lista mostra"
        );

        // `Select all` continua marcando tudo o que o **filtro** alcança, não só o que está à vista.
        let summary = state.select_all_visible(&servers);
        assert_eq!(
            summary.marked, 4,
            "as 4 marcáveis do escopo inteiro (o toggle não estreita a ação)"
        );
        assert_eq!(state.marked_count(), 4);
        assert_eq!(
            state.visible_rows(&servers, None).len(),
            4,
            "agora todas aparecem"
        );

        // Desligar volta a mostrar tudo.
        state.set_show_only_marked(false);
        assert_eq!(state.visible_rows(&servers, None).len(), 4);
    }

    /// Esquecer o seletor do escopo e esquecer **todos** são coisas diferentes.
    /// CONTRACT: IF-240
    #[test]
    fn the_none_row_and_the_broom_differ_in_scope() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        for name in ["Local", "Produção"] {
            state.apply(
                &[selection(name, &["dev"])],
                &ApplyScope::Server(name.to_string()),
                "todas",
            );
        }

        state.set_server_filter(Some("Local".to_string()));
        state.forget_scope_selector(&servers);
        assert_eq!(state.selector_for("Local"), None);
        assert_eq!(
            state.selector_for("Produção"),
            Some("todas"),
            "a linha `(none)` mexe só no escopo"
        );
    }

    /// `Reset` devolve a seleção ao que o **seletor** marcou, desfazendo o que foi feito à mão.
    ///
    /// A decisão do dono: "o clear sempre limpa nossa seleção manual"; com seletor aplicado, ele
    /// limpa tudo **e** volta ao que a query marcou. Sem seletor é o `Clear` de sempre.
    /// CONTRACT: IF-241
    #[test]
    fn reset_brings_back_what_the_selector_marked() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a", "test_a"]));

        // O seletor marca `dev`; depois a mão marca `prod_a` e tira `dev`.
        state.apply(&[selection("Local", &["dev"])], &ApplyScope::All, "todas");
        state.set_marked("Local", "prod_a", true);
        state.set_marked("Local", "dev", false);
        assert_eq!(state.marked_count(), 1, "só o `prod_a` está marcado");

        let (cleared, restored) = state.reset_visible(&servers);
        assert_eq!(cleared, 1, "o `prod_a` (manual) saiu");
        assert_eq!(restored, 1, "o `dev` (do seletor) voltou");
        assert!(state.is_selectable("Local", "dev"));
        assert!(
            state.marked_on("Local") == 1
                && state
                    .rows(&servers)
                    .iter()
                    .any(|row| row.database == "dev" && row.marked),
            "a seleção é a do seletor outra vez"
        );
    }

    /// Sem seletor, `Reset` é o `Clear` de sempre: desmarca e fica vazio.
    /// CONTRACT: IF-241
    #[test]
    fn reset_without_a_selector_is_just_clear() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a"]));
        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "prod_a", true);

        let (cleared, restored) = state.reset_visible(&servers);
        assert_eq!((cleared, restored), (2, 0));
        assert_eq!(state.marked_count(), 0);
    }

    /// Esquecer o seletor tira o alvo do `Reset`: ele volta a ser um `Clear`.
    /// CONTRACT: IF-241
    #[test]
    fn forgetting_the_selector_turns_reset_back_into_clear() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a"]));
        state.apply(&[selection("Local", &["dev"])], &ApplyScope::All, "todas");
        assert!(state.has_selector_in_scope(&servers));

        state.forget_all_selectors();
        assert!(!state.has_selector_in_scope(&servers));

        // Sem alvo, o reset desmarca em vez de remarcar.
        let (cleared, restored) = state.reset_visible(&servers);
        assert_eq!((cleared, restored), (1, 0), "o `dev` sai");
        assert_eq!(state.marked_count(), 0);
    }

    /// O `Reset` respeita filtro × busca: só mexe no que a lista está mostrando.
    /// CONTRACT: IF-241
    #[test]
    fn reset_touches_only_what_the_filter_and_search_show() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.discover(&servers[0], catalog(["dev", "prod_a"]));
        state.apply(&[selection("Local", &["dev"])], &ApplyScope::All, "todas");
        // Bagunça à mão fora do futuro recorte.
        state.set_marked("Local", "prod_a", true);

        state.set_search("prod_".to_string());
        let (cleared, restored) = state.reset_visible(&servers);
        assert_eq!(cleared, 1, "o `prod_a` visível sai");
        assert_eq!(restored, 0, "o `dev` está fora da busca e não é tocado");
        assert_eq!(state.marked_on("Local"), 1, "o `dev` continua marcado");
    }

    /// O escopo "todos" só tem nome quando **todos** concordam — e "nenhum" é um valor.
    ///
    /// Bug relatado pelo dono: com o filtro em `Local` e o seletor aplicado só nele, a linha
    /// `All servers` do dropdown dizia estar no mesmo seletor, porque os servidores **sem** seletor
    /// eram ignorados na comparação. O escopo não estava nele: só o `Local` estava.
    /// CONTRACT: IF-237
    #[test]
    fn the_all_servers_scope_is_named_only_when_every_server_agrees() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        // Aplicado **só** no Local (é o que o filtro manda).
        state.apply(
            &[selection("Local", &["dev"])],
            &ApplyScope::Server("Local".to_string()),
            "todas",
        );

        assert_eq!(
            state.selector_for_all(&servers),
            None,
            "`Local 2` não tem seletor: \"todos\" não está em `todas`"
        );
        assert_eq!(
            state.selector_for_scope(&servers),
            None,
            "sem filtro é o escopo de todos"
        );
        assert_eq!(
            state.scope_origin(&servers),
            None,
            "o status também não pode afirmar a origem"
        );

        // Filtrando o Local, o nome aparece: o escopo é só ele.
        state.set_server_filter(Some("Local".to_string()));
        assert_eq!(state.selector_for_scope(&servers).as_deref(), Some("todas"));
        assert_eq!(
            state.selector_for_all(&servers),
            None,
            "a linha `All servers` **não** herda o seletor do filtro"
        );

        // Aplicando em todos, o nome volta a valer para o escopo inteiro.
        state.set_server_filter(None);
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::All,
            "todas",
        );
        assert_eq!(state.selector_for_all(&servers).as_deref(), Some("todas"));
        assert_eq!(state.selector_for_scope(&servers).as_deref(), Some("todas"));
    }

    /// `(none)` esquece o rótulo do escopo e **não** desmarca base nenhuma.
    /// CONTRACT: IF-237
    #[test]
    fn forgetting_the_selector_does_not_unmark_anything() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        state.apply(
            &[
                selection("Local", &["dev"]),
                selection("Produção", &["prod_a"]),
            ],
            &ApplyScope::All,
            "Producao",
        );
        assert_eq!(state.marked_count(), 2);

        // Com filtro: só o seletor do servidor filtrado sai de cena.
        state.set_server_filter(Some("Local".to_string()));
        state.forget_scope_selector(&servers);
        assert_eq!(state.selector_for("Local"), None);
        assert_eq!(state.selector_for("Produção"), Some("Producao"));
        assert_eq!(state.marked_count(), 2, "esquecer não desmarca");

        // Sem filtro: todos.
        state.set_server_filter(None);
        state.forget_scope_selector(&servers);
        assert_eq!(state.selector_for("Produção"), None);
        assert_eq!(state.marked_count(), 2);
    }

    /// O `apply` conta o que marcou e o que pulou, para o recado da sidebar.
    /// CONTRACT: IF-237
    #[test]
    fn applying_reports_what_it_marked_and_what_it_skipped() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);

        let summary = state.apply(
            &[ServerSelection {
                server: "Local".to_string(),
                databases: vec![
                    choice("dev"),
                    DatabaseChoice {
                        name: "test_a".to_string(),
                        blocked_by: Some("template*".to_string()),
                        warning: None,
                    },
                    DatabaseChoice {
                        name: "homem_a".to_string(),
                        blocked_by: None,
                        warning: Some("user 'ana' has no password".to_string()),
                    },
                ],
            }],
            &ApplyScope::All,
            "Producao",
        );

        assert_eq!(summary.marked, 1);
        assert_eq!(summary.excluded, 1);
        assert_eq!(summary.no_password, 1);
        assert_eq!(
            summary.describe_skips().as_deref(),
            Some("1 skipped (excluded by excludePatterns) · 1 skipped (no password)"),
            "o que entrou fica no status derivado; aqui só o que ficou de fora"
        );
    }

    /// A seleção sobrevive a paginar, filtrar e buscar (R7).
    /// CONTRACT: IF-227
    #[test]
    fn the_selection_survives_paging_filtering_and_searching() {
        let mut state = SelectionState::default();
        let servers = servers();
        state.sync_registry(&servers);
        let many: Vec<String> = (0..80).map(|index| format!("base_{index:02}")).collect();
        state.discover(&servers[0], catalog_from(&many));

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
        state.discover(&servers[0], catalog(["dev", "prod_a"]));
        state.set_marked("Local", "dev", true);
        state.set_marked("Local", "prod_a", true);

        // A base `prod_a` sumiu do servidor.
        let lost = state.discover(&servers[0], catalog(["dev"]));
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
        state.discover(&servers[0], catalog(["dev", "postgres", "template_zero"]));

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
        state.discover(&servers[0], catalog(["dev", "alpha"]));
        state.discover(&servers[1], catalog(["dev", "zulu"]));

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
        state.discover(&servers[0], catalog(["dev", "test2"]));

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
            catalog(["dev", "test2", "postgres", "template0"]),
        );
        state.discover(&servers[1], catalog(["dev"]));

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

        state.reset_visible(&servers);
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
        state.reset_visible(&servers);
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
        state.discover(&servers[0], catalog_from(&many));

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
        state.discover(&servers[0], catalog_from(&many));

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
