//! Cancelamento cooperativo.
//!
//! O mesmo token serve ao Ctrl+C no CLI e ao botão de cancelar na GUI (D9). O trabalho de longa
//! duração observa o token; quem mata árvore de processos é o runner de processos (T27).

pub use tokio_util::sync::CancellationToken;

/// Cria um token de cancelamento novo (não cancelado).
pub fn cancellation_token() -> CancellationToken {
    CancellationToken::new()
}

/// Escuta Ctrl+C e cancela o token; também encerra quando o token for cancelado por outro caminho
/// (a GUI, por exemplo). Devolve o handle da task para quem quiser aguardar.
pub fn spawn_ctrl_c_listener(token: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if result.is_ok() {
                    token.cancel();
                }
            }
            () = token.cancelled() => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_new_token_is_not_cancelled() {
        assert!(!cancellation_token().is_cancelled());
    }

    #[tokio::test]
    async fn listener_finishes_when_the_token_is_cancelled() {
        let token = cancellation_token();
        let listener = spawn_ctrl_c_listener(token.clone());

        token.cancel();

        tokio::time::timeout(Duration::from_secs(5), listener)
            .await
            .expect("listener must finish")
            .expect("listener must not panic");
    }

    #[tokio::test]
    async fn cancellation_propagates_to_child_tokens() {
        let parent = cancellation_token();
        let child = parent.child_token();

        assert!(!child.is_cancelled());
        parent.cancel();
        assert!(child.is_cancelled());
    }
}
