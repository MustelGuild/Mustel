//! Atualização do produto pelo GitHub Releases.
//!
//! O ciclo é: consultar a release estável mais recente ([`check`]), escolher os executáveis da
//! plataforma ([`asset_name_for`]), baixar e verificar ([`checksums`]/[`download`]) e aplicar com o
//! helper ([`apply`]). Só a GUI consome o módulo; o estado local fica em [`state`].
//!
//! Nada aqui abre rede por conta própria: o HTTP entra por [`UpdateTransport`], então a suíte usa
//! um falso e o binário usa [`ReqwestTransport`].

pub mod apply;
pub mod check;
pub mod download;
pub mod model;
pub mod state;
pub mod transport;

pub use apply::{
    ApplyRequest, ApplyTarget, apply_from_args, helper_command, parse_args, replace_all, run,
};
pub use check::{check, check_current};
pub use download::{checksums, download};
pub use model::{
    Asset, Release, TargetPlatform, api_latest_url, asset_name_for, current_version, parse_tag,
    repository_slug,
};
pub use state::UpdateState;
pub use transport::{ReqwestTransport, UpdateTransport};
