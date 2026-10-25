//! O que a janela pede ao núcleo, e o que ela recebe de volta (T42).
//!
//! As telas **devolvem** [`Request`]s em vez de executar: quem tem runtime, canal e token é o
//! [`App`](crate::app::App). É o que mantém o desenho puro e testável.

use std::path::PathBuf;

use mustel_core::config::ServerConfigEntry;

/// Pedido de uma tela para o `App` executar.
///
/// Sem `Eq` porque carrega preferências de aparência com `f32` (escala de fonte); `PartialEq` é o
/// que os testes de tela comparam.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Recarrega o cadastro do disco.
    ReloadServers,
    /// Grava um servidor (cadastra, atualiza ou renomeia).
    SaveServer {
        /// Nome que o servidor tinha quando o formulário abriu (`None` = cadastro novo).
        original: Option<String>,
        /// O servidor como ficou no formulário.
        server: ServerConfigEntry,
    },
    /// Remove um servidor pelo nome.
    RemoveServer(String),
    /// Guarda a senha de um servidor no cofre.
    SetPassword {
        /// Servidor.
        name: String,
        /// Senha em texto (vai direto para o cofre).
        password: String,
    },
    /// Testa a conexão de um servidor.
    TestConnection(ServerConfigEntry),
    /// Cancela o job em andamento.
    CancelJob,
    /// Abre/fecha o painel de conversa com a IA (aba Query).
    ToggleAiPanel,
    /// Vai para a aba Settings (o painel da IA pede quando falta configuração).
    OpenSettings,
    /// Joga o SQL escolhido no editor da aba Query, **substituindo** o conteúdo.
    InsertEditorText(String),
    /// Grava a lista de perfis de IA (seção `gui.ai`) e, quando há uma, a **chave** do perfil.
    ///
    /// Os dois andam juntos de propósito: o formulário salva perfil **e** chave no mesmo clique, e
    /// mandar só a chave (como era antes) deixava o perfil fora do arquivo — o `Test` depois dizia
    /// que ele não existia.
    SaveAiProfiles {
        /// Perfis, como ficaram na tela.
        profiles: Vec<mustel_core::config::model::AiProfile>,
        /// `id` do ativo.
        active: String,
        /// Chave a guardar (perfil + chave), quando o dono digitou uma.
        key: Option<(String, String)>,
    },
    /// Apaga a chave de um perfil (perfil removido).
    RemoveAiKey {
        /// `id` do perfil.
        profile: String,
    },
    /// Faz uma chamada curta ao provedor para conferir perfil e chave.
    TestAiProfile {
        /// `id` do perfil.
        profile: String,
    },
    /// Lê o DDL do banco (do cache, ou baixando) para o painel da IA.
    LoadDdl {
        /// Servidor do cadastro.
        server: String,
        /// Banco escolhido.
        database: String,
        /// Ignora o cache e baixa de novo?
        refresh: bool,
    },
    /// Pede à IA o SQL para a conversa atual.
    GenerateSql {
        /// `id` do perfil (a chave vai do cofre, no executor).
        profile: String,
        /// DDL da sessão (o texto vai junto: o executor não conhece o painel).
        ddl: String,
        /// Histórico da conversa, na ordem.
        messages: Vec<mustel_core::usecases::ai::ChatMessage>,
    },
    /// Descobre os bancos do servidor e se o cofre tem a senha dele.
    ///
    /// É o que alimenta o combo de banco da linha "Load autocomplete": o cadastro pod
    /// e não
    /// listar todos os bancos (ou nenhum, quando o servidor usa `fetchAllDatabases`).
    ///
    /// `None` descobre **todos** os servidores do cadastro, um a um e em melhor esforço: um
    /// servidor fora do ar não pode impedir os outros (é o `⟳ Discover all`).
    DiscoverDatabases {
        /// Servidor do cadastro (`None` = todos).
        server: Option<String>,
    },
    /// A lista de seletores mudou: o app compõe a seção `gui` (aparência + seletores + paginação)
    /// e grava.
    ///
    /// A lista de seletores mudou: o app compõe a seção `gui` (aparência + seletores + salvas)
    /// e grava.
    ///
    /// A tela do cadastro devolve **este** pedido, e não um `SaveGuiPrefs` pronto: ela só
    /// conhece os seletores, e gravar o que ela conhece apagaria a aparência.
    SaveSelections,
    /// A lista de **queries salvas** mudou (aba Query): o app compõe a seção `gui` inteira e grava.
    ///
    /// Mesmo motivo do `SaveSelections`: a aba Query conhece só as salvas, e gravar apenas isso
    /// apagaria tema, fonte e seletores.
    SaveSavedQueries(Vec<mustel_core::config::model::SavedQuery>),
    /// Roda uma query de seleção salva e marca as bases do escopo pedido (R1).
    ///
    /// Aborta a carga inteira quando um host não está no cadastro (D9) — o contrário do
    /// `DiscoverDatabases`, que segue em melhor esforço.
    ApplySelection {
        /// Nome da query salva em `gui.selections`.
        query: String,
        /// Todos os servidores, ou um só.
        scope: crate::selection::ApplyScope,
    },
    /// Carrega o esquema de um banco, para o autocomplete.
    LoadSchema {
        /// Servidor do cadastro.
        server: String,
        /// Banco de referência.
        database: String,
    },
    /// Aplica as preferências de aparência no contexto do `egui` (fonte e escala).
    ApplyGuiPrefs(crate::fonts::Chosen),
    /// Grava as preferências da janela na seção `gui` do `mustel.jsonc`.
    SaveGuiPrefs(mustel_core::config::model::GuiPrefs),
    /// Ensaia uma query de seleção (aba Settings: `Test`).
    ///
    /// Roda o **mesmo** caminho da carga real: abre a conexão do servidor do cadastro e valida as
    /// linhas. Devolve só números, nunca a connection string nem a senha (D12).
    TestSelection(mustel_core::config::model::SelectionQuery),
    /// Executa a consulta nos servidores escolhidos, com as **bases marcadas** (D15).
    ///
    /// O servidor vai pelo nome do cadastro: as bases explícitas são as que a tela marcou, e o
    /// plano roda só elas (`fetch_all_databases: false`, `all: false`). Servidor sem base marcada
    /// nem chega aqui.
    RunQuery {
        /// Pares `(servidor, bases marcadas)`, na ordem do cadastro.
        targets: Vec<(String, Vec<String>)>,
        /// SQL da caixa de texto.
        sql: String,
        /// Diretório base da saída.
        output_directory: String,
    },
}

