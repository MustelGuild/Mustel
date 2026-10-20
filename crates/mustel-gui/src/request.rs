//! O que a janela pede ao núcleo, e o que ela recebe de volta (T42).
//!
//! As telas **devolvem** [`Request`]s em vez de executar: quem tem runtime, canal e token é o
//! [`App`](crate::app::App). É o que mantém o desenho puro e testável.

use std::path::PathBuf;

use mustel_core::config::ServerConfigEntry;

/// Pedido de uma tela para o `App` executar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Recarrega o cadastro do disco.
    ReloadServers,
    /// Grava um servidor (cadastra ou atualiza).
    SaveServer(ServerConfigEntry),
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
    /// O job falhou, com a mensagem pronta do núcleo.
    Failed(String),
}
