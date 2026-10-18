//! Execução de um processo externo, com captura de saída e cancelamento.
//!
//! **Sem shell:** o programa e cada argumento vão como itens separados de [`ProcessOptions::args`],
//! como o `ArgumentList` do legado (`UseShellExecute = false`). Nada é interpretado por `sh`/`cmd`.
//!
//! **Sem janela:** no Windows não pedimos console novo, então o filho usa o console do próprio CLI —
//! o equivalente ao `CreateNoWindow = true` do legado.
//!
//! **Cancelamento:** quando o token é cancelado, a **árvore** do processo é morta e o retorno é
//! [`ProcessError::Cancelled`] (o CLI traduz para exit 130). No Windows isso é `taskkill /T /F`; no
//! Unix o filho abre o próprio grupo de processos e o grupo inteiro leva `SIGKILL`. O utilitário
//! `kill` é usado em vez de `libc::kill` porque o workspace tem `unsafe_code = "forbid"`.

use std::io;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::AsyncReadExt;
use tokio::process::Child;

use crate::signal::CancellationToken;

/// O que executar, como e com qual ambiente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOptions {
    /// Programa a executar (caminho absoluto, quando o localizador achou um).
    pub program: PathBuf,
    /// Argumentos, **cada um como um item** — nada é passado por shell.
    pub args: Vec<String>,
    /// Diretório de trabalho; `None` herda o do CLI.
    pub working_dir: Option<PathBuf>,
    /// Variáveis de ambiente acrescentadas às herdadas (ex.: `PGPASSWORD`).
    pub env: Vec<(String, String)>,
}

impl ProcessOptions {
    /// Opções para um programa, sem argumentos, ambiente nem diretório próprios.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_dir: None,
            env: Vec::new(),
        }
    }

    /// Acrescenta argumentos.
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Acrescenta uma variável de ambiente.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Define o diretório de trabalho.
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }
}

/// Resultado de uma execução que **concluiu** (mesmos campos do `ProcessExecutionResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutcome {
    /// Código de saída do processo.
    pub exit_code: i32,
    /// stdout capturado, decodificado como UTF-8 (lossy).
    pub stdout: String,
    /// stderr capturado, decodificado como UTF-8 (lossy).
    pub stderr: String,
}

impl ProcessOutcome {
    /// O processo terminou bem (`Success` do legado).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Falha ao executar o processo.
///
/// O legado embrulhava tudo no resultado (`ExitCode = -1` e o texto em `stderr`); aqui a diferença
/// fica no **tipo**, para o comando decidir o exit code (o backup, por exemplo, devolve 1 para
/// falha de execução e 3 quando o binário não existe).
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// O processo não pôde ser iniciado (binário ausente, sem permissão, ...).
    #[error("failed to start {program}: {source}")]
    Spawn {
        /// Programa que se tentou executar.
        program: String,
        /// Erro do sistema operacional.
        source: io::Error,
    },
    /// Falha de E/S ao esperar ou ler a saída do processo.
    #[error("{program} failed: {source}")]
    Io {
        /// Programa em execução.
        program: String,
        /// Erro do sistema operacional.
        source: io::Error,
    },
    /// Cancelado pelo usuário: a árvore de processos foi morta.
    #[error("operation cancelled")]
    Cancelled,
}

