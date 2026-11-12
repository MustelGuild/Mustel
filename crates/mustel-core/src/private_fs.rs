//! Arquivos sensíveis: modo `0600` no arquivo e `0700` no diretório, no Unix.
//!
//! No Windows a garantia é a ACL do `%APPDATA%` (por usuário), já documentada; estas funções só
//! mudam algo no Unix.

use std::io::Write as _;
use std::path::Path;

/// Cria o diretório (e os pais) e restringe para `0700` no Unix.
pub fn create_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    restrict_dir(path)
}

/// Grava o arquivo inteiro com `0600` no Unix (cria ou trunca).
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    restrict_file(path)
}

/// Grava um arquivo **novo** com `0600` no Unix e falha se o caminho já existir.
///
/// `create_new` é o que impede sobrescrever um arquivo e seguir um symlink plantado: a abertura
/// com `O_CREAT | O_EXCL` recusa o caminho que já existe.
pub fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

/// Abre para *append*, criando com `0600` no Unix, e restringe um arquivo preexistente.
pub fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    restrict_file(path)?;
    Ok(file)
}

#[cfg(unix)]
/// Restringe um arquivo existente para `0600` no Unix.
pub fn restrict_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
/// No Windows não há modo POSIX; a ACL do diretório é a garantia.
pub fn restrict_file(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
/// Restringe um diretório existente para `0700` no Unix.
pub fn restrict_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
/// No Windows não há modo POSIX; a ACL do diretório é a garantia.
pub fn restrict_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_files_are_0600_and_directories_are_0700() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("tempdir");
        let directory = root.path().join("config");
        create_dir(&directory).expect("cria");
        let path = directory.join("mustel.jsonc");
        write(&path, b"{}").expect("grava");

        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&directory)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    /// A gravação de relatório não pode sobrescrever nem seguir um symlink.
    #[test]
    fn write_new_refuses_an_existing_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("report.log");
        super::write_new(&path, b"primeiro").expect("cria");

        let error = super::write_new(&path, b"segundo").expect_err("não sobrescreve");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).expect("lê"), b"primeiro");
    }

    #[cfg(unix)]
    #[test]
    fn write_new_does_not_follow_a_symlink() {
        let root = tempfile::tempdir().expect("tempdir");
        let target = root.path().join("target");
        std::fs::write(&target, b"original").expect("target");
        let link = root.path().join("report.log");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let error = super::write_new(&link, b"adulterado").expect_err("recusa o symlink");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&target).expect("lê"), b"original");
    }
}
