//! Núcleo do Mustel.
//!
//! Aqui vivem as regras de negócio e os casos de uso (configuração, credenciais, Postgres,
//! `query run`, `database`, `settings`). **Este crate não conhece UI**: nada de `clap`,
//! `inquire`, `indicatif` ou `egui` (D9).
//!
//! Os frontends (`mustel` CLI e `mustel gui`) consomem estes casos de uso passando entradas já
//! resolvidas e recebendo eventos por um `EventSink`.

pub mod ai_transport;
pub mod completion;
pub mod config;
pub mod credentials;
pub mod error;
pub mod events;
pub mod pgpass;
pub mod postgres;
pub mod process;
pub mod query;
pub mod signal;
pub mod usecases;
pub mod validation;
