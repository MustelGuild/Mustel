//! O helper de substituição e o relançamento do app.
//!
//! O executável em execução não pode ser sobrescrito no Windows. O app copia **ele mesmo** para um
//! helper temporário, baixa os payloads e chama o helper com
//! `--apply-update --target … --payload … --launch … --from <versão> -- <args>`. O helper (o modo
//! oculto) faz o backup `.old`, troca os executáveis e relança o app — com
//! `MUSTEL_UPDATED_FROM` no sucesso ou `MUSTEL_UPDATE_FAILED` na falha.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::activity::{Area, Entry, Status, Store};

/// Quanto tempo o helper insiste antes de desistir (lock do Windows).
const RETRY: Duration = Duration::from_secs(30);
/// Intervalo entre as tentativas.
const RETRY_STEP: Duration = Duration::from_millis(100);
/// `CREATE_NO_WINDOW` do Windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Um par executável baixado → executável em uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyTarget {
    /// O executável que será substituído.
    pub target: PathBuf,
    /// O arquivo baixado com a versão nova.
    pub payload: PathBuf,
}

/// O que o helper precisa para aplicar e relançar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    /// Os pares de executáveis.
    pub targets: Vec<ApplyTarget>,
    /// O executável que deve ser relançado (o que estava em uso).
    pub launch: PathBuf,
    /// A versão de origem (vai no env do relaunch).
    pub from: String,
    /// Os argumentos do app, sem o nome do binário.
    pub relaunch: Vec<OsString>,
}

/// Lê os argumentos do modo oculto.
///
/// Devolve `None` quando o primeiro argumento não é `--apply-update` (é o app normal).
pub fn parse_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<ApplyRequest>, String> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--apply-update")) {
        return Ok(None);
    }

    let mut targets: Vec<ApplyTarget> = Vec::new();
    let mut launch = None;
    let mut from = None;
    let mut relaunch = Vec::new();

    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--target") => {
                let target = args.next().ok_or("--target sem valor")?;
                targets.push(ApplyTarget {
                    target: PathBuf::from(target),
                    payload: PathBuf::new(),
                });
            }
            Some("--payload") => {
                let payload = args.next().ok_or("--payload sem valor")?;
                let Some(last) = targets.last_mut() else {
                    return Err("--payload before --target".to_string());
                };
                if !last.payload.as_os_str().is_empty() {
                    return Err("--payload repeated for the same --target".to_string());
                }
                last.payload = PathBuf::from(payload);
            }
            Some("--launch") => {
                launch = Some(PathBuf::from(args.next().ok_or("--launch sem valor")?));
            }
            Some("--from") => {
                from = Some(
                    args.next()
                        .ok_or("--from sem valor")?
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            Some("--") => {
                relaunch.extend(args);
                break;
            }
            other => return Err(format!("unexpected argument: {other:?}")),
        }
    }

    if targets.is_empty() {
        return Err("no --target given".to_string());
    }
    if targets
        .iter()
        .any(|entry| entry.payload.as_os_str().is_empty())
    {
        return Err("every --target needs a --payload".to_string());
    }
    let launch = launch.ok_or("--launch missing")?;
    if !targets.iter().any(|entry| entry.target == launch) {
        return Err("--launch is not one of the targets".to_string());
    }
    let from = from.ok_or("--from missing")?;

    Ok(Some(ApplyRequest {
        targets,
        launch,
        from,
        relaunch,
    }))
}

