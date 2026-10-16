//! Casos capturados do oráculo.
//!
//! Só entram aqui casos **determinísticos**: ajuda de todos os comandos do escopo, erros de uso e
//! erros de arquivo ausente. Casos que dependem de Postgres/pg_dump ficam para os testes de
//! integração das tarefas de comando (T28/T29/T32+).

use crate::capture::ConfigSeed;

/// Um caso: slug + argumentos + observações + estado inicial da configuração.
pub struct Case {
    /// Identificador estável (vira `oracle/baseline/<slug>.json`).
    pub slug: &'static str,
    /// Argumentos passados ao binário.
    pub argv: &'static [&'static str],
    /// Observações registradas na captura.
    pub notes: &'static [&'static str],
    /// Estado inicial da configuração do usuário.
    pub seed: ConfigSeed,
}

const fn help(slug: &'static str, argv: &'static [&'static str]) -> Case {
    Case {
        slug,
        argv,
        notes: &[],
        seed: ConfigSeed::Absent,
    }
}

const fn help_with_notes(
    slug: &'static str,
    argv: &'static [&'static str],
    notes: &'static [&'static str],
) -> Case {
    Case {
        slug,
        argv,
        notes,
        seed: ConfigSeed::Absent,
    }
}

/// Ajuda do binário e de cada grupo/subcomando do escopo.
pub fn help_cases() -> Vec<Case> {
    vec![
        help_with_notes(
            "help-root",
            &["--help"],
            &["formatação é N4 (não bloqueia)"],
        ),
        help("help-query", &["query", "--help"]),
        help("help-query-run", &["query", "run", "--help"]),
        help("help-database", &["database", "--help"]),
        help("help-database-backup", &["database", "backup", "--help"]),
        help("help-database-restore", &["database", "restore", "--help"]),
        help("help-database-pgpass", &["database", "pgpass", "--help"]),
        help_with_notes(
            "help-database-pgpass-add",
            &["database", "pgpass", "add", "--help"],
            &["o Mustel acrescenta a forma posicional `pgpass add <banco>` (decisão do usuário)"],
        ),
        help(
            "help-database-pgpass-list",
            &["database", "pgpass", "list", "--help"],
        ),
        help(
            "help-database-pgpass-remove",
            &["database", "pgpass", "remove", "--help"],
        ),
        help("help-settings", &["settings", "--help"]),
        help(
            "help-settings-db-servers",
            &["settings", "db-servers", "--help"],
        ),
        help(
            "help-settings-db-servers-ls",
            &["settings", "db-servers", "ls", "--help"],
        ),
        help_with_notes(
            "help-settings-db-servers-add",
            &["settings", "db-servers", "add", "--help"],
            &["o Mustel acrescenta `-i/--interactive` e `-W/--password` (decisão do usuário)"],
        ),
        help(
            "help-settings-db-servers-rm",
            &["settings", "db-servers", "rm", "--help"],
        ),
        help(
            "help-settings-db-servers-test",
            &["settings", "db-servers", "test", "--help"],
        ),
        help(
            "help-settings-db-servers-set-password",
            &["settings", "db-servers", "set-password", "--help"],
        ),
    ]
}

/// Erros de uso e de arquivo ausente — fixam exit codes determinísticos (N1/N2).
pub fn error_cases() -> Vec<Case> {
    vec![
        Case {
            slug: "error-unknown-command",
            argv: &["nao-existe"],
            notes: &["exit code de comando desconhecido"],
            seed: ConfigSeed::Absent,
        },
        Case {
            slug: "error-unknown-option",
            argv: &["query", "run", "--nao-existe"],
            notes: &["exit code de opção desconhecida"],
            seed: ConfigSeed::Absent,
        },
        Case {
            slug: "error-missing-required-argument",
            argv: &["database", "pgpass", "remove"],
            notes: &["posicional obrigatório ausente"],
            seed: ConfigSeed::Absent,
        },
        Case {
            slug: "error-query-input-missing",
            argv: &["query", "run", "-i", "nao-existe.sql", "--no-confirm"],
            notes: &["arquivo de entrada inexistente (exit 1, exceção própria do legado)"],
            seed: ConfigSeed::Absent,
        },
        Case {
            slug: "error-database-restore-file-missing",
            argv: &["database", "restore", "mydb", "nao-existe.dump"],
            notes: &[
                "dump ausente: o comando devolve 2 (FurLabFileNotFoundException), não 1",
                "com config real presente o caminho muda (pedia senha antes) — por isso todo caso fixa a config",
            ],
            seed: ConfigSeed::Absent,
        },
        Case {
            slug: "error-db-servers-rm-unknown",
            argv: &["settings", "db-servers", "rm", "-n", "nao-existe"],
            notes: &["servidor inexistente na configuração"],
            seed: ConfigSeed::Fixture,
        },
        Case {
            slug: "error-db-servers-test-offline",
            argv: &["settings", "db-servers", "test", "-n", "offline"],
            notes: &[
                "senha ausente em modo não-interativo: o legado quebra (exit 40); o Mustel divergirá de propósito",
            ],
            seed: ConfigSeed::Fixture,
        },
    ]
}