/// O que um job devolve para a janela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobResult {
    /// O cadastro recarregado.
    Servers(Vec<ServerConfigEntry>),
    /// O DDL chegou (ou falhou) para o painel da IA.
    Ddl {
        /// Texto filtrado e data, ou o motivo da falha.
        result: Result<mustel_core::usecases::ddl::Ddl, String>,
    },
    /// A IA respondeu (ou falhou) para o painel.
    SqlGenerated {
        /// SQL normalizado, ou o motivo da falha.
        result: Result<String, String>,
    },
    /// Os perfis foram gravados.
    AiProfilesSaved,
    /// O `Test` de um perfil terminou.
    AiTested {
        /// `id` do perfil testado.
        profile: String,
        /// Deu certo? (e a mensagem, de sucesso ou de erro)
        result: Result<String, String>,
    },
    /// Um servidor foi gravado.
    Saved(String),
    /// Um servidor foi removido.
    Removed(String),
    /// A senha foi guardada.
    PasswordSaved(String),
    /// O teste de conexão passou.
    Tested {
        /// Servidor testado.
        server: String,
        /// Bancos encontrados.
        databases: Vec<String>,
    },
    /// A consulta terminou; os caminhos vêm do núcleo.
    QueryDone {
        /// Consolidado, quando houve linha.
        consolidated: Option<PathBuf>,
        /// Log da execução.
        log: PathBuf,
        /// Servidores que entraram.
        servers: usize,
        /// Bancos com sucesso.
        successes: usize,
        /// Bancos com falha.
        failures: usize,
        /// Soma das linhas.
        total_rows: u64,
    },
    /// A consulta não achou banco nenhum (o comando não falhou).
    QueryWithoutDatabases,
    /// O esquema de referência foi carregado (para o autocomplete).
    Schema {
        /// Servidor de onde veio.
        server: String,
        /// Banco de onde veio.
        database: String,
        /// O esquema, ou a mensagem de erro pronta.
        schema: Result<mustel_core::usecases::schema::Schema, String>,
    },
    /// A descoberta de bancos de um servidor terminou.
    Databases {
        /// Servidor descoberto.
        server: String,
        /// Bancos do servidor (**todo** o catálogo, inclusive templates), ou o motivo de não ter
        /// dado para listar.
        ///
        /// Vai o catálogo inteiro porque cada tela usa um recorte: a aba Query conta e mostra o que
        /// o servidor tem (para a contagem não mudar quando um seletor nomeia uma base que a
        /// descoberta escondia), e os combos de execução filtram pelo que dá para conectar.
        databases: Result<Vec<mustel_core::usecases::db_servers::CatalogDatabase>, String>,
        /// O cofre tinha senha para o servidor?
        has_password: bool,
    },
    /// Uma query de seleção foi carregada e validada contra o cadastro.
    Selection {
        /// Rótulo da query.
        query: String,
        /// O escopo que a tela pediu (volta para ser aplicado só nele).
        scope: crate::selection::ApplyScope,
        /// O que a query trouxe, por servidor do cadastro.
        selection: Vec<mustel_core::usecases::custom_databases::ServerSelection>,
        /// Linhas descartadas por falta de host/banco.
        ignored_rows: usize,
    },
    /// A aplicação de uma query de seleção falhou.
    ///
    /// Variante própria (e não o `Failed` genérico) porque a tela precisa saber **o que** não
    /// aplicou: o alerta da sidebar nomeia a query e a seleção anterior fica de pé.
    SelectionFailed {
        /// Rótulo da query que não aplicou.
        query: String,
        /// Erro agregado, pronto para a tela.
        error: String,
    },
    /// O resultado do `Test` de uma query de seleção (aba Connections › Query Selector).
    SelectionTested {
        /// Nome da query ensaiada.
        name: String,
        /// Os números, ou o erro agregado pronto para a tela.
        outcome: Result<crate::query::SelectionOutcome, String>,
    },
    /// O job falhou, com a mensagem pronta do núcleo.
    Failed(String),
}
