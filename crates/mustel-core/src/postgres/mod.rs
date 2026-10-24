//! Camada PostgreSQL (driver `tokio-postgres`).
//!
//! Decisão D12: **`tokio-postgres`**, escolhido pelo usuário. A alternativa (`sqlx`) foi descartada;
//! o registro completo, com o que está pronto e o que falta, está em `docs/rust-postgres-spike.md`.
//!
//! Organização: [`settings`] monta a configuração de conexão a partir do cadastro e das flags;
//! [`tls`] traduz o `sslMode` do legado; [`value`] converte valores do servidor para o texto que vai
//! ao CSV (a parte sensível a paridade); [`client`] é a fronteira fina com o driver; [`locator`]
//! acha os binários do PostgreSQL e [`password`] resolve de onde vem a senha.

pub mod client;
pub mod connection_string;
pub mod locator;
pub mod password;
pub mod settings;
pub mod tls;
pub mod value;
