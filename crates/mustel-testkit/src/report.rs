//! Geração do relatório de paridade em Markdown.

use std::fmt::Write as _;

use crate::compare::{CaseComparison, Level, MIGRATED_COMMANDS};

/// Conta quantos casos terminaram em cada nível.
#[derive(Debug, Default, Clone, Copy)]
pub struct Summary {
    /// Casos sem divergência de contrato.
    pub passed: usize,
    /// Casos com divergência aceita (N4/adição).
    pub warned: usize,
    /// Casos com violação de contrato (N1–N3).
    pub failed: usize,
    /// Casos não avaliados (stubs, texto não-UTF-8).
    pub skipped: usize,
}

impl Summary {
    /// Consolida o resultado de todos os casos.
    pub fn of(comparisons: &[CaseComparison]) -> Self {
        let mut summary = Self::default();
        for comparison in comparisons {
            match comparison.worst() {
                Level::Pass => summary.passed += 1,
                Level::Warn => summary.warned += 1,
                Level::Fail => summary.failed += 1,
                Level::Skipped => summary.skipped += 1,
            }
        }
        summary
    }

    /// Indica se o gate deve falhar.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }
}

/// Renderiza o relatório completo.
pub fn render(comparisons: &[CaseComparison], generated_at: &str) -> String {
    let summary = Summary::of(comparisons);
    let mut out = String::new();

    let _ = writeln!(out, "# Relatório de paridade — {generated_at}\n");
    let _ = writeln!(
        out,
        "passou: **{}** · avisos N4: {} · **falhas: {}** · pulados: {}\n",
        summary.passed, summary.warned, summary.failed, summary.skipped
    );

    if MIGRATED_COMMANDS.is_empty() {
        let _ = writeln!(
            out,
            "> Nenhum comando está marcado como migrado ainda: os casos de comportamento são pulados e\n> só a superfície (ajuda) e os erros determinísticos são comparados.\n"
        );
    }

    let _ = writeln!(out, "| caso | resultado | itens |");
    let _ = writeln!(out, "|------|-----------|-------|");
    for comparison in comparisons {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            comparison.slug,
            label(comparison.worst()),
            comparison.checks.len()
        );
    }

    let _ = writeln!(out, "\n## Detalhes\n");
    for comparison in comparisons {
        let _ = writeln!(
            out,
            "### `{}` — {}\n",
            comparison.slug,
            label(comparison.worst())
        );
        for check in &comparison.checks {
            let _ = writeln!(
                out,
                "- {} **{}**: {}",
                label(check.level),
                check.name,
                check.detail.replace('\n', "\n  ")
            );
        }
        let _ = writeln!(out);
    }

    out
}

fn label(level: Level) -> &'static str {
    match level {
        Level::Pass => "OK",
        Level::Warn => "N4 (aviso)",
        Level::Fail => "FALHA",
        Level::Skipped => "pulado",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::Check;

    fn comparison(slug: &str, level: Level) -> CaseComparison {
        CaseComparison {
            slug: slug.to_string(),
            checks: vec![Check {
                name: "exit code".to_string(),
                level,
                detail: "oráculo=0 mustel=0".to_string(),
            }],
        }
    }

    #[test]
    fn summary_counts_each_level() {
        let comparisons = vec![
            comparison("a", Level::Pass),
            comparison("b", Level::Warn),
            comparison("c", Level::Fail),
            comparison("d", Level::Skipped),
        ];
        let summary = Summary::of(&comparisons);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.warned, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert!(summary.has_failures());
    }

    #[test]
    fn render_includes_every_case() {
        let comparisons = vec![comparison("help-root", Level::Pass)];
        let markdown = render(&comparisons, "2026-10-15 00:00");
        assert!(markdown.contains("`help-root`"));
        assert!(markdown.contains("falhas: 0"));
    }
}
