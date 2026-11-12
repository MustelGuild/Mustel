//! O updater na janela: estado, alvos, download e aplicação.

use std::path::{Path, PathBuf};

use mustel_core::error::CoreError;
use mustel_core::signal::CancellationToken;
use mustel_core::update::{
    ApplyRequest, ApplyTarget, Asset, Release, ReqwestTransport, TargetPlatform, checksums,
    download, helper_command,
};

use crate::toast::ToastId;

/// O estado do updater na janela.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// A release conhecida, quando o check achou uma versão nova.
    pub(crate) release: Option<Release>,
    /// O toast sticky do update.
    pub(crate) toast: Option<ToastId>,
    /// O token do download em andamento (para o `X` cancelar).
    pub(crate) cancel: Option<CancellationToken>,
    /// Há um check em andamento?
    pub(crate) checking: bool,
    /// Há um download em andamento?
    pub(crate) downloading: bool,
    /// O check automático já foi agendado nesta sessão?
    pub(crate) checked_once: bool,
}

/// Um executável a atualizar e o asset correspondente.
#[derive(Debug, Clone)]
pub(crate) struct Target {
    /// Nome do arquivo no diretório (`mustel.exe`, …).
    pub(crate) name: String,
    /// O executável em uso/sibling.
    pub(crate) target: PathBuf,
    /// Onde baixar o payload (quando a pasta é gravável).
    pub(crate) payload: PathBuf,
    /// O asset da release.
    pub(crate) asset: Asset,
}

/// O que o download terminou fazendo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Installed {
    /// O helper foi disparado; o app vai reiniciar.
    Started,
    /// A pasta do executável não é gravável: o arquivo ficou aqui para instalar à mão.
    Manual(PathBuf),
}

/// Os alvos do update para este binário.
///
/// No Windows, os **dois** `.exe` que estiverem lado a lado; no Linux, o `mustel`. O executável em
/// uso precisa estar entre os alvos (senão o helper não teria o que relançar).
pub(crate) fn targets(release: &Release, current: &Path) -> Result<Vec<Target>, String> {
    let Some(dir) = current.parent() else {
        return Err("Could not find the folder of this executable.".to_string());
    };
    let platform = TargetPlatform::current();

    let mut names: Vec<String> = Vec::new();
    match platform {
        TargetPlatform::Windows => {
            for name in ["mustel.exe", "mustel-gui.exe"] {
                if dir.join(name).is_file() {
                    names.push(name.to_string());
                }
            }
        }
        TargetPlatform::Linux => {
            let name = current
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            if mustel_core::update::asset_name_for(platform, &name).is_some() {
                names.push(name);
            }
        }
        TargetPlatform::MacOs => {
            return Err("Updates are not available on macOS.".to_string());
        }
    }

    if names.is_empty() {
        return Err("No executable to update was found next to this app.".to_string());
    }

    let version = release.version.to_string();
    let mut targets = Vec::new();
    for name in names {
        let Some(asset_name) = mustel_core::update::asset_name_for(platform, &name) else {
            continue;
        };
        let Some(asset) = release.asset(asset_name) else {
            return Err(format!("The release has no asset '{asset_name}'."));
        };
        targets.push(Target {
            name: name.clone(),
            target: dir.join(&name),
            payload: dir.join(format!(".mustel-update-{version}-{name}.bin")),
            asset: asset.clone(),
        });
    }

    if !targets.iter().any(|entry| entry.target == current) {
        return Err(
            "This executable does not match a release package; update it manually.".to_string(),
        );
    }

    Ok(targets)
}

/// O caminho do helper (uma cópia do executável em uso, no mesmo diretório).
pub(crate) fn helper_path(current: &Path) -> PathBuf {
    let dir = current.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    match current.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if !extension.is_empty() => {
            dir.join(format!(".mustel-helper-{pid}.{extension}"))
        }
        _ => dir.join(format!(".mustel-helper-{pid}")),
    }
}

/// A URL da API (produção) ou o override de debug para o smoke.
pub(crate) fn check_url() -> Result<String, String> {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("MUSTEL_UPDATE_API")
        && !url.trim().is_empty()
    {
        return Ok(url);
    }

    let slug = mustel_core::update::repository_slug(env!("CARGO_PKG_REPOSITORY"))
        .ok_or_else(|| "The repository URL is not configured.".to_string())?;
    Ok(mustel_core::update::api_latest_url(&slug))
}

/// Pasta de fallback quando o diretório do executável não é gravável.
pub(crate) fn fallback_dir(version: &str) -> Result<PathBuf, String> {
    let root = mustel_core::activity::paths::state_root()
        .ok_or_else(|| "The system has no state directory for the update.".to_string())?;
    Ok(root.join("updates").join(version))
}

