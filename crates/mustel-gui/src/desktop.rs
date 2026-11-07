//! Integração com a área de trabalho: abrir uma pasta no gerenciador de arquivos, abrir um arquivo
//! no editor do sistema e compor o relatório no cliente de e-mail.
//!
//! Duas responsabilidades, separadas de propósito:
//!
//! - **decidir** o comando e os argumentos por sistema operacional ([`open_command_for`],
//!   [`open_file_command_for`], [`compose_email_command_for`]) — funções puras, testáveis em
//!   qualquer máquina;
//! - **executar** ([`open_directory`], [`open_file`], [`compose_email`]), que são os únicos pontos
//!   que fazem `spawn`.
//!
//! O seletor de pastas propriamente dito vive em [`crate::picker`]; aqui só o que toca o SO.
//!
//! **Anexo do e-mail:** `mailto:` não anexa de forma portátil. No Linux o `xdg-email --attach`
//! anexa de verdade; Windows/macOS recebem o parâmetro `attach=` (Thunderbird/Evolution honram;
//! Outlook e Mail.app ignoram) e o `.log` salvo é a garantia.

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

/// Programa e argumentos para abrir `file` no **editor** do sistema.
///
/// Função pura, como [`open_command_for`]. As diferenças por SO não são capricho:
///
/// - **Windows**: `notepad`. Abrir arquivo com a ação padrão pediria `ShellExecute` (FFI, barrado
///   por `unsafe_code = "forbid"`) ou um shell (`cmd /c start`, proibido pela regra de processos
///   externos); `explorer arquivo` só **seleciona** o arquivo na pasta — não edita. `notepad` é o
///   editor que existe em toda instalação e abre texto puro sem intermediário.
/// - **Linux/macOS**: `xdg-open`/`open` com o arquivo abrem no aplicativo padrão do dono (o editor
///   que ele escolheu), que é o comportamento desejado quando existe.
pub fn open_file_command_for(os: TargetOs, file: &Path) -> (String, Vec<String>) {
    let path = file.to_string_lossy().to_string();

    match os {
        TargetOs::Windows => ("notepad".to_string(), vec![path]),
        TargetOs::Linux => ("xdg-open".to_string(), vec![path]),
        TargetOs::MacOs => ("open".to_string(), vec![path]),
    }
}

/// Abre `file` no editor do sistema.
///
/// Como [`open_directory`], o processo é disparado e **não** é esperado: o editor é do usuário, não
/// do app.
pub fn open_file(file: &Path) -> std::io::Result<()> {
    let (program, args) = open_file_command_for(TargetOs::current(), file);
    Command::new(program).args(args).spawn().map(|_| ())
}

/// Programa e argumentos para compor o relatório no cliente de e-mail.
///
/// Função pura. O **corpo** e o **assunto** vão crus para o `xdg-email` (Linux) e percent-encoded
/// dentro da URL `mailto:` (Windows/macOS). O anexo é melhor esforço: `xdg-email --attach` no
/// Linux; nos outros, o parâmetro `attach=` da URL, que só alguns clientes honram.
///
/// Nunca há shell: tudo é `Command::new(program).args(...)`.
pub fn compose_email_command_for(
    os: TargetOs,
    to: &str,
    subject: &str,
    body: &str,
    attachment: Option<&Path>,
) -> (String, Vec<String>) {
    match os {
        TargetOs::Linux => {
            let mut args = Vec::new();
            if let Some(path) = attachment {
                args.push("--attach".to_string());
                args.push(path.to_string_lossy().to_string());
            }
            args.push("--subject".to_string());
            args.push(subject.to_string());
            args.push("--body".to_string());
            args.push(body.to_string());
            args.push(to.to_string());

            ("xdg-email".to_string(), args)
        }
        // `FileProtocolHandler` entrega a URL ao aplicativo padrão sem `cmd /c start` (shell).
        TargetOs::Windows => (
            "rundll32.exe".to_string(),
            vec![
                "url.dll,FileProtocolHandler".to_string(),
                mailto_url(to, subject, body, attachment),
            ],
        ),
        TargetOs::MacOs => (
            "open".to_string(),
            vec![mailto_url(to, subject, body, attachment)],
        ),
    }
}

/// Abre o cliente de e-mail com o relatório.
///
/// No Linux, se o `xdg-email` (xdg-utils) não existir, cai no `xdg-open` com a URL `mailto:` — o
/// anexo, nesse caso, fica só no arquivo salvo.
pub fn compose_email(
    to: &str,
    subject: &str,
    body: &str,
    attachment: Option<&Path>,
) -> std::io::Result<()> {
    let os = TargetOs::current();
    let (program, args) = compose_email_command_for(os, to, subject, body, attachment);

    match Command::new(&program).args(&args).spawn() {
        Ok(_) => Ok(()),
        Err(_error) if os == TargetOs::Linux && program == "xdg-email" => {
            let url = mailto_url(to, subject, body, attachment);
            Command::new("xdg-open").arg(url).spawn().map(|_| ())
        }
        Err(error) => Err(error),
    }
}

