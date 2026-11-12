//! Download do executável, checksums e verificação de SHA-256.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::CoreError;

use super::model::{Asset, Release};
use super::transport::UpdateTransport;

/// O asset com os checksums de todos os anexos.
const SUMS_ASSET: &str = "SHA256SUMS";

/// O SHA-256 esperado de cada asset.
///
/// Usa o `digest` que a API do GitHub informa (quando presente) e, para o que faltar, baixa o
/// `SHA256SUMS` da mesma release.
pub async fn checksums<T: UpdateTransport>(
    transport: &T,
    release: &Release,
) -> Result<BTreeMap<String, String>, CoreError> {
    let mut sums = BTreeMap::new();
    let mut missing = false;

    for asset in &release.assets {
        if asset.name == SUMS_ASSET {
            continue;
        }
        match asset.digest.as_deref().and_then(parse_digest) {
            Some(hash) => {
                sums.insert(asset.name.clone(), hash);
            }
            None => missing = true,
        }
    }

    if missing {
        let Some(asset) = release.asset(SUMS_ASSET) else {
            return Err(CoreError::Other(
                "The release has no SHA256SUMS; refusing to install it.".to_string(),
            ));
        };
        let response = transport.get(&asset.url).await?;
        if !(200..300).contains(&response.status) {
            return Err(CoreError::Other(format!(
                "Could not download SHA256SUMS (HTTP {})",
                response.status
            )));
        }
        let text = String::from_utf8(response.body)
            .map_err(|error| CoreError::Other(format!("SHA256SUMS is not UTF-8: {error}")))?;
        for (name, hash) in parse_sums(&text) {
            sums.entry(name).or_insert(hash);
        }
    }

    Ok(sums)
}

/// `sha256:<hex>` → `<hex>`.
fn parse_digest(digest: &str) -> Option<String> {
    let hash = digest.strip_prefix("sha256:")?;
    is_sha256(hash).then(|| hash.to_ascii_lowercase())
}

/// Linhas `hash  ./nome` (ou `hash *nome`) do `SHA256SUMS`.
pub fn parse_sums(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let name = parts.next()?;
            if !is_sha256(hash) {
                return None;
            }
            let name = name.strip_prefix('*').unwrap_or(name);
            let name = name.strip_prefix("./").unwrap_or(name);
            Some((name.to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

/// 64 dígitos hexadecimais.
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit())
}

/// Baixa `asset` para `destination` e confere tamanho e SHA-256.
///
/// `progress(done, total)` devolve `false` para cancelar; o arquivo parcial é removido em qualquer
/// falha. No Unix o arquivo final fica executável (`0755`).
pub async fn download<T: UpdateTransport>(
    transport: &T,
    asset: &Asset,
    destination: &Path,
    expected_sha256: &str,
    progress: &mut (dyn FnMut(u64, Option<u64>) -> bool + Send),
) -> Result<(), CoreError> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            CoreError::Other(format!("Could not create the update folder: {error}"))
        })?;
    }

    if let Err(error) = transport.download(&asset.url, destination, progress).await {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(error);
    }

    let metadata = tokio::fs::metadata(destination).await.map_err(|error| {
        CoreError::Other(format!("Could not inspect the downloaded update: {error}"))
    })?;
    if asset.size > 0 && metadata.len() != asset.size {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(CoreError::Other(format!(
            "The update has {} bytes; the release says {}",
            metadata.len(),
            asset.size
        )));
    }

    let path = destination.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|error| CoreError::Other(format!("The checksum task failed: {error}")))??;

    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(CoreError::Other(
            "The update failed the SHA-256 check; the downloaded file was discarded.".to_string(),
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(destination, permissions).map_err(|error| {
            CoreError::Other(format!("Could not make the update executable: {error}"))
        })?;
    }

    Ok(())
}

/// SHA-256 (hex minúsculo) de um arquivo.
fn sha256_file(path: &Path) -> Result<String, CoreError> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        CoreError::Other(format!("Could not open the downloaded file: {error}"))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            CoreError::Other(format!("Could not read the downloaded file: {error}"))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(lower_hex(&hasher.finalize()))
}

