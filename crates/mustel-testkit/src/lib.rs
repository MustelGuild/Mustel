//! Harness de paridade do Mustel.
//!
//! Abriga fixtures e os dois binários do oráculo:
//! - `oracle-capture` — executa o FurLab .NET (`legacy/`) e grava `oracle/baseline/`;
//! - `oracle-compare` — executa o `mustel` e compara exit code, stdout/stderr normalizados e
//!   artefatos byte a byte, classificando divergências N1–N3 (falha) e N4 (aviso).
//!
//! Ver `oracle/README.md` e a seção 4.4 do plano (`MustelPlans/migracao-rust.md`).
