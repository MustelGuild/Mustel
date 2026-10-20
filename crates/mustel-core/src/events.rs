//! Vocabulário de eventos do núcleo (T40).
//!
//! Todo caso de uso do `mustel-core` reporta o que acontece **por aqui**, em dados — nunca em texto
//! de interface. Quem escuta decide a forma: o CLI desenha no terminal (tabela ao vivo, mensagens),
//! a GUI (T41) desenha na janela.
//!
//! A regra que faz isso valer: o núcleo não formata mensagem de tela nem pergunta nada ao usuário.
//! Entrada vem **resolvida** (quem resolve é o frontend, com os prompts) e saída sai por
//! [`EventSink`].

use std::path::Path;
use std::time::Duration;

/// Como uma execução terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Error,
}

impl Status {
    /// Texto que o legado grava no log e no `_erros.csv`.
    pub fn text(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Error => "Error",
        }
    }
}

/// O que aconteceu numa execução.
///
/// Os eventos são emprestados (`&'a str`) porque são emitidos no meio do trabalho, sem alocar: quem
/// escuta tem de copiar o que quiser guardar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreEvent<'a> {
    /// Um banco terminou — sucesso ou erro (a linha da tabela ao vivo do legado).
    DatabaseFinished {
        /// Nome do servidor.
        server: &'a str,
        /// Banco alvo.
        database: &'a str,
        /// Como terminou.
        status: Status,
        /// Linhas trazidas (0 no erro).
        rows: u64,
        /// Quanto durou a execução daquele banco (zero no erro, como o legado).
        duration: Duration,
        /// Mensagem do erro, vazia no sucesso.
        error: &'a str,
    },
    /// O servidor não tem banco nenhum para rodar (a linha `no databases` do legado).
    ServerWithoutDatabases {
        /// Nome do servidor.
        server: &'a str,
    },
    /// A fila de descoberta chegou neste servidor (o `Discovering databases on x...` do legado).
    ///
    /// Sai para **todo** servidor da fila quando algum descobre, inclusive para os que só vão ler o
    /// cadastro: é o que o legado faz com o `ctx.Status` dentro do laço.
    DiscoveringDatabases {
        /// Nome do servidor.
        server: &'a str,
    },
    /// A descoberta automática falhou; o comando segue com o que o cadastro tiver.
    DiscoveryFailed {
        /// Nome do servidor.
        server: &'a str,
        /// Mensagem do driver.
        error: &'a str,
    },
    /// O servidor não tem banco no cadastro e a descoberta falhou: ele entra sem banco.
    FallingBackToConfiguredDatabases {
        /// Nome do servidor.
        server: &'a str,
    },
    /// Nenhum banco acessível entre os servidores escolhidos: a execução não começa.
    NoAccessibleDatabases,
    /// Os arquivos da execução, para quem quiser mostrar os caminhos.
    FilesWritten {
        /// Consolidado, quando houve alguma linha de dados.
        consolidated: Option<&'a Path>,
        /// `_erros.csv`, quando houve falha.
        errors: Option<&'a Path>,
        /// `_log.csv` (existe caminho mesmo quando o arquivo não foi criado).
        log: &'a Path,
    },
    /// Os números do resumo final.
    Summary {
        /// Servidores que entraram na execução.
        servers: usize,
        /// Bancos que responderam com sucesso.
        successes: usize,
        /// Bancos que falharam.
        failures: usize,
        /// Soma das linhas trazidas.
        total_rows: u64,
    },
}

/// Quem escuta os eventos do núcleo.
///
/// `&self` (e não `&mut self`) porque os produtores rodam concorrentes dentro de um mesmo caso de
/// uso; quem precisar de estado usa interior mutabilidade (`RefCell`, `Mutex`, canal).
///
/// O padrão dos testes e de quem não quer acompanhar é o [`NoopSink`].
pub trait EventSink {
    /// Recebe um evento.
    fn emit(&self, event: CoreEvent<'_>);
}

/// Escuta que descarta tudo — para testes e para quem só quer o resultado.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _event: CoreEvent<'_>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Escuta que guarda os eventos em forma legível — é o que os testes usam para conferir a
    /// sequência emitida sem depender de texto de interface.
    #[derive(Default)]
    struct VecSink(RefCell<Vec<String>>);

    impl EventSink for VecSink {
        fn emit(&self, event: CoreEvent<'_>) {
            self.0.borrow_mut().push(format!("{event:?}"));
        }
    }

    #[test]
    fn the_noop_sink_swallows_everything() {
        NoopSink.emit(CoreEvent::ServerWithoutDatabases { server: "dev" });
    }

    #[test]
    fn a_sink_sees_the_events_in_order() {
        let sink = VecSink::default();

        sink.emit(CoreEvent::ServerWithoutDatabases { server: "vazio" });
        sink.emit(CoreEvent::DatabaseFinished {
            server: "dev",
            database: "db",
            status: Status::Success,
            rows: 3,
            duration: Duration::from_millis(12),
            error: "",
        });

        let seen = sink.0.borrow();
        assert_eq!(seen.len(), 2, "dois eventos: {seen:?}");
        assert!(seen[0].contains("ServerWithoutDatabases"), "{:?}", seen[0]);
        assert!(seen[1].contains("Success"), "{:?}", seen[1]);
    }

    #[test]
    fn the_status_text_is_the_one_the_legacy_writes() {
        assert_eq!(Status::Success.text(), "Success");
        assert_eq!(Status::Error.text(), "Error");
    }
}