/// A pasta aceita escrita?
pub(crate) fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".mustel-write-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Baixa os payloads e, quando dá, dispara o helper.
///
/// `fallback` é usado quando a pasta do executável é somente leitura: os arquivos vão para lá e o
/// retorno é [`Installed::Manual`] (nada é substituído).
pub(crate) async fn install(
    release: &Release,
    targets: &[Target],
    current: &Path,
    fallback: Option<&Path>,
    token: &CancellationToken,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<Installed, CoreError> {
    let transport = ReqwestTransport::new()?;
    let sums = checksums(&transport, release).await?;

    let directory = fallback
        .map(Path::to_path_buf)
        .or_else(|| current.parent().map(Path::to_path_buf))
        .ok_or_else(|| CoreError::Other("Could not find the update folder.".to_string()))?;
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| {
            CoreError::Other(format!("Could not create the update folder: {error}"))
        })?;

    let total: u64 = targets.iter().map(|target| target.asset.size).sum();
    let total = total.max(1);
    let mut base = 0_u64;
    let mut downloaded: Vec<PathBuf> = Vec::new();

    for target in targets {
        let Some(checksum) = sums.get(&target.asset.name) else {
            cleanup(&downloaded).await;
            return Err(CoreError::Other(format!(
                "The release has no SHA-256 for '{}'.",
                target.asset.name
            )));
        };
        let payload = match fallback {
            Some(_) => directory.join(&target.name),
            None => target.payload.clone(),
        };

        let mut on_chunk = |done: u64, _asset_total: Option<u64>| {
            progress(base + done, Some(total));
            !token.is_cancelled()
        };
        if let Err(error) =
            download(&transport, &target.asset, &payload, checksum, &mut on_chunk).await
        {
            cleanup(&downloaded).await;
            return Err(error);
        }
        base += target.asset.size;
        downloaded.push(payload);
    }

    if let Some(directory) = fallback {
        return Ok(Installed::Manual(directory.to_path_buf()));
    }

    // Copia o exe atual para o helper (o Windows não deixa mover um exe em execução).
    let helper = helper_path(current);
    let source = current.to_path_buf();
    let destination = helper.clone();
    tokio::task::spawn_blocking(move || std::fs::copy(&source, &destination))
        .await
        .map_err(|error| CoreError::Other(format!("The helper copy task failed: {error}")))?
        .map_err(|error| CoreError::Other(format!("Could not copy the updater helper: {error}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755));
    }

    let request = ApplyRequest {
        targets: targets
            .iter()
            .map(|target| ApplyTarget {
                target: target.target.clone(),
                payload: target.payload.clone(),
            })
            .collect(),
        launch: current.to_path_buf(),
        from: env!("CARGO_PKG_VERSION").to_string(),
        relaunch: std::env::args_os().skip(1).collect(),
    };
    helper_command(&helper, &request).spawn().map_err(|error| {
        CoreError::Other(format!("Could not start the updater helper: {error}"))
    })?;

    Ok(Installed::Started)
}

/// Apaga os payloads já baixados (falha ou cancelamento).
async fn cleanup(paths: &[PathBuf]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            version: semver::Version::new(1, 1, 0),
            assets: vec![
                Asset {
                    name: "mustel".to_string(),
                    url: "x".to_string(),
                    size: 1,
                    digest: Some(format!("sha256:{}", "a".repeat(64))),
                },
                Asset {
                    name: "mustel.exe".to_string(),
                    url: "x".to_string(),
                    size: 1,
                    digest: Some(format!("sha256:{}", "b".repeat(64))),
                },
                Asset {
                    name: "mustel-gui.exe".to_string(),
                    url: "x".to_string(),
                    size: 1,
                    digest: Some(format!("sha256:{}", "c".repeat(64))),
                },
            ],
        }
    }

    /// CONTRACT: IF-270
    #[test]
    fn the_linux_target_is_the_raw_mustel() {
        let dir = tempfile::tempdir().expect("temporário");
        let current = dir.path().join("mustel");
        std::fs::write(&current, b"x").expect("grava");

        if cfg!(target_os = "linux") {
            let targets = targets(&release(), &current).expect("alvos");
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].name, "mustel");
            assert_eq!(targets[0].target, current);
        }
    }

    #[test]
    fn a_renamed_executable_is_refused() {
        let dir = tempfile::tempdir().expect("temporário");
        let current = dir.path().join("outro");
        std::fs::write(&current, b"x").expect("grava");

        if cfg!(target_os = "linux") {
            assert!(targets(&release(), &current).is_err());
        }
    }

    #[test]
    fn the_helper_sits_next_to_the_executable() {
        let path = helper_path(Path::new("/tmp/mustel"));
        assert_eq!(path.parent(), Some(Path::new("/tmp")));
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        assert!(name.starts_with(".mustel-helper-"), "{name}");
    }

    /// CONTRACT: IF-270
    #[test]
    fn a_writable_directory_is_detected() {
        let dir = tempfile::tempdir().expect("temporário");
        assert!(writable(dir.path()));
    }

    /// O fallback do update fica em `<state>/updates/<versão>`.
    ///
    /// É o caminho usado quando a pasta do executável é somente leitura (instalação do sistema);
    /// o app não pode sobrescrever `/usr/bin/mustel`, então guarda o payload aqui.
    /// CONTRACT: IF-270
    #[test]
    fn the_fallback_lives_under_the_state_root() {
        if let Ok(path) = fallback_dir("1.1.0") {
            assert!(path.ends_with("updates/1.1.0"), "{path:?}");
        }
    }

    /// CONTRACT: IF-268
    #[test]
    fn the_check_url_is_the_github_latest_release() {
        if std::env::var_os("MUSTEL_UPDATE_API").is_none() {
            let url = check_url().expect("url");
            assert!(url.contains("api.github.com"), "{url}");
            assert!(url.ends_with("/releases/latest"), "{url}");
        }
    }
}
