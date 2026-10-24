//! Fixtures e comparação do contrato de interface do Mustel.
//!
//! O baseline (`fixtures/baseline/`) é **contrato congelado**: foi capturado do FurLab .NET antes da
//! remoção do `legacy/`, e não há mais como re-gravá-lo. O binário `oracle-compare` reexecuta o
//! `mustel` contra as capturas e classifica as diferenças (N1–N3 falham, N4 avisam); os testes de
//! contrato de `crates/mustel` usam as mesmas funções (`capture`/`compare`) dentro de `cargo test`.
//!
//! Ver `fixtures/README.md` e `docs/contrato-de-interface.md`.

pub mod capture;
pub mod compare;
pub mod config_sandbox;
pub mod paths;
pub mod pgpass_sandbox;
pub mod report;
