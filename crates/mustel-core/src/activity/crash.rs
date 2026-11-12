//! Hook de panic: um crash deixa uma linha no mesmo log.
//!
//! O hook é global e cobre qualquer thread (a da UI e as do runtime). Ele grava a mensagem, o
//! local e um **backtrace**, e então chama o hook anterior — no terminal a mensagem padrão continua
//! aparecendo; no `mustel-gui` do Windows, onde não há console, o arquivo é a única testemunha.
//!
//! Uma flag atômica impede que um panic dentro do próprio hook entre em recursão.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use super::entry::{Area, Entry, Status};
use super::store::Store;

/// Garante uma única instalação por processo.
static INSTALLED: Once = Once::new();
/// Evita recursão se a gravação (ou o hook anterior) entrar em pânico.
static IN_HOOK: AtomicBool = AtomicBool::new(false);

/// Instala o hook global, gravando no `store`.
///
/// Chamadas seguintes são ignoradas: o primeiro `store` vence.
pub fn install(store: Store) {
    INSTALLED.call_once(move || {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !IN_HOOK.swap(true, Ordering::SeqCst) {
                let entry = Entry::new(Area::App, "app.panic", Status::Error)
                    .with_error(panic_message(info));
                let _ = store.append(&entry);
                IN_HOOK.store(false, Ordering::SeqCst);
            }
            previous(info);
        }));
    });
}

/// A mensagem do panic com localização e backtrace.
fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic".to_string()
    };

    let location = info
        .location()
        .map(|location| format!(" at {}:{}", location.file(), location.line()))
        .unwrap_or_default();

    format!(
        "{message}{location}\n\nBacktrace:\n{}",
        std::backtrace::Backtrace::force_capture()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::now_ms;

    /// O hook grava a mensagem do panic no arquivo do log.
    ///
    /// Um único teste instala o hook (é global); os demais só leem o arquivo.
    /// CONTRACT: IF-265
    #[test]
    fn a_panic_is_written_to_the_log() {
        let dir = tempfile::tempdir().expect("temporário");
        let store = Store::new(dir.path());
        install(store.clone());

        let _ = std::panic::catch_unwind(|| panic!("explosão de teste"));

        let entries = store.load(1, now_ms()).expect("lê o log");
        let crash = entries
            .iter()
            .find(|entry| entry.op == "app.panic")
            .expect("o panic virou entrada");
        let message = crash.error.as_deref().unwrap_or("");
        assert!(message.contains("explosão de teste"), "{message}");
        assert!(message.contains("Backtrace:"), "o backtrace acompanha");
    }
}
