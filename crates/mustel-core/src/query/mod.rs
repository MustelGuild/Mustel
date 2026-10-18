//! `query run`: montar a execução, rodar a consulta e exportar o CSV.
//!
//! O núcleo cuida do que é regra — formatar valor ([`crate::postgres::value`]), escrever CSV
//! ([`csv_writer`]), decidir quem entra na execução ([`discovery`]) —; o CLI cuida de prompt,
//! progresso e mensagens.

pub mod csv_writer;
pub mod discovery;
