//! Casos de uso: a **orquestração** de cada comando, no núcleo (T40).
//!
//! Um caso de uso recebe a entrada **resolvida** — o frontend já perguntou, já leu o cadastro e já
//! buscou a senha — e reporta o que acontece por [`EventSink`](crate::events::EventSink). Não
//! imprime, não pergunta e não conhece terminal nem janela: é o que permite o CLI e a GUI (T41)
//! rodarem o mesmo comando.

pub mod database;
pub mod database_backup;
pub mod database_restore;
pub mod db_servers;
pub mod query_run;
pub mod schema;