/// O comando que o app usa para chamar o helper.
pub fn helper_command(helper: &Path, request: &ApplyRequest) -> Command {
    let mut command = Command::new(helper);
    command.arg("--apply-update");
    for target in &request.targets {
        command
            .arg("--target")
            .arg(&target.target)
            .arg("--payload")
            .arg(&target.payload);
    }
    command
        .arg("--launch")
        .arg(&request.launch)
        .arg("--from")
        .arg(&request.from);
    if !request.relaunch.is_empty() {
        command.arg("--");
        command.args(&request.relaunch);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Executa o modo oculto quando ele foi pedido. `None` = app normal.
pub fn apply_from_args(args: impl IntoIterator<Item = OsString>) -> Option<i32> {
    match parse_args(args) {
        Ok(None) => None,
        Ok(Some(request)) => Some(run(&request)),
        Err(error) => {
            eprintln!("mustel update: {error}");
            Some(2)
        }
    }
}

/// Aplica a troca, registra no log e relança o app (novo ou antigo).
pub fn run(request: &ApplyRequest) -> i32 {
    let result = replace_with_retry(&request.targets);
    log(&result, request);

    let (key, value) = match &result {
        Ok(()) => ("MUSTEL_UPDATED_FROM", request.from.clone()),
        Err(error) => ("MUSTEL_UPDATE_FAILED", error.clone()),
    };
    let relaunched = spawn_launch(&request.launch, &request.relaunch, key, &value);

    match (result.is_ok(), relaunched.is_ok()) {
        (true, true) => 0,
        _ => 1,
    }
}

/// Tenta a troca até o executável em uso liberar (Windows).
fn replace_with_retry(targets: &[ApplyTarget]) -> Result<(), String> {
    let deadline = Instant::now() + RETRY;
    loop {
        match replace_all(targets) {
            Ok(()) => return Ok(()),
            Err(error) if Instant::now() < deadline => {
                std::thread::sleep(RETRY_STEP);
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Faz o backup de todos os alvos e move os payloads; restaura tudo se algo falhar.
pub fn replace_all(targets: &[ApplyTarget]) -> Result<(), String> {
    for entry in targets {
        if !entry.target.is_file() {
            return Err(format!(
                "the executable to update does not exist: {}",
                entry.target.display()
            ));
        }
    }

    let mut backed_up: Vec<(&Path, PathBuf)> = Vec::new();
    for entry in targets {
        let backup = backup_path(&entry.target);
        let _ = std::fs::remove_file(&backup);
        if let Err(error) = std::fs::rename(&entry.target, &backup) {
            rollback(&backed_up);
            return Err(format!(
                "could not back up {}: {error}",
                entry.target.display()
            ));
        }
        backed_up.push((&entry.target, backup));
    }

    for entry in targets {
        if let Err(error) = std::fs::rename(&entry.payload, &entry.target) {
            rollback(&backed_up);
            return Err(format!(
                "could not replace {}: {error}",
                entry.target.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&entry.target, std::fs::Permissions::from_mode(0o755));
        }
    }

    Ok(())
}

/// `<exe>.old`.
fn backup_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(".old");
    PathBuf::from(name)
}

/// Desfaz os backups já feitos (do último para o primeiro).
fn rollback(backed_up: &[(&Path, PathBuf)]) {
    for (target, backup) in backed_up.iter().rev() {
        let _ = std::fs::remove_file(target);
        let _ = std::fs::rename(backup, target);
    }
}

/// Relança o app com o env de resultado.
fn spawn_launch(
    launch: &Path,
    relaunch: &[OsString],
    key: &str,
    value: &str,
) -> std::io::Result<()> {
    let mut command = Command::new(launch);
    command.args(relaunch).env(key, value);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map(|_| ())
}

/// Registra o apply no log de atividade (melhor esforço).
fn log(result: &Result<(), String>, request: &ApplyRequest) {
    let Some(store) = crate::activity::log_dir().map(Store::new) else {
        return;
    };
    let mut entry = Entry::new(
        Area::App,
        "app.update.apply",
        if result.is_ok() {
            Status::Ok
        } else {
            Status::Error
        },
    )
    .with_detail(format!("Mustel {}", request.from));
    if let Err(error) = result {
        entry = entry.with_error(error);
    }
    let _ = store.append(&entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_normal_app_has_no_apply_request() {
        assert_eq!(parse_args(args(&["gui"])), Ok(None));
        assert_eq!(parse_args(args(&[])), Ok(None));
    }

    #[test]
    fn pairs_launch_and_relaunch_survive_the_parse() {
        let parsed = parse_args(args(&[
            "--apply-update",
            "--target",
            "/tmp/mustel",
            "--payload",
            "/tmp/new mustel",
            "--target",
            "/tmp/mustel-gui.exe",
            "--payload",
            "/tmp/new gui",
            "--launch",
            "/tmp/mustel",
            "--from",
            "1.0.0",
            "--",
            "gui",
        ]))
        .expect("parse")
        .expect("é o modo apply");

        assert_eq!(parsed.targets.len(), 2);
        assert_eq!(parsed.targets[0].target, PathBuf::from("/tmp/mustel"));
        assert_eq!(parsed.targets[0].payload, PathBuf::from("/tmp/new mustel"));
        assert_eq!(parsed.launch, PathBuf::from("/tmp/mustel"));
        assert_eq!(parsed.from, "1.0.0");
        assert_eq!(parsed.relaunch, args(&["gui"]));
    }

    #[test]
    fn malformed_apply_arguments_are_refused() {
        assert!(parse_args(args(&["--apply-update"])).is_err());
        assert!(
            parse_args(args(&[
                "--apply-update",
                "--target",
                "/tmp/a",
                "--launch",
                "/tmp/a",
                "--from",
                "1.0.0"
            ]))
            .is_err(),
            "sem payload"
        );
        assert!(
            parse_args(args(&[
                "--apply-update",
                "--target",
                "/tmp/a",
                "--payload",
                "/tmp/b",
                "--launch",
                "/tmp/outro",
                "--from",
                "1.0.0"
            ]))
            .is_err(),
            "launch fora dos alvos"
        );
    }

    #[test]
    fn the_helper_command_carries_every_pair() {
        let request = ApplyRequest {
            targets: vec![
                ApplyTarget {
                    target: "/tmp/mustel".into(),
                    payload: "/tmp/new-mustel".into(),
                },
                ApplyTarget {
                    target: "/tmp/mustel-gui".into(),
                    payload: "/tmp/new-gui".into(),
                },
            ],
            launch: "/tmp/mustel".into(),
            from: "1.0.0".to_string(),
            relaunch: args(&["gui"]),
        };

        let command = helper_command(Path::new("/tmp/helper"), &request);
        let args: Vec<String> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();

        assert_eq!(command.get_program().to_string_lossy(), "/tmp/helper");
        assert_eq!(args[0], "--apply-update");
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--target", "/tmp/mustel"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--payload", "/tmp/new-gui"])
        );
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(args[args.len() - 1], "gui");
    }

    /// CONTRACT: IF-270
    #[test]
    fn replacing_keeps_a_backup_and_moves_the_payload() {
        let dir = tempfile::tempdir().expect("temporário");
        let target = dir.path().join("mustel");
        let payload = dir.path().join("new-mustel");
        std::fs::write(&target, b"old").expect("grava");
        std::fs::write(&payload, b"new").expect("grava");

        replace_all(&[ApplyTarget {
            target: target.clone(),
            payload,
        }])
        .expect("troca");

        assert_eq!(std::fs::read(&target).expect("lê"), b"new");
        assert_eq!(
            std::fs::read(backup_path(&target)).expect("lê o backup"),
            b"old"
        );
    }

    /// CONTRACT: IF-270
    #[test]
    fn a_failed_payload_move_restores_every_target() {
        let dir = tempfile::tempdir().expect("temporário");
        let first = dir.path().join("mustel");
        let second = dir.path().join("mustel-gui");
        let first_payload = dir.path().join("new-mustel");
        std::fs::write(&first, b"old-a").expect("grava");
        std::fs::write(&second, b"old-b").expect("grava");
        std::fs::write(&first_payload, b"new-a").expect("grava");

        let result = replace_all(&[
            ApplyTarget {
                target: first.clone(),
                payload: first_payload,
            },
            ApplyTarget {
                target: second.clone(),
                payload: dir.path().join("nao-existe"),
            },
        ]);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&first).expect("lê"), b"old-a");
        assert_eq!(std::fs::read(&second).expect("lê"), b"old-b");
    }
}
