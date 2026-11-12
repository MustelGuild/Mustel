//! O estado local do updater: último check, backoff e versão dispensada.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Espera mínima entre tentativas depois de uma falha (1 h).
pub const BACKOFF_MS: u64 = 60 * 60 * 1000;
/// Quanto tempo a versão dispensada fica silenciosa (7 dias).
pub const DISMISS_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// O conteúdo de `<state>/Mustel/update.json`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateState {
    /// Dia **UTC** do último check bem-sucedido (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_day: Option<String>,
    /// Quando foi a última tentativa (sucesso ou falha), para o backoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attempt_ms: Option<u64>,
    /// Versão que o usuário fechou no toast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_version: Option<String>,
    /// Quando ela foi dispensada, para os 7 dias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_at_ms: Option<u64>,
}

impl UpdateState {
    /// Lê o arquivo; ausente/corrompido vira o default (nunca impede o app de abrir).
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Grava o estado (melhor esforço do chamador).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(path, text)
    }

    /// Já checou hoje? Está dentro do backoff de uma falha?
    pub fn should_check(&self, today: &str, now_ms: u64) -> bool {
        if self.last_check_day.as_deref() == Some(today) {
            return false;
        }
        if let Some(last) = self.last_attempt_ms
            && now_ms.saturating_sub(last) < BACKOFF_MS
        {
            return false;
        }
        true
    }

    /// Registra um check bem-sucedido (com ou sem versão nova).
    pub fn record_success(&mut self, today: &str, now_ms: u64) {
        self.last_check_day = Some(today.to_string());
        self.last_attempt_ms = Some(now_ms);
    }

    /// Registra uma tentativa que falhou (para o backoff).
    pub fn record_failure(&mut self, now_ms: u64) {
        self.last_attempt_ms = Some(now_ms);
    }

    /// A mesma versão foi dispensada há menos de 7 dias?
    pub fn dismissed_recently(&self, version: &str, now_ms: u64) -> bool {
        self.dismissed_version.as_deref() == Some(version)
            && self
                .dismissed_at_ms
                .is_some_and(|at| now_ms.saturating_sub(at) < DISMISS_MS)
    }

    /// Marca a versão como dispensada agora.
    pub fn dismiss(&mut self, version: &str, now_ms: u64) {
        self.dismissed_version = Some(version.to_string());
        self.dismissed_at_ms = Some(now_ms);
    }
}

/// `<state>/Mustel/update.json`, quando o SO expõe a pasta.
pub fn default_path() -> Option<PathBuf> {
    crate::activity::paths::state_root().map(|root| root.join("update.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000_000;
    const DAY: u64 = 24 * 60 * 60 * 1000;

    /// CONTRACT: IF-268
    #[test]
    fn a_check_runs_once_per_day() {
        let mut state = UpdateState::default();
        assert!(state.should_check("2026-11-07", NOW));

        state.record_success("2026-11-07", NOW);
        assert!(!state.should_check("2026-11-07", NOW + 1));
        assert!(state.should_check("2026-11-08", NOW + DAY));
    }

    /// CONTRACT: IF-268
    #[test]
    fn a_failure_backs_off_for_one_hour() {
        let mut state = UpdateState::default();
        state.record_failure(NOW);

        assert!(!state.should_check("2026-11-08", NOW + 1_000));
        assert!(state.should_check("2026-11-08", NOW + BACKOFF_MS));
    }

    /// CONTRACT: IF-268
    #[test]
    fn a_dismissed_version_stays_quiet_for_seven_days() {
        let mut state = UpdateState::default();
        state.dismiss("1.1.0", NOW);

        assert!(state.dismissed_recently("1.1.0", NOW + DAY));
        assert!(!state.dismissed_recently("1.1.0", NOW + DISMISS_MS));
        assert!(!state.dismissed_recently("1.2.0", NOW + DAY));
    }

    #[test]
    fn the_state_round_trips_through_the_file() {
        let dir = tempfile::tempdir().expect("temporário");
        let path = dir.path().join("update.json");
        let mut state = UpdateState::default();
        state.record_success("2026-11-07", NOW);
        state.dismiss("1.1.0", NOW);

        state.save(&path).expect("grava");
        let back = UpdateState::load(&path);

        assert_eq!(back, state);
    }

    #[test]
    fn a_corrupt_file_reads_as_default() {
        let dir = tempfile::tempdir().expect("temporário");
        let path = dir.path().join("update.json");
        std::fs::write(&path, "{nao e json").expect("escreve");

        assert_eq!(UpdateState::load(&path), UpdateState::default());
    }

    #[test]
    fn the_default_path_is_next_to_the_activity_log() {
        if let Some(path) = default_path() {
            assert_eq!(
                path.file_name().and_then(|name| name.to_str()),
                Some("update.json")
            );
        }
    }
}
