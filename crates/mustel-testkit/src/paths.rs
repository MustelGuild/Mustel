//! Caminhos usados pelo harness (raiz do repositório, binário legado, baseline).

use std::path::{Path, PathBuf};

/// Raiz do repositório (dois níveis acima deste crate).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate> must live two levels under the repo root")
        .to_path_buf()
}

/// Diretório onde o binário do oráculo é publicado.
pub fn oracle_bin_dir() -> PathBuf {
    repo_root().join("oracle").join("bin")
}

/// Diretório do baseline versionado.
pub fn baseline_dir() -> PathBuf {
    repo_root().join("oracle").join("baseline")
}

/// Diretório de trabalho dos fixtures (ignorado pelo Git).
pub fn workspace_dir() -> PathBuf {
    repo_root().join("oracle").join("workdir")
}

/// Projeto do CLI legado.
pub fn legacy_project() -> PathBuf {
    repo_root()
        .join("legacy")
        .join("FurLab.CLI")
        .join("FurLab.CLI.csproj")
}

/// Caminho do binário do Mustel, se existir (`MUSTEL_BIN` tem precedência).
pub fn mustel_binary() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os("MUSTEL_BIN") {
        let path = PathBuf::from(override_path);
        if path.is_file() {
            return Some(path);
        }
    }

    for profile in ["debug", "release"] {
        let dir = repo_root().join("target").join(profile);
        for suffix in ["", ".exe"] {
            let candidate = dir.join(format!("mustel{suffix}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Diretório isolado de configuração do Mustel usado nas comparações.
pub fn mustel_config_dir() -> PathBuf {
    workspace_dir().join("mustel-config")
}

/// Caminho do `mustel.jsonc` usado nas comparações.
pub fn mustel_config_file() -> PathBuf {
    mustel_config_dir().join("mustel.jsonc")
}

/// Nomes possíveis do executável publicado do legado.
const ORACLE_EXE_NAMES: [&str; 2] = ["FurLab.CLI", "fur"];

/// Caminho do executável do oráculo, se já estiver publicado (aceita `MUSTEL_ORACLE_BIN`).
pub fn oracle_binary() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os("MUSTEL_ORACLE_BIN") {
        let path = PathBuf::from(override_path);
        if path.is_file() {
            return Some(path);
        }
    }

    let dir = oracle_bin_dir();
    for name in ORACLE_EXE_NAMES {
        for suffix in ["", ".exe"] {
            let candidate = dir.join(format!("{name}{suffix}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
