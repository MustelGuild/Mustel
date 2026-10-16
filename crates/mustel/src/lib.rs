//! Frontend de terminal do Mustel.
//!
//! Este crate é a biblioteca do binário `mustel` (mais o `main.rs`, que é só a costura de entrada).
//! Ter biblioteca permite que os testes exercitem a árvore de comandos e as primitivas de UI
//! diretamente, em vez de só via processo — e evita que código ainda não usado pelos comandos caia em
//! `dead_code`.

pub mod cli;
pub mod commands;
pub mod exit;
pub mod ui;
