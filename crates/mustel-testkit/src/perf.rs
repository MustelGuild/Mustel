//! Medição de tempo de execução para o baseline de performance.
//!
//! Substitui o `hyperfine` (não instalado na máquina da migração e mais uma dependência externa do que
//! o necessário): mede N execuções completas do processo e resume mediana/mínimo/máximo. Continua
//! sendo tempo de parede de processo — inclui partida do runtime, que é justamente o que se quer
//! comparar.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Resultado de uma medição.
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Rótulo da medição.
    pub label: String,
    /// Argumentos usados.
    pub argv: Vec<String>,
    /// Quantidade de execuções.
    pub runs: usize,
    /// Mediana em milissegundos.
    pub median_ms: f64,
    /// Menor tempo observado em milissegundos.
    pub min_ms: f64,
    /// Maior tempo observado em milissegundos.
    pub max_ms: f64,
    /// Exit codes diferentes de zero observados.
    pub failures: usize,
    /// Observações.
    pub notes: Vec<String>,
}

impl Measurement {
    /// Mediana formatada para o relatório.
    pub fn median(&self) -> String {
        format!("{:.1} ms", self.median_ms)
    }
}

/// Executa `runs` vezes e resume os tempos.
pub fn measure(
    label: &str,
    binary: &Path,
    cwd: &Path,
    argv: &[&str],
    env: &[(&str, &str)],
    runs: usize,
    notes: &[&str],
) -> Measurement {
    let mut durations = Vec::with_capacity(runs);
    let mut failures = 0usize;

    for _ in 0..runs {
        let started = Instant::now();
        let mut command = Command::new(binary);
        command.args(argv).current_dir(cwd);
        for (key, value) in env {
            command.env(key, value);
        }

        match command.output() {
            Ok(output) => {
                if !output.status.success() {
                    failures += 1;
                }
                durations.push(started.elapsed());
            }
            Err(_) => failures += 1,
        }
    }

    durations.sort();
    let to_ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
    let median_ms = if durations.is_empty() {
        f64::NAN
    } else {
        to_ms(durations[durations.len() / 2])
    };

    Measurement {
        label: label.to_string(),
        argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
        runs,
        median_ms,
        min_ms: durations.first().map(|d| to_ms(*d)).unwrap_or(f64::NAN),
        max_ms: durations.last().map(|d| to_ms(*d)).unwrap_or(f64::NAN),
        failures,
        notes: notes.iter().map(|note| (*note).to_string()).collect(),
    }
}

/// Tamanho de um arquivo em bytes, quando existe.
pub fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|metadata| metadata.len())
}

/// Tamanho legível (KB/MB).
pub fn human_size(bytes: u64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        format!("{kb:.0} KB")
    } else {
        format!("{:.1} MB", kb / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_uses_the_right_unit() {
        assert_eq!(human_size(512 * 1024), "512 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn file_size_is_none_for_missing_paths() {
        assert_eq!(file_size(Path::new("/definitely/not/here")), None);
    }
}
