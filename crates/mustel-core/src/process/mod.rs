//! Execução de processos externos (`pg_dump`, `pg_restore`, `psql`).
//!
//! Porte de `legacy/FurLab.Core/Services/ProcessExecutor.cs`. Três diferenças de estrutura, todas
//! internas (o comportamento observável é o mesmo):
//!
//! 1. em vez de devolver `-1` com o texto do erro em `stderr` como o legado fazia, o runner separa
//!    **falha de partida** de **execução concluída** em [`ProcessError`];
//! 2. `stdout` e `stderr` são lidos **em paralelo** (o legado lia um depois do outro, o que trava
//!    quando o processo enche o buffer do outro cano);
//! 3. no cancelamento a **árvore** de processos é morta e o resultado é [`ProcessError::Cancelled`]
//!    — o legado não ligava o token ao processo (o Ctrl+C matava o grupo inteiro pelo console).
//!
//! Nada aqui passa por shell: o programa e cada argumento vão como itens separados de `Vec<String>`,
//! como o `ArgumentList` do legado (que também usava `UseShellExecute = false`).

pub mod runner;

pub use runner::{ProcessError, ProcessOptions, ProcessOutcome, run};
