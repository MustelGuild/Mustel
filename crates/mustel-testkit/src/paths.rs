//! Caminhos usados pelos testes e pelo comparador (raiz do repositório, binário, baseline).

use std::path::{Path, PathBuf};

/// Raiz do repositório (dois níveis acima deste crate).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate> must live two levels under the repo root")
        .to_path_buf()
}

/// Diretório do baseline versionado.
pub fn baseline_dir() -> PathBuf {
    repo_root().join("fixtures").join("baseline")
}

/// Diretório de trabalho dos fixtures (ignorado pelo Git).
pub fn workspace_dir() -> PathBuf {
    repo_root().join("fixtures").join("workdir")
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

/// Caminho do `pgpass.conf` isolado usado nas comparações.
///
/// Fica dentro do diretório de trabalho do harness (ignorado pelo Git) e chega ao `mustel` por
/// `MUSTEL_PGPASS_FILE` — o arquivo real do usuário nunca é tocado do lado do Mustel.
pub fn mustel_pgpass_file() -> PathBuf {
    workspace_dir()
        .join("mustel-pgpass")
        .join("postgresql")
        .join("pgpass.conf")
}