/// A URL `mailto:` com assunto, corpo e (melhor esforço) anexo.
pub fn mailto_url(to: &str, subject: &str, body: &str, attachment: Option<&Path>) -> String {
    // RFC 6068: as quebras de linha do corpo vão em CRLF.
    let body = body.replace("\r\n", "\n").replace('\n', "\r\n");
    let mut url = format!(
        "mailto:{}?subject={}&body={}",
        percent_encode(to),
        percent_encode(subject),
        percent_encode(&body)
    );

    if let Some(path) = attachment {
        url.push_str("&attach=");
        url.push_str(&percent_encode(&file_url(path)));
    }

    url
}

/// Percent-encoding de um valor de query (RFC 3986).
fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// A URL `file://` de um caminho absoluto.
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");

    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

/// Resolve o caminho digitado na caixa para um caminho absoluto.///
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

    /// O comando de **abrir arquivo** por SO: editor no Windows, aplicativo padrão nos demais.
    ///
    /// O Windows não pode usar `explorer` (só seleciona) nem shell/FFI — daí o `notepad`.
    #[test]
    fn the_file_open_command_uses_the_system_editor() {
        let config = Path::new("C:\\Users\\dev\\AppData\\Roaming\\mustel\\mustel.jsonc");

        let windows = open_file_command_for(TargetOs::Windows, config);
        assert_eq!(windows.0, "notepad");
        assert_eq!(
            windows.1,
            vec!["C:\\Users\\dev\\AppData\\Roaming\\mustel\\mustel.jsonc".to_string()]
        );

        assert_eq!(
            open_file_command_for(
                TargetOs::Linux,
                Path::new("/home/dev/.config/mustel/mustel.jsonc")
            )
            .0,
            "xdg-open"
        );
        assert_eq!(
            open_file_command_for(TargetOs::MacOs, Path::new("/Users/dev/mustel.jsonc")).0,
            "open"
        );
    }

    /// Caminho com espaço também vai cru na abertura de arquivo.
    #[test]
    fn a_file_path_with_spaces_stays_a_single_argument() {
        let (_, args) = open_file_command_for(
            TargetOs::Windows,
            Path::new("C:\\Users\\dev\\App Data\\mustel.jsonc"),
        );
        assert_eq!(args.len(), 1, "um argumento só: {args:?}");
        assert_eq!(args[0], "C:\\Users\\dev\\App Data\\mustel.jsonc");
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

    /// A URL `mailto:` codifica assunto e corpo; as linhas do corpo vão em CRLF.
    #[test]
    fn the_mailto_url_encodes_the_subject_and_the_body() {
        let url = mailto_url(
            "support@mustelguild.dev",
            "Mustel 1.0.0 report",
            "linha 1\nlinha 2",
            None,
        );

        assert!(
            url.starts_with("mailto:support%40mustelguild.dev?subject=Mustel%201.0.0%20report"),
            "{url}"
        );
        assert!(url.contains("body=linha%201%0D%0Alinha%202"), "{url}");
        assert!(!url.contains(' '), "nenhum espaço cru: {url}");
    }

    /// O anexo vira uma URL `file://` percent-encoded dentro do `mailto:`.
    #[test]
    fn the_attachment_rides_as_an_encoded_file_url() {
        let url = mailto_url(
            "a@b.dev",
            "s",
            "b",
            Some(Path::new("/tmp/mustel report.log")),
        );

        assert!(
            url.contains("attach=file%3A%2F%2F%2Ftmp%2Fmustel%20report.log"),
            "{url}"
        );
    }

    /// No Linux o `xdg-email` anexa de verdade.
    /// CONTRACT: IF-267
    #[test]
    fn linux_attaches_the_report_with_xdg_email() {
        let path = Path::new("/tmp/mustel-report.log");
        let (program, args) =
            compose_email_command_for(TargetOs::Linux, "a@b.dev", "assunto", "corpo", Some(path));

        assert_eq!(program, "xdg-email");
        assert!(
            args.iter().any(|arg| arg.as_str() == "--attach"),
            "{args:?}"
        );
        assert!(
            args.iter()
                .any(|arg| arg.as_str() == "/tmp/mustel-report.log"),
            "{args:?}"
        );
        assert_eq!(args.last().map(String::as_str), Some("a@b.dev"));
    }

    /// No Windows a URL vai para o `FileProtocolHandler` — sem shell.
    #[test]
    fn windows_uses_the_protocol_handler_without_a_shell() {
        let (program, args) =
            compose_email_command_for(TargetOs::Windows, "a@b.dev", "s", "b", None);

        assert_eq!(program, "rundll32.exe");
        assert_eq!(args[0], "url.dll,FileProtocolHandler");
        assert!(args[1].starts_with("mailto:a%40b.dev"), "{}", args[1]);
    }

    /// No macOS o `open` entrega a URL ao cliente padrão.
    #[test]
    fn macos_opens_the_default_mail_client() {
        let (program, args) = compose_email_command_for(TargetOs::MacOs, "a@b.dev", "s", "b", None);

        assert_eq!(program, "open");
        assert!(args[0].starts_with("mailto:"), "{}", args[0]);
    }
}
