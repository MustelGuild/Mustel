//! As telas da GUI (T42).
//!
//! Cada tela é **estado + desenho**: ela não fala com o núcleo nem com o tokio. O que ela faz é
//! desenhar e devolver [`Request`](crate::request::Request)s — quem executa, cancelar e devolver o
//! resultado é o [`App`](crate::app::App). Assim a tela é testável sem janela, e a regra continua toda
//! no núcleo.

pub mod query;
pub mod servers;
pub mod settings;
