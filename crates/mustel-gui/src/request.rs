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
    /// Descobre os bancos do servidor e se o cofre tem a senha dele.
    ///
    /// É o que alimenta o combo de banco da linha "Referência do autocomplete": o cadastro pode não
    /// listar todos os bancos (ou nenhum, quando o servidor usa `fetchAllDatabases`).
    DiscoverDatabases {
        /// Servidor do cadastro.
        server: String,
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
    /// Executa a consulta nos servidores escolhidos.
    RunQuery {
        /// Servidores escolhidos, na ordem da tela.
        servers: Vec<ServerConfigEntry>,
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
        /// Bancos do servidor, ou o motivo de não ter dado para listar.
        databases: Result<Vec<String>, String>,
        /// O cofre tinha senha para o servidor?
        has_password: bool,
    },
    /// O job falhou, com a mensagem pronta do núcleo.
    Failed(String),
}
