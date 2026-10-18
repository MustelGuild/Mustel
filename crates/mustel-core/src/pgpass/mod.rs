//! Credenciais do PostgreSQL no arquivo `pgpass.conf` (`database pgpass`).
//!
//! Porte de `legacy/FurLab.Core/Services/PgPassService.cs` e do grupo `database pgpass` do CLI
//! legado. O arquivo é o do `libpq`/`psql`: uma entrada por linha, campos separados por `:`,
//! `host:porta:banco:usuário:senha`, com `*` valendo como curinga em qualquer campo menos a senha.
//!
//! O `pgpass.conf` é um mecanismo **separado** do cofre de senhas (D6): a senha do arquivo fica em
//! texto claro, que é o contrato do formato — nada aqui toca o keyring.

pub mod entry;
pub mod paths;
pub mod store;

pub use entry::{PgPassEntry, parse_line};
pub use paths::{PGPASS_FILE_ENV, PGPASS_FILE_NAME, pgpass_file_path};
pub use store::{PgPassError, PgPassOutcome, PgPassStore};
