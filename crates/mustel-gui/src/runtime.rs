//! Runtime tokio de background (T41).
//!
//! A janela roda na thread principal e desenha a 60 Hz; o trabalho do núcleo (consultas em
//! paralelo, `pg_dump`, conexões) roda atrás, num runtime próprio. A ponte é o
//! [`ChannelSink`](crate::events::ChannelSink): o job manda eventos e pede repaint, a janela lê os
//! eventos no próximo quadro.
//!
//! O runtime é criado **uma vez** e derrubado ao fechar a janela: criar um por job custaria threads
//! a cada clique.

use std::future::Future;

use mustel_core::signal::{CancellationToken, cancellation_token};
use tokio::task::JoinHandle;

/// Runtime de background onde os jobs do núcleo rodam.
pub struct Runtime {
    runtime: Option<tokio::runtime::Runtime>,
    handle: tokio::runtime::Handle,
}

impl Runtime {
    /// Cria o runtime multi-thread.
    pub fn new() -> Result<Self, std::io::Error> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("mustel-gui-worker")
            .build()?;
        let handle = runtime.handle().clone();

        Ok(Self {
            runtime: Some(runtime),
            handle,
        })
    }

    /// Dispara um job e devolve o handle dele e o token para cancelar.
    ///
    /// O token nasce aqui e é **entregue ao job** (que só o observa) e à interface (que o cancela).
    pub fn spawn<F, Fut>(&self, job: F) -> (CancellationToken, JoinHandle<()>)
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let token = cancellation_token();
        let future = job(token.clone());

        (token, self.handle.spawn(future))
    }

    /// Roda um futuro até o fim, **bloqueando a thread atual** — para jobs curtos em testes.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }
}

impl Drop for Runtime {
    /// Derruba o runtime ao fechar a janela, esperando os jobs em andamento.
    ///
    /// Quando a janela é aberta pelo subcomando `gui`, este `Runtime` é derrubado **de dentro**
    /// do runtime do CLI (o `#[tokio::main]`): o shutdown normal bloqueia e o tokio entra em
    /// pânico ("Cannot drop a runtime in a context where blocking is not allowed"). Nesse caso o
    /// shutdown é o de fundo — as threads terminam sozinhas e o processo já está saindo.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                runtime.shutdown_background();
            } else {
                runtime.shutdown_timeout(std::time::Duration::from_millis(500));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_runs_on_the_background_runtime_and_can_be_cancelled() {
        let runtime = Runtime::new().expect("cria o runtime");
        let (token, handle) = runtime.spawn(|token| async move {
            // Espera o cancelamento chegar, como um job real observaria o token.
            while !token.is_cancelled() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        });

        token.cancel();
        runtime.block_on(handle).expect("o job termina sem pânico");
    }

    #[test]
    fn the_runtime_runs_a_future_to_completion() {
        let runtime = Runtime::new().expect("cria o runtime");

        assert_eq!(runtime.block_on(async { 2 + 2 }), 4);
    }

    /// Fechar a janela do subcomando `gui` derruba este runtime **de dentro** do runtime do CLI.
    ///
    /// O shutdown normal bloqueia e o tokio entra em pânico nesse contexto; o `Drop` precisa
    /// reconhecer o caso e usar o shutdown de fundo. É o caminho de todo fechamento da janela —
    /// inclusive o do reinício depois do update.
    /// CONTRACT: IF-270
    #[test]
    fn dropping_the_runtime_inside_another_runtime_does_not_panic() {
        let outer = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime externo");

        outer.block_on(async {
            let inner = Runtime::new().expect("runtime da janela");
            drop(inner);
        });
    }
}