/// Converte bytes para hexadecimal minúsculo (dois dígitos por byte).
///
/// O `digest` 0.11 (usado pelo `sha2` 0.11) não implementa mais `LowerHex` na saída do
/// `finalize`; a formatação é feita aqui, do mesmo jeito que o `mustel-testkit` já faz.
fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &[u8] = b"conteudo do update";

    fn hash_of(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        lower_hex(&hasher.finalize())
    }

    struct Fake {
        sums: Option<String>,
        cancel: bool,
        body: Vec<u8>,
    }

    impl UpdateTransport for Fake {
        async fn get(
            &self,
            _url: &str,
        ) -> Result<crate::update::transport::HttpResponse, CoreError> {
            let Some(sums) = &self.sums else {
                return Ok(crate::update::transport::HttpResponse {
                    status: 404,
                    body: Vec::new(),
                });
            };
            Ok(crate::update::transport::HttpResponse {
                status: 200,
                body: sums.clone().into_bytes(),
            })
        }

        async fn download<'a>(
            &'a self,
            _url: &'a str,
            into: &'a Path,
            progress: &'a mut (dyn FnMut(u64, Option<u64>) -> bool + Send),
        ) -> Result<(), CoreError> {
            let total = Some(self.body.len() as u64);
            if !progress(self.body.len() as u64 / 2, total) {
                return Err(CoreError::Cancelled);
            }
            tokio::fs::write(into, &self.body)
                .await
                .map_err(|error| CoreError::Other(error.to_string()))?;
            if self.cancel || !progress(self.body.len() as u64, total) {
                return Err(CoreError::Cancelled);
            }
            Ok(())
        }
    }

    fn asset(size: u64) -> Asset {
        Asset {
            name: "mustel.exe".to_string(),
            url: "https://example.invalid/mustel.exe".to_string(),
            size,
            digest: None,
        }
    }

    /// CONTRACT: IF-270
    #[tokio::test]
    async fn a_matching_hash_is_accepted() {
        let dir = tempfile::tempdir().expect("temporário");
        let destination = dir.path().join("mustel.exe");
        let transport = Fake {
            sums: None,
            cancel: false,
            body: BODY.to_vec(),
        };
        let mut progress = |_done: u64, _total: Option<u64>| true;

        download(
            &transport,
            &asset(BODY.len() as u64),
            &destination,
            &hash_of(BODY),
            &mut progress,
        )
        .await
        .expect("baixa");

        assert_eq!(std::fs::read(&destination).expect("lê"), BODY);
    }

    /// CONTRACT: IF-270
    #[tokio::test]
    async fn a_bad_hash_discards_the_file() {
        let dir = tempfile::tempdir().expect("temporário");
        let destination = dir.path().join("mustel.exe");
        let transport = Fake {
            sums: None,
            cancel: false,
            body: BODY.to_vec(),
        };
        let mut progress = |_done: u64, _total: Option<u64>| true;

        let error = download(
            &transport,
            &asset(BODY.len() as u64),
            &destination,
            &"0".repeat(64),
            &mut progress,
        )
        .await
        .expect_err("hash errado");

        assert!(error.to_string().contains("SHA-256"), "{error}");
        assert!(!destination.exists(), "o parcial foi apagado");
    }

    #[tokio::test]
    async fn a_size_mismatch_discards_the_file() {
        let dir = tempfile::tempdir().expect("temporário");
        let destination = dir.path().join("mustel.exe");
        let transport = Fake {
            sums: None,
            cancel: false,
            body: BODY.to_vec(),
        };
        let mut progress = |_done: u64, _total: Option<u64>| true;

        download(
            &transport,
            &asset(BODY.len() as u64 + 10),
            &destination,
            &hash_of(BODY),
            &mut progress,
        )
        .await
        .expect_err("tamanho errado");

        assert!(!destination.exists(), "o parcial foi apagado");
    }

    /// CONTRACT: IF-270
    #[tokio::test]
    async fn cancelling_keeps_no_partial_file() {
        let dir = tempfile::tempdir().expect("temporário");
        let destination = dir.path().join("mustel.exe");
        let transport = Fake {
            sums: None,
            cancel: true,
            body: BODY.to_vec(),
        };
        let mut progress = |_done: u64, _total: Option<u64>| true;

        let error = download(
            &transport,
            &asset(BODY.len() as u64),
            &destination,
            &hash_of(BODY),
            &mut progress,
        )
        .await
        .expect_err("cancelado");
        assert!(matches!(error, CoreError::Cancelled), "{error}");
        assert!(!destination.exists());
    }

    #[test]
    fn sha256sums_parse_with_and_without_dot_slash() {
        let hash = "a".repeat(64);
        let text = format!("{hash}  ./mustel\n{hash} *mustel-gui.exe\nlixo\n");

        assert_eq!(
            parse_sums(&text),
            vec![
                ("mustel".to_string(), hash.clone()),
                ("mustel-gui.exe".to_string(), hash)
            ]
        );
    }

    #[tokio::test]
    async fn the_digest_wins_and_sums_fill_the_gaps() {
        let hash = "b".repeat(64);
        let release = Release {
            version: semver::Version::new(1, 1, 0),
            assets: vec![
                Asset {
                    name: "mustel.exe".to_string(),
                    url: "x".to_string(),
                    size: 1,
                    digest: Some(format!("sha256:{}", "a".repeat(64))),
                },
                Asset {
                    name: "mustel-gui.exe".to_string(),
                    url: "x".to_string(),
                    size: 1,
                    digest: None,
                },
                Asset {
                    name: SUMS_ASSET.to_string(),
                    url: "https://example.invalid/SHA256SUMS".to_string(),
                    size: 1,
                    digest: None,
                },
            ],
        };
        let transport = Fake {
            sums: Some(format!("{hash}  ./mustel-gui.exe\n")),
            cancel: false,
            body: Vec::new(),
        };

        let sums = checksums(&transport, &release).await.expect("checksums");

        assert_eq!(sums.get("mustel.exe"), Some(&"a".repeat(64)));
        assert_eq!(sums.get("mustel-gui.exe"), Some(&hash));
    }

    #[tokio::test]
    async fn a_release_without_sums_and_without_digest_is_refused() {
        let release = Release {
            version: semver::Version::new(1, 1, 0),
            assets: vec![Asset {
                name: "mustel".to_string(),
                url: "x".to_string(),
                size: 1,
                digest: None,
            }],
        };
        let transport = Fake {
            sums: None,
            cancel: false,
            body: Vec::new(),
        };

        assert!(checksums(&transport, &release).await.is_err());
    }
}
