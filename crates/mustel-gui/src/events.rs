//! Eventos do núcleo, já em forma própria, e o canal que os leva à janela (T41).
//!
//! [`CoreEvent`] empresta os textos (`&'a str`) porque é emitido no meio do trabalho, sem alocar. A
//! janela não pode guardar empréstimos — ela desenha depois, em outro momento —, então o
//! [`ChannelSink`] **copia** para [`GuiEvent`] e avisa o `egui` de que há desenho novo.
//!
//! O `match` de [`GuiEvent::from_core`] é **exaustivo de propósito**: um evento novo no núcleo quebra
//! a compilação aqui, e não passa despercebido pela GUI.

use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use mustel_core::events::{CoreEvent, DatabaseStep, EventSink, Status};

/// Um evento do núcleo com os textos copiados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiEvent {
    /// Um banco terminou (linha da tabela de progresso).
    DatabaseFinished {
        /// Servidor.
        server: String,
        /// Banco.
        database: String,
        /// Como terminou.
        status: Status,
        /// Linhas trazidas.
        rows: u64,
        /// Duração.
        duration: Duration,
        /// Erro, vazio no sucesso.
        error: String,
    },
    /// O servidor não tinha banco nenhum.
    ServerWithoutDatabases {
        /// Servidor.
        server: String,
    },
    /// A fase de descoberta chegou neste servidor.
    DiscoveringDatabases {
        /// Servidor.
        server: String,
    },
    /// A descoberta automática falhou.
    DiscoveryFailed {
        /// Servidor.
        server: String,
        /// Mensagem do driver.
        error: String,
    },
    /// O servidor entrou sem banco porque a descoberta falhou.
    FallingBackToConfiguredDatabases {
        /// Servidor.
        server: String,
    },
    /// Nenhum banco acessível entre os servidores escolhidos.
    NoAccessibleDatabases,
    /// Os arquivos da execução.
    FilesWritten {
        /// Consolidado, quando houve linha.
        consolidated: Option<String>,
        /// Arquivo de erros, quando houve falha.
        errors: Option<String>,
        /// Log.
        log: String,
    },
    /// Os números do resumo final.
    Summary {
        /// Servidores que entraram na execução.
        servers: usize,
        /// Bancos com sucesso.
        successes: usize,
        /// Bancos com falha.
        failures: usize,
        /// Soma das linhas.
        total_rows: u64,
    },
    /// O `--all` do `backup` não achou banco.
    NoDatabasesFound,
    /// O `--all` do `restore` não achou arquivo.
    NoDumpFilesFound,
    /// O `--all` do `restore` pulou um nome inválido.
    SkippingDatabase {
        /// Nome tirado do arquivo.
        database: String,
    },
    /// Um banco do `--all` falhou; o comando segue.
    DatabaseStepFailed {
        /// Qual comando.
        step: DatabaseStep,
        /// Banco.
        database: String,
        /// Mensagem sem o prefixo `Error: `.
        error: String,
    },
}

impl GuiEvent {
    /// Copia um evento do núcleo para a forma própria da janela.
    pub fn from_core(event: CoreEvent<'_>) -> Self {
        match event {
            CoreEvent::DatabaseFinished {
                server,
                database,
                status,
                rows,
                duration,
                error,
            } => Self::DatabaseFinished {
                server: server.to_string(),
                database: database.to_string(),
                status,
                rows,
                duration,
                error: error.to_string(),
            },
            CoreEvent::ServerWithoutDatabases { server } => Self::ServerWithoutDatabases {
                server: server.to_string(),
            },
            CoreEvent::DiscoveringDatabases { server } => Self::DiscoveringDatabases {
                server: server.to_string(),
            },
            CoreEvent::DiscoveryFailed { server, error } => Self::DiscoveryFailed {
                server: server.to_string(),
                error: error.to_string(),
            },
            CoreEvent::FallingBackToConfiguredDatabases { server } => {
                Self::FallingBackToConfiguredDatabases {
                    server: server.to_string(),
                }
            }
            CoreEvent::NoAccessibleDatabases => Self::NoAccessibleDatabases,
            CoreEvent::FilesWritten {
                consolidated,
                errors,
                log,
            } => Self::FilesWritten {
                consolidated: consolidated.map(|path| path.display().to_string()),
                errors: errors.map(|path| path.display().to_string()),
                log: log.display().to_string(),
            },
            CoreEvent::Summary {
                servers,
                successes,
                failures,
                total_rows,
            } => Self::Summary {
                servers,
                successes,
                failures,
                total_rows,
            },
            CoreEvent::NoDatabasesFound => Self::NoDatabasesFound,
            CoreEvent::NoDumpFilesFound => Self::NoDumpFilesFound,
            CoreEvent::SkippingDatabase { database } => Self::SkippingDatabase {
                database: database.to_string(),
            },
            CoreEvent::DatabaseStepFailed {
                step,
                database,
                error,
            } => Self::DatabaseStepFailed {
                step,
                database: database.to_string(),
                error: error.to_string(),
            },
        }
    }
}

