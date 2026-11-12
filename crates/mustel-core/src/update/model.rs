//! O modelo da release e a escolha do asset do executável.

use semver::Version;

use crate::error::CoreError;

/// A plataforma cujo asset o updater consome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPlatform {
    /// Windows: os `.exe` soltos da release.
    Windows,
    /// Linux: o `mustel` cru.
    Linux,
    /// macOS: fora dos alvos (não há asset).
    MacOs,
}

impl TargetPlatform {
    /// A plataforma deste binário.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Uma release estável do GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Versão semântica (sem o `v` da tag).
    pub version: Version,
    /// Os assets anexados.
    pub assets: Vec<Asset>,
}

impl Release {
    /// O asset com este nome, se existir.
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

/// Um asset da release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// Nome do arquivo no anexo.
    pub name: String,
    /// URL de download.
    pub url: String,
    /// Tamanho em bytes (0 quando a API não informa).
    pub size: u64,
    /// Digest do GitHub (`sha256:<hex>`), quando presente.
    pub digest: Option<String>,
}

/// `v1.2.3` ou `1.2.3` → [`Version`].
pub fn parse_tag(tag: &str) -> Option<Version> {
    let tag = tag.trim();
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

/// O nome do asset que corresponde a um executável.
///
/// No Windows o updater atualiza os dois `.exe` soltos (`mustel.exe` e `mustel-gui.exe`), desde que
/// existam lado a lado; no Linux o asset é o `mustel` cru.
pub fn asset_name_for(platform: TargetPlatform, exe_name: &str) -> Option<&'static str> {
    match platform {
        TargetPlatform::Windows => match exe_name.to_ascii_lowercase().as_str() {
            "mustel.exe" => Some("mustel.exe"),
            "mustel-gui.exe" => Some("mustel-gui.exe"),
            _ => None,
        },
        TargetPlatform::Linux => (exe_name == "mustel").then_some("mustel"),
        TargetPlatform::MacOs => None,
    }
}

/// `https://github.com/<owner>/<repo>` → `<owner>/<repo>`.
pub fn repository_slug(repository: &str) -> Option<String> {
    let rest = repository
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repository = parts.next()?;
    if owner.is_empty() || repository.is_empty() || parts.next().is_some() {
        return None;
    }
    let repository = repository.strip_suffix(".git").unwrap_or(repository);
    Some(format!("{owner}/{repository}"))
}

/// A URL da release mais recente na API do GitHub.
pub fn api_latest_url(slug: &str) -> String {
    format!("https://api.github.com/repos/{slug}/releases/latest")
}

/// A versão deste binário.
pub fn current_version() -> Result<Version, CoreError> {
    Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| CoreError::Other(format!("invalid build version: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parse_with_and_without_the_v_prefix() {
        assert_eq!(parse_tag("v1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(parse_tag("1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(
            parse_tag("v1.2.3-beta.1"),
            Some(Version::parse("1.2.3-beta.1").unwrap())
        );
        assert_eq!(parse_tag("release-1.2.3"), None);
    }

    #[test]
    fn the_windows_assets_are_the_two_sibling_exes() {
        assert_eq!(
            asset_name_for(TargetPlatform::Windows, "mustel.exe"),
            Some("mustel.exe")
        );
        assert_eq!(
            asset_name_for(TargetPlatform::Windows, "MUSTEL-GUI.EXE"),
            Some("mustel-gui.exe")
        );
        assert_eq!(asset_name_for(TargetPlatform::Windows, "outro.exe"), None);
    }

    #[test]
    fn the_linux_asset_is_the_raw_mustel() {
        assert_eq!(
            asset_name_for(TargetPlatform::Linux, "mustel"),
            Some("mustel")
        );
        assert_eq!(asset_name_for(TargetPlatform::Linux, "mustel-gui"), None);
    }

    #[test]
    fn macos_has_no_asset() {
        assert_eq!(asset_name_for(TargetPlatform::MacOs, "mustel"), None);
    }

    #[test]
    fn the_repository_slug_comes_from_the_cargo_metadata() {
        assert_eq!(
            repository_slug("https://github.com/MustelGuild/Mustel"),
            Some("MustelGuild/Mustel".to_string())
        );
        assert_eq!(
            repository_slug("https://github.com/MustelGuild/Mustel.git/"),
            Some("MustelGuild/Mustel".to_string())
        );
        assert_eq!(repository_slug("https://gitlab.com/a/b"), None);
        assert_eq!(repository_slug("https://github.com/a"), None);
    }

    #[test]
    fn the_api_url_is_the_latest_release() {
        assert_eq!(
            api_latest_url("MustelGuild/Mustel"),
            "https://api.github.com/repos/MustelGuild/Mustel/releases/latest"
        );
    }
}
