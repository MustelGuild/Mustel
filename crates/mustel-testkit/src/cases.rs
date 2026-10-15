//! Casos capturados do oráculo.
//!
//! Só entram aqui casos **determinísticos**: ajuda de todos os comandos do escopo, erros de uso e
//! erros de arquivo ausente. Casos que dependem de Postgres/pg_dump ficam para os testes de
//! integração das tarefas de comando (T28/T29/T32+).

/// Um caso simples: slug + argumentos (+ observações).
pub struct Case {
    /// Identificador estável (vira `oracle/baseline/<slug>.json`).
    pub slug: &'static str,
    /// Argumentos passados ao binário.
    pub argv: &'static [&'static str],
    /// Observações registradas na captura.
    pub notes: &'static [&'static str],
}

/// Ajuda do binário e de cada grupo/subcomando do escopo.
pub const HELP_CASES: &[Case] = &[
    Case {
        slug: "help-root",
        argv: &["--help"],
        notes: &["formatação é N4 (não bloqueia)"],
    },
    Case {
        slug: "help-query",
        argv: &["query", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-query-run",
        argv: &["query", "run", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database",
        argv: &["database", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database-backup",
        argv: &["database", "backup", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database-restore",
        argv: &["database", "restore", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database-pgpass",
        argv: &["database", "pgpass", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database-pgpass-add",
        argv: &["database", "pgpass", "add", "--help"],
        notes: &[
            "o Mustel acrescenta a forma posicional `pgpass add <banco>` (decisão do usuário)",
        ],
    },
    Case {
        slug: "help-database-pgpass-list",
        argv: &["database", "pgpass", "list", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-database-pgpass-remove",
        argv: &["database", "pgpass", "remove", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings",
        argv: &["settings", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings-db-servers",
        argv: &["settings", "db-servers", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings-db-servers-ls",
        argv: &["settings", "db-servers", "ls", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings-db-servers-add",
        argv: &["settings", "db-servers", "add", "--help"],
        notes: &["o Mustel acrescenta `-i/--interactive` e `-W/--password` (decisão do usuário)"],
    },
    Case {
        slug: "help-settings-db-servers-rm",
        argv: &["settings", "db-servers", "rm", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings-db-servers-test",
        argv: &["settings", "db-servers", "test", "--help"],
        notes: &[],
    },
    Case {
        slug: "help-settings-db-servers-set-password",
        argv: &["settings", "db-servers", "set-password", "--help"],
        notes: &[],
    },
];

/// Erros de uso e de arquivo ausente — fixam exit codes determinísticos (N1/N2).
pub const ERROR_CASES: &[Case] = &[
    Case {
        slug: "error-unknown-command",
        argv: &["nao-existe"],
        notes: &["exit code de comando desconhecido"],
    },
    Case {
        slug: "error-unknown-option",
        argv: &["query", "run", "--nao-existe"],
        notes: &["exit code de opção desconhecida"],
    },
    Case {
        slug: "error-missing-required-argument",
        argv: &["database", "pgpass", "remove"],
        notes: &["posicional obrigatório ausente"],
    },
    Case {
        slug: "error-query-input-missing",
        argv: &["query", "run", "-i", "nao-existe.sql", "--no-confirm"],
        notes: &["arquivo de entrada inexistente"],
    },
    Case {
        slug: "error-database-restore-file-missing",
        argv: &["database", "restore", "mydb", "nao-existe.dump"],
        notes: &["arquivo de dump inexistente"],
    },
    Case {
        slug: "error-db-servers-rm-unknown",
        argv: &["settings", "db-servers", "rm", "-n", "nao-existe"],
        notes: &["servidor inexistente na configuração"],
    },
    Case {
        slug: "error-db-servers-test-offline",
        argv: &["settings", "db-servers", "test", "-n", "offline"],
        notes: &[
            "conexão recusada em 127.0.0.1:1 — mensagem depende do SO, comparar com tolerância",
        ],
    },
];