/// Leva os eventos do núcleo para a janela.
///
/// `emit` roda **nas threads do runtime tokio** (as consultas e os `pg_dump` em paralelo), então o
/// canal é o único ponto de contato com a UI: manda o evento e pede um repaint. Nada aqui bloqueia.
pub struct ChannelSink {
    sender: Sender<GuiEvent>,
    /// Avisa o `egui` de que há evento novo para desenhar.
    repaint: Box<dyn Fn() + Send + Sync>,
}

impl ChannelSink {
    /// Escuta os eventos e os entrega ao canal dado.
    pub fn new(sender: Sender<GuiEvent>, repaint: Box<dyn Fn() + Send + Sync>) -> Self {
        Self { sender, repaint }
    }

    /// Cria o par sink/receptor com o aviso de repaint que a janela precisa.
    pub fn with_repaint(repaint: Box<dyn Fn() + Send + Sync>) -> (Self, Receiver<GuiEvent>) {
        let (sender, receiver) = std::sync::mpsc::channel();

        (Self::new(sender, repaint), receiver)
    }
}

impl EventSink for ChannelSink {
    fn emit(&self, event: CoreEvent<'_>) {
        // Canal fechado quer dizer que a janela não existe mais: o trabalho segue e o evento cai.
        if self.sender.send(GuiEvent::from_core(event)).is_ok() {
            (self.repaint)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn the_sink_delivers_an_owned_copy_and_asks_for_a_repaint() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&repaints);
        let sink = ChannelSink::new(
            sender,
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );

        {
            // O evento empresta o texto: o sink precisa copiar antes de devolver.
            let database = String::from("mydb");
            sink.emit(CoreEvent::DatabaseFinished {
                server: "dev",
                database: &database,
                status: Status::Success,
                rows: 3,
                duration: Duration::from_millis(12),
                error: "",
            });
            // `database` morre aqui; o que foi para o canal tem de sobreviver.
        }

        assert_eq!(repaints.load(Ordering::SeqCst), 1, "um repaint por evento");
        assert_eq!(
            receiver.recv().expect("entrega"),
            GuiEvent::DatabaseFinished {
                server: "dev".to_string(),
                database: "mydb".to_string(),
                status: Status::Success,
                rows: 3,
                duration: Duration::from_millis(12),
                error: String::new(),
            }
        );
    }

    #[test]
    fn every_core_event_has_an_owned_twin() {
        // Uma amostra de cada variante: o `match` de `from_core` é exaustivo, então uma variante
        // nova no núcleo não compila sem passar por aqui.
        let events = vec![
            GuiEvent::from_core(CoreEvent::ServerWithoutDatabases { server: "vazio" }),
            GuiEvent::from_core(CoreEvent::DiscoveringDatabases { server: "dev" }),
            GuiEvent::from_core(CoreEvent::DiscoveryFailed {
                server: "dev",
                error: "sem rota",
            }),
            GuiEvent::from_core(CoreEvent::FallingBackToConfiguredDatabases { server: "dev" }),
            GuiEvent::from_core(CoreEvent::NoAccessibleDatabases),
            GuiEvent::from_core(CoreEvent::NoDatabasesFound),
            GuiEvent::from_core(CoreEvent::NoDumpFilesFound),
            GuiEvent::from_core(CoreEvent::SkippingDatabase { database: "x y" }),
            GuiEvent::from_core(CoreEvent::DatabaseStepFailed {
                step: DatabaseStep::Backup,
                database: "mydb",
                error: "falhou",
            }),
            GuiEvent::from_core(CoreEvent::Summary {
                servers: 1,
                successes: 1,
                failures: 0,
                total_rows: 2,
            }),
        ];

        assert_eq!(events.len(), 10);
        assert_eq!(
            events[0],
            GuiEvent::ServerWithoutDatabases {
                server: "vazio".to_string()
            }
        );
        assert_eq!(
            events[7],
            GuiEvent::SkippingDatabase {
                database: "x y".to_string()
            }
        );
    }

    #[test]
    fn a_closed_window_drops_the_event_without_panicking() {
        let (sender, receiver) = std::sync::mpsc::channel();
        drop(receiver);
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&repaints);
        let sink = ChannelSink::new(
            sender,
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );

        sink.emit(CoreEvent::NoAccessibleDatabases);

        assert_eq!(
            repaints.load(Ordering::SeqCst),
            0,
            "sem janela não há repaint"
        );
    }
}
