//! `query run`: montar a execução, rodar a consulta e exportar o CSV.
//!
//! O núcleo cuida do que é regra — formatar valor ([`crate::postgres::value`]), escrever CSV
//! ([`csv_writer`]), decidir quem entra na execução ([`discovery`]), classificar a consulta
//! ([`analyzer`]) e rodar os bancos em paralelo com um escritor único ([`executor`]) —; o CLI cuida de
//! prompt, progresso e mensagens.

pub mod analyzer;
pub mod csv_writer;
pub mod discovery;
pub mod executor;
pub mod plan;
