//! Integração com a área de trabalho: abrir uma pasta no gerenciador de arquivos.
//!
//! Duas responsabilidades, separadas de propósito:
//!
//! - **decidir** o comando e os argumentos por sistema operacional ([`open_command_for`]) — função pura,
//!   testável em qualquer máquina;
//! - **executar** ([`open_directory`]), que é o único ponto que faz `spawn`.
//!
//! O seletor de pastas propriamente dito vive em [`crate::picker`]; aqui só o que toca o SO.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Sistema operacional alvo do comando de abrir pasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    /// Windows: `explorer`.
    Windows,
    /// Linux: `xdg-open`.
    Linux,
    /// macOS: `open` (fora dos alvos suportados, mas o comando existe).
    MacOs,
}

impl TargetOs {
    /// O SO em que este binário foi compilado.
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

/// Programa e argumentos para abrir `directory` no gerenciador de arquivos.
///
/// Função pura: o teste confere a montagem sem abrir janela nenhuma.
///
/// O Windows usa `explorer` porque é o gerenciador nativo. **Não** se checa o código de saída:
/// `explorer` devolve 1 mesmo quando abre a pasta com sucesso, então tratá-lo como erro daria falso
/// negativo em todo clique certo.
pub fn open_command_for(os: TargetOs, directory: &Path) -> (String, Vec<String>) {
    let path = directory.to_string_lossy().to_string();

    match os {
        // O argumento vai cru: `explorer` aceita caminho com espaços sem aspas (e aspas viram parte
        // do nome). Nunca há shell no meio — é `Command::arg`, não uma string interpolada.
        TargetOs::Windows => ("explorer".to_string(), vec![path]),
        TargetOs::Linux => ("xdg-open".to_string(), vec![path]),
        TargetOs::MacOs => ("open".to_string(), vec![path]),
    }
}

/// Abre `directory` no gerenciador de arquivos do SO atual.
///
/// O processo é disparado e **não** é esperado: o gerenciador é do usuário, não do app. A saída
/// também não é capturada (o `explorer` imprime e retorna; esperar por ele travaria a UI).
pub fn open_directory(directory: &Path) -> std::io::Result<()> {
    let (program, args) = open_command_for(TargetOs::current(), directory);
    Command::new(program).args(args).spawn().map(|_| ())
}

/// Resolve o caminho digitado na caixa para um caminho absoluto.
///
/// A caixa aceita caminho relativo (`./results`); abrir ou criar relativo ao *working directory do
/// processo* é frágil — o app pode ter sido aberto de outro lugar. Relativo é resolvido a partir de
/// `base`, e o resultado sai normalizado.
pub fn resolve_directory(input: &str, base: &Path) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return base.join("results");
    }

    let path = Path::new(trimmed);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };

    std::path::absolute(&absolute).unwrap_or(absolute)
}

/// A pasta existe e é um diretório? É o predicado que habilita o botão de abrir.
///
/// Um arquivo com o mesmo nome **não** habilita: abrir "a pasta" apontando para um arquivo seria
/// outra ação (revelar o arquivo), que não é o que o botão promete.
pub fn can_open(directory: &Path) -> bool {
    directory.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O comando de cada SO: programa e argumento único com o caminho cru.
    #[test]
    fn the_open_command_is_native_per_os() {
        let directory = Path::new("C:\\Users\\dev\\resultados");

        let windows = open_command_for(TargetOs::Windows, directory);
        assert_eq!(windows.0, "explorer");
        assert_eq!(windows.1, vec!["C:\\Users\\dev\\resultados".to_string()]);

        let linux = open_command_for(TargetOs::Linux, Path::new("/home/dev/resultados"));
        assert_eq!(linux.0, "xdg-open");
        assert_eq!(linux.1, vec!["/home/dev/resultados".to_string()]);

        let mac = open_command_for(TargetOs::MacOs, Path::new("/Users/dev/resultados"));
        assert_eq!(mac.0, "open");
    }

    /// Caminho com espaço não é citado nem interpolado: vai como um argumento só.
    #[test]
    fn a_path_with_spaces_stays_a_single_argument() {
        let (_, args) = open_command_for(TargetOs::Linux, Path::new("/tmp/minha pasta/resultados"));
        assert_eq!(args, vec!["/tmp/minha pasta/resultados".to_string()]);
    }

    /// Relativo é resolvido a partir da base informada, e sai absoluto.
    #[test]
    fn a_relative_path_is_resolved_against_the_base() {
        // A base vem de um diretório real: `absolute` no Windows prefixa a unidade do processo, então
        // uma base inventada como `/projeto` não é o teste certo em nenhum dos dois SOs.
        let base = tempfile::tempdir().expect("tempdir");
        let base_path = base.path();

        assert_eq!(
            resolve_directory("./results", base_path),
            base_path.join("results")
        );
        assert_eq!(
            resolve_directory("saida", base_path),
            base_path.join("saida")
        );
        assert!(
            resolve_directory("saida", base_path).is_absolute(),
            "o resultado precisa ser absoluto"
        );
    }

    /// Caminho absoluto passa intacto.
    #[test]
    fn an_absolute_path_is_kept() {
        let base = tempfile::tempdir().expect("tempdir");
        let absolute = base.path().join("dados");
        assert_eq!(
            resolve_directory(&absolute.display().to_string(), base.path()),
            absolute
        );
    }

    /// Caixa vazia cai no default do produto (`results`), não no diretório atual.
    #[test]
    fn an_empty_box_falls_back_to_results() {
        let base = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            resolve_directory("   ", base.path()),
            base.path().join("results")
        );
    }

    /// O botão só habilita para diretório existente.
    #[test]
    fn the_open_button_requires_an_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(can_open(dir.path()), "diretório existente habilita");

        let missing = dir.path().join("nao-existe");
        assert!(!can_open(&missing), "pasta ausente não habilita");

        let file = dir.path().join("arquivo.txt");
        std::fs::write(&file, "x").expect("escreve");
        assert!(!can_open(&file), "arquivo com o mesmo nome não habilita");
    }
}