/// Executa o processo até o fim, capturando `stdout` e `stderr`.
pub async fn run(
    options: &ProcessOptions,
    token: &CancellationToken,
) -> Result<ProcessOutcome, ProcessError> {
    let program = options.program.display().to_string();

    let mut command = tokio::process::Command::new(&options.program);
    command
        .args(&options.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(dir) = &options.working_dir {
        command.current_dir(dir);
    }
    for (key, value) in &options.env {
        command.env(key, value);
    }
    spawn_in_own_group(&mut command);

    let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
        program: program.clone(),
        source,
    })?;

    // Os dois canos são drenados **em paralelo**: ler um até o fim antes do outro trava quando o
    // processo enche o buffer do que ninguém está lendo (foi o que o legado fez).
    let stdout_task = child.stdout.take().map(spawn_reader);
    let stderr_task = child.stderr.take().map(spawn_reader);

    let status = tokio::select! {
        status = child.wait() => status,
        () = token.cancelled() => {
            kill_tree(&mut child).await;
            return Err(ProcessError::Cancelled);
        }
    };

    let status = status.map_err(|source| ProcessError::Io {
        program: program.clone(),
        source,
    })?;
    let stdout = collect(stdout_task, &program).await?;
    let stderr = collect(stderr_task, &program).await?;

    Ok(ProcessOutcome {
        exit_code: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

/// Lê um cano até o fim, em uma task própria.
fn spawn_reader(
    mut pipe: impl AsyncReadExt + Unpin + Send + 'static,
) -> tokio::task::JoinHandle<io::Result<Vec<u8>>> {
    tokio::spawn(async move {
        let mut buffer = Vec::new();
        pipe.read_to_end(&mut buffer).await?;
        Ok(buffer)
    })
}

/// Espera a leitura e decodifica (bytes inválidos viram `U+FFFD`, como o `ReadToEnd` do .NET).
async fn collect(
    task: Option<tokio::task::JoinHandle<io::Result<Vec<u8>>>>,
    program: &str,
) -> Result<String, ProcessError> {
    let Some(task) = task else {
        return Ok(String::new());
    };

    let bytes = task
        .await
        .map_err(|error| ProcessError::Io {
            program: program.to_string(),
            source: io::Error::other(error.to_string()),
        })?
        .map_err(|source| ProcessError::Io {
            program: program.to_string(),
            source,
        })?;

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// No Unix o filho abre o próprio grupo de processos, para o cancelamento derrubar a árvore.
#[cfg(unix)]
fn spawn_in_own_group(command: &mut tokio::process::Command) {
    command.process_group(0);
}

/// No Windows o grupo não é a unidade de kill: `taskkill /T` derruba a árvore pelo pid.
#[cfg(not(unix))]
fn spawn_in_own_group(_command: &mut tokio::process::Command) {}

/// Mata a árvore do processo, sem deixar saída do utilitário vazar para o nosso stdout.
async fn kill_tree(child: &mut Child) {
    if let Some(pid) = child.id() {
        kill_group(pid);
    }

    // Cinto e suspensório: garante que o filho direto também morra.
    let _ = child.kill().await;
}

/// No Windows a árvore é derrubada por `taskkill /T`, e a saída dele não pode aparecer na nossa.
#[cfg(windows)]
fn kill_group(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// No Unix o filho abre o próprio grupo (`process_group(0)`), então `killpg` derruba a árvore
/// inteira — a syscall direta, sem depender do parsing de pid negativo do utilitário `kill`.
#[cfg(unix)]
fn kill_group(pid: u32) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    // O grupo é o do filho, que virou líder (`process_group(0)` no momento do spawn).
    if let Ok(pid) = i32::try_from(pid) {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }
}

/// Em outros SOs não há grupo para derrubar: resta o `child.kill()` de `kill_tree`.
#[cfg(not(any(unix, windows)))]
fn kill_group(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::cancellation_token;

    /// Programa que roda um trecho de script, no SO atual.
    ///
    /// No Windows é o **PowerShell** (e não o `cmd`): o `cmd` redirecionado se comporta de forma
    /// imprevisível — `cmd /c "echo erro 1>&2"` com stdout redirecionado fica esperando entrada —, e
    /// além disso o script entra como **um** argumento, sem reinterpretação de shell.
    #[cfg(windows)]
    fn script(_unix: &str, windows: &str) -> ProcessOptions {
        ProcessOptions::new("powershell").with_args(["-NoProfile", "-Command", windows])
    }

    #[cfg(unix)]
    fn script(unix: &str, _windows: &str) -> ProcessOptions {
        ProcessOptions::new("sh").with_args(["-c", unix])
    }

    /// Programa longo o suficiente para dar tempo de cancelar.
    fn sleeper() -> ProcessOptions {
        #[cfg(windows)]
        {
            // `timeout` recusa entrada redirecionada; `ping` não.
            ProcessOptions::new("ping").with_args(["-n", "30", "127.0.0.1"])
        }
        #[cfg(unix)]
        {
            ProcessOptions::new("sleep").with_args(["30"])
        }
    }

    #[tokio::test]
    async fn captures_stdout_stderr_and_the_exit_code() {
        let token = cancellation_token();

        let outcome = run(
            &script("echo ola", "[Console]::Out.WriteLine('ola')"),
            &token,
        )
        .await
        .expect("executa");
        assert!(outcome.success(), "exit code {}", outcome.exit_code);
        assert_eq!(outcome.stdout.trim_end(), "ola");
        assert_eq!(outcome.stderr, "");

        let outcome = run(&script("exit 7", "exit 7"), &token)
            .await
            .expect("executa");
        assert_eq!(outcome.exit_code, 7);
        assert!(!outcome.success());

        let outcome = run(
            &script("echo erro >&2", "[Console]::Error.WriteLine('erro')"),
            &token,
        )
        .await
        .expect("executa");
        assert!(
            outcome.stderr.contains("erro"),
            "stderr: {:?}",
            outcome.stderr
        );
    }

    #[tokio::test]
    async fn passes_environment_variables() {
        let options = script(
            "echo $MUSTEL_TESTE",
            "[Console]::Out.WriteLine($env:MUSTEL_TESTE)",
        )
        .with_env("MUSTEL_TESTE", "valor");

        let outcome = run(&options, &cancellation_token()).await.expect("executa");
        assert_eq!(outcome.stdout.trim_end(), "valor");
    }

    #[tokio::test]
    async fn uses_the_working_directory() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        // `canonicalize` no Windows devolve o prefixo de caminho estendido (`\\\\?\\`), que nenhum
        // programa imprime: comparamos sem ele.
        let expected = dir
            .path()
            .canonicalize()
            .expect("caminho real")
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/");

        let options = script("pwd", "[Console]::Out.WriteLine((Get-Location).Path)")
            .with_working_dir(dir.path());

        let outcome = run(&options, &cancellation_token()).await.expect("executa");
        assert_eq!(outcome.stdout.trim_end().replace('\\', "/"), expected);
    }

    #[tokio::test]
    async fn a_missing_program_is_a_spawn_error() {
        let options = ProcessOptions::new("mustel-programa-que-nao-existe");

        let error = run(&options, &cancellation_token())
            .await
            .expect_err("não pode iniciar");

        assert!(matches!(error, ProcessError::Spawn { .. }), "{error:?}");
        assert!(error.to_string().contains("mustel-programa-que-nao-existe"));
    }

    #[tokio::test]
    async fn cancellation_kills_the_child_and_reports_cancelled() {
        let token = cancellation_token();
        let cancel = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            cancel.cancel();
        });

        let started = std::time::Instant::now();
        let error = run(&sleeper(), &token).await.expect_err("cancelado");

        assert!(matches!(error, ProcessError::Cancelled), "{error:?}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "o cancelamento não pode esperar o processo terminar: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn a_large_amount_of_output_does_not_deadlock() {
        // Enche os dois canos ao mesmo tempo: ler um até o fim antes do outro travaria aqui.
        let options = script(
            "i=0; while [ $i -lt 40000 ]; do echo linha; echo erro >&2; i=$((i+1)); done",
            "1..40000 | % { [Console]::Out.WriteLine('linha'); [Console]::Error.WriteLine('erro') }",
        );

        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            run(&options, &cancellation_token()),
        )
        .await
        .expect("não pode travar")
        .expect("executa");

        assert_eq!(outcome.stdout.matches("linha").count(), 40000);
        assert_eq!(outcome.stderr.matches("erro").count(), 40000);
    }

    /// Os argumentos chegam **literais**: se houvesse shell no meio, `$(...)` e `;` seriam
    /// interpretados. Só no Unix porque o Windows não tem um `echo` que não seja o do `cmd`.
    #[cfg(unix)]
    #[tokio::test]
    async fn arguments_are_not_interpreted_by_a_shell() {
        let hostile = "$(echo invadido); rm -rf /";
        let options = ProcessOptions::new("echo").with_args([hostile]);

        let outcome = run(&options, &cancellation_token()).await.expect("executa");
        assert_eq!(outcome.stdout.trim_end(), hostile);
    }

    /// O cancelamento derruba a **árvore** do lado Unix: o neto que o filho pôs em segundo plano
    /// (`(sleep 2; touch arquivo) &`) morre junto com o grupo.
    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_the_whole_process_group() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let marker = dir.path().join("sobreviveu.txt");
        let script_text = format!("(sleep 2; touch '{}') & sleep 30", marker.display());
        let options = ProcessOptions::new("sh").with_args(["-c", &script_text]);

        let token = cancellation_token();
        let cancel = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            cancel.cancel();
        });

        assert!(matches!(
            run(&options, &token).await.expect_err("cancelado"),
            ProcessError::Cancelled
        ));

        // Espera mais do que o neto levaria para tocar o arquivo.
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        assert!(
            !marker.exists(),
            "o neto sobreviveu ao cancelamento: {}",
            marker.display()
        );
    }

    /// O cancelamento derruba a **árvore** também no Windows: o neto que o filho criou (`ping`) tem
    /// de morrer junto. Só no Windows, porque o teste depende do `taskkill /T` e do `tasklist`.
    #[cfg(windows)]
    #[tokio::test]
    async fn cancellation_kills_the_whole_process_tree() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_file = dir.path().join("neto.pid");
        let script_text = format!(
            "$p = Start-Process -PassThru -NoNewWindow -FilePath ping -ArgumentList '-n','30','127.0.0.1'; \
             $p.Id | Set-Content -Path '{}'; Start-Sleep -Seconds 30",
            pid_file.display()
        );
        let options =
            ProcessOptions::new("powershell").with_args(["-NoProfile", "-Command", &script_text]);

        let token = cancellation_token();
        let cancel = token.clone();
        let handle = tokio::spawn(async move { run(&options, &token).await });

        // O filho registra o pid do neto antes de dormir; sem ele não dá para checar depois.
        let mut neto = None;
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse::<u32>()
            {
                neto = Some(pid);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let neto = neto.expect("o filho precisa registrar o pid do neto");

        cancel.cancel();
        assert!(matches!(
            handle.await.expect("task").expect_err("cancelado"),
            ProcessError::Cancelled
        ));

        // Dá tempo do `taskkill /T` agir e confirma que o neto morreu.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        assert!(
            !process_is_alive(neto),
            "o neto {neto} sobreviveu ao cancelamento"
        );
    }

    /// Um pid continua vivo? (No Windows, quem responde isso é o `tasklist`.)
    #[cfg(windows)]
    fn process_is_alive(pid: u32) -> bool {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist roda");

        String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
    }
}
