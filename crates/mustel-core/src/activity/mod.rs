//! Log de atividade: o que o Mustel fez e o que falhou.
//!
//! Uma linha JSONL por evento, em arquivos diários (`activity-YYYY-MM-DD.jsonl`), retidos por 30
//! dias. O log é **local**: só o relatório anonimizado sai da máquina, e mesmo assim depois de uma
//! prévia explícita.
//!
//! - [`entry`] — o modelo do evento;
//! - [`paths`] — onde o log e os relatórios moram;
//! - [`store`] — append, leitura da janela e poda;
//! - [`redact`] — o mapa de anonimização usado pelo relatório;
//! - [`report`] — o texto do relatório (o `.log`);
//! - [`crash`] — o hook de panic.

pub mod crash;
pub mod entry;
pub mod paths;
pub mod redact;
pub mod report;
pub mod store;

/// Quantos dias de log são mantidos (a janela da aba e a poda).
pub const RETENTION_DAYS: u32 = 30;

pub use entry::{Area, Entry, Status, op_label};
pub use paths::{LOG_DIR_ENV, home_dir, log_dir, log_dir_with, reports_dir, reports_dir_with};
pub use redact::{Context as RedactContext, Redactor, Replacement};
pub use report::{AppInfo, Report, render};
pub use store::Store;
