//! Consulta da release mais recente e comparação de versão.

use semver::Version;
use serde::Deserialize;

use crate::error::CoreError;

use super::model::{Asset, Release, parse_tag};
use super::transport::UpdateTransport;

/// O subconjunto da resposta `GET /releases/latest` que interessa.
#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

/// Um asset da resposta da API.
#[derive(Debug, Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// Busca a release estável mais recente em `url`.
///
/// Devolve `None` quando a versão é igual ou mais antiga que `current` (ou quando não há release);
/// erros de rede/HTTP/JSON viram [`CoreError`].
pub async fn check<T: UpdateTransport>(
    transport: &T,
    current: &Version,
    url: &str,
) -> Result<Option<Release>, CoreError> {
    let response = transport.get(url).await?;

    if response.status == 404 {
        // Repositório sem releases: não é erro.
        return Ok(None);
    }
    if !(200..300).contains(&response.status) {
        return Err(CoreError::Other(format!(
            "GitHub returned HTTP {} for the latest release",
            response.status
        )));
    }

    let body = String::from_utf8(response.body)
        .map_err(|error| CoreError::Other(format!("The release reply is not UTF-8: {error}")))?;

    parse_release(&body, current)
}

/// Lê o JSON da API e decide se ele é mais novo que `current`.
pub fn parse_release(json: &str, current: &Version) -> Result<Option<Release>, CoreError> {
    let api: ApiRelease = serde_json::from_str(json)
        .map_err(|error| CoreError::Other(format!("Could not read the release: {error}")))?;

    let Some(version) = parse_tag(&api.tag_name) else {
        return Err(CoreError::Other(format!(
            "The release tag '{}' is not a version",
            api.tag_name
        )));
    };

    if version <= *current {
        return Ok(None);
    }

    let assets = api
        .assets
        .into_iter()
        .map(|asset| Asset {
            name: asset.name,
            url: asset.browser_download_url,
            size: asset.size,
            digest: asset.digest,
        })
        .collect();

    Ok(Some(Release { version, assets }))
}

/// Como [`check`], usando a versão embutida neste binário.
pub async fn check_current<T: UpdateTransport>(
    transport: &T,
    url: &str,
) -> Result<Option<Release>, CoreError> {
    let current = super::model::current_version()?;
    check(transport, &current, url).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::transport::HttpResponse;

    struct Fake {
        status: u16,
        body: String,
    }

    impl UpdateTransport for Fake {
        async fn get(&self, _url: &str) -> Result<HttpResponse, CoreError> {
            Ok(HttpResponse {
                status: self.status,
                body: self.body.clone().into_bytes(),
            })
        }

        async fn download<'a>(
            &'a self,
            _url: &'a str,
            _into: &'a std::path::Path,
            _progress: &'a mut (dyn FnMut(u64, Option<u64>) -> bool + Send),
        ) -> Result<(), CoreError> {
            Ok(())
        }
    }

    fn release_json(tag: &str) -> String {
        format!(
            r#"{{
                "tag_name": "{tag}",
                "html_url": "https://github.com/MustelGuild/Mustel/releases/tag/{tag}",
                "assets": [
                    {{"name": "mustel.exe", "browser_download_url": "https://example.invalid/mustel.exe", "size": 42, "digest": "sha256:aa"}},
                    {{"name": "mustel-gui.exe", "browser_download_url": "https://example.invalid/mustel-gui.exe", "size": 43}},
                    {{"name": "SHA256SUMS", "browser_download_url": "https://example.invalid/SHA256SUMS", "size": 10}}
                ]
            }}"#
        )
    }

    /// CONTRACT: IF-268
    #[tokio::test]
    async fn a_newer_release_comes_with_its_assets() {
        let transport = Fake {
            status: 200,
            body: release_json("v1.1.0"),
        };

        let release = check(
            &transport,
            &Version::new(1, 0, 0),
            "https://example.invalid/latest",
        )
        .await
        .expect("consulta")
        .expect("há versão nova");

        assert_eq!(release.version, Version::new(1, 1, 0));
        assert_eq!(release.assets.len(), 3);
        assert_eq!(
            release.asset("mustel.exe").map(|asset| asset.size),
            Some(42)
        );
        assert_eq!(
            release
                .asset("mustel.exe")
                .and_then(|asset| asset.digest.as_deref()),
            Some("sha256:aa")
        );
    }

    /// CONTRACT: IF-268
    #[tokio::test]
    async fn equal_or_older_releases_are_not_an_update() {
        let transport = Fake {
            status: 200,
            body: release_json("v1.0.0"),
        };
        assert!(
            check(
                &transport,
                &Version::new(1, 0, 0),
                "https://example.invalid/latest"
            )
            .await
            .expect("consulta")
            .is_none()
        );

        let transport = Fake {
            status: 200,
            body: release_json("v0.9.0"),
        };
        assert!(
            check(
                &transport,
                &Version::new(1, 0, 0),
                "https://example.invalid/latest"
            )
            .await
            .expect("consulta")
            .is_none()
        );
    }

    #[tokio::test]
    async fn a_missing_release_is_not_an_error() {
        let transport = Fake {
            status: 404,
            body: String::new(),
        };

        assert!(
            check(
                &transport,
                &Version::new(1, 0, 0),
                "https://example.invalid/latest"
            )
            .await
            .expect("consulta")
            .is_none()
        );
    }

    #[tokio::test]
    async fn an_http_error_is_actionable() {
        let transport = Fake {
            status: 403,
            body: String::new(),
        };

        let error = check(
            &transport,
            &Version::new(1, 0, 0),
            "https://example.invalid/latest",
        )
        .await
        .expect_err("403 vira erro");
        assert!(error.to_string().contains("403"), "{error}");
    }

    #[tokio::test]
    async fn a_broken_reply_is_an_error() {
        let transport = Fake {
            status: 200,
            body: "{".to_string(),
        };

        assert!(
            check(
                &transport,
                &Version::new(1, 0, 0),
                "https://example.invalid/latest"
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn a_non_version_tag_is_rejected() {
        let json = r#"{"tag_name":"release","html_url":"x","assets":[]}"#;
        assert!(parse_release(json, &Version::new(1, 0, 0)).is_err());
    }
}
