//! Casos capturados do oráculo.
//!
//! Só entram aqui casos **determinísticos**: ajuda de todos os comandos do escopo, erros de uso e
//! erros de arquivo ausente. Casos que dependem de Postgres/pg_dump ficam para os testes de
//! integração das tarefas de comando (T28/T29/T32+).

use crate::capture::{ConfigSeed, PgPassSeed};

/// Um caso: slug + argumentos + observações + estado inicial dos arquivos do usuário.
pub struct Case {
    /// Identificador estável (vira `oracle/baseline/<slug>.json`).
    pub slug: &'static str,
    /// Argumentos passados ao binário.
    pub argv: &'static [&'static str],
    /// Observações registradas na captura.
    pub notes: &'static [&'static str],
    /// Estado inicial da configuração do usuário.
    pub seed: ConfigSeed,
    /// Estado inicial do `pgpass.conf` do usuário.
    pub pgpass_seed: PgPassSeed,
}

const fn help(slug: &'static str, argv: &'static [&'static str]) -> Case {
    Case {
        slug,
        argv,
        notes: &[],
        seed: ConfigSeed::Absent,
        pgpass_seed: PgPassSeed::Unchanged,
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
        pgpass_seed: PgPassSeed::Unchanged,
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
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-unknown-option",
            argv: &["query", "run", "--nao-existe"],
            notes: &["exit code de opção desconhecida"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-missing-required-argument",
            argv: &["database", "pgpass", "remove"],
            notes: &["posicional obrigatório ausente"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-query-input-missing",
            argv: &["query", "run", "-i", "nao-existe.sql", "--no-confirm"],
            notes: &["arquivo de entrada inexistente (exit 1, exceção própria do legado)"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-database-restore-file-missing",
            argv: &["database", "restore", "mydb", "nao-existe.dump"],
            notes: &[
                "dump ausente: o comando devolve 2 (FurLabFileNotFoundException), não 1",
                "com config real presente o caminho muda (pedia senha antes) — por isso todo caso fixa a config",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-db-servers-rm-unknown",
            argv: &["settings", "db-servers", "rm", "-n", "nao-existe"],
            notes: &["servidor inexistente na configuração"],
            seed: ConfigSeed::Fixture,
            pgpass_seed: PgPassSeed::Unchanged,
        },
        Case {
            slug: "error-db-servers-test-offline",
            argv: &["settings", "db-servers", "test", "-n", "offline"],
            notes: &[
                "senha ausente em modo não-interativo: o legado quebra (exit 40); o Mustel divergirá de propósito",
            ],
            seed: ConfigSeed::Fixture,
            pgpass_seed: PgPassSeed::Unchanged,
        },
    ]
}

/// Casos de `database pgpass` (T23).
///
/// Todos usam `--database`: a forma posicional é **adição do Mustel** (o legado responde
/// `Error: Unknown command 'mydb'.`), então ela não pode aparecer num argv que roda nos dois
/// binários. Todo caso aqui semeia o `pgpass.conf` — nenhum pode ler o arquivo real do usuário, que
/// tem credenciais de verdade.
///
/// Os três primeiros são de **validação**: o exit code (2) vem do corpo do comando legado, e o
/// artefato anexado prova que a recusa acontece **antes** de o arquivo existir.
pub fn pgpass_cases() -> Vec<Case> {
    vec![
        Case {
            slug: "error-pgpass-add-missing-database",
            argv: &["database", "pgpass", "add"],
            notes: &[
                "sem `--database`: o legado valida o banco antes da senha, então não chega a pedir senha",
                "exit 2 vem do corpo do comando, não do parser (que sai com 1)",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "error-pgpass-add-invalid-host",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "mydb",
                "--host",
                "host_com_underscore",
                "--password",
                "senha",
            ],
            notes: &["host recusado por `SecurityUtils.IsValidHost` (o curinga `*` passa)"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "error-pgpass-add-invalid-port",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "mydb",
                "--port",
                "70000",
                "--password",
                "senha",
            ],
            notes: &["porta fora de 1–65535; o curinga `*` passa"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "database-pgpass-add-new",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "mydb",
                "--password",
                "senha123",
            ],
            notes: &["arquivo ausente: o comando cria o diretório e o arquivo"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "database-pgpass-add-duplicate",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "mydb",
                "--password",
                "outra_senha",
            ],
            notes: &[
                "identidade já no arquivo: exit 0, mensagem `Entry already exists` e arquivo intacto",
                "a senha não faz parte da identidade",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Fixture,
        },
        Case {
            slug: "database-pgpass-add-wildcards",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "*",
                "--host",
                "*",
                "--port",
                "*",
                "--username",
                "*",
                "--password",
                "senhapadrao",
            ],
            notes: &["curinga em host, porta, banco e usuário (spec `pgpass-wildcard-validation`)"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "database-pgpass-add-appends-to-a-file-without-final-newline",
            argv: &[
                "database",
                "pgpass",
                "add",
                "--database",
                "novo",
                "--password",
                "senha_nova",
            ],
            notes: &[
                "arquivo sem quebra de linha no fim: a entrada nova entra **colada** na última",
                "comportamento do `File.AppendAllText` — paridade mesmo onde é feio",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::NoTrailingNewline,
        },
        Case {
            slug: "database-pgpass-list-seeded",
            argv: &["database", "pgpass", "list"],
            notes: &[
                "tabela é N4 (codepage do console); o comparador confere o conteúdo por tokens ASCII",
                "o comentário do fixture não pode aparecer na listagem",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Fixture,
        },
        Case {
            slug: "database-pgpass-list-empty",
            argv: &["database", "pgpass", "list"],
            notes: &["arquivo ausente: `No entries configured in pgpass.conf.` com exit 0"],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Absent,
        },
        Case {
            slug: "database-pgpass-remove-ok",
            argv: &[
                "database",
                "pgpass",
                "remove",
                "app_dev",
                "--host",
                "db.interno",
                "--port",
                "5433",
                "--username",
                "deploy",
            ],
            notes: &[
                "a chave é host:porta:banco:usuário — remover pelo banco sozinho não acharia",
                "o comentário do fixture tem de sobreviver à reescrita",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Fixture,
        },
        Case {
            slug: "database-pgpass-remove-not-found",
            argv: &["database", "pgpass", "remove", "nao-existe"],
            notes: &[
                "entrada inexistente → `Entry not found: ...` e exit **0** (não é erro) e arquivo intacto",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Fixture,
        },
        Case {
            slug: "database-pgpass-remove-wildcard-host",
            argv: &["database", "pgpass", "remove", "mydb", "--host", "*"],
            notes: &[
                "cenário da spec `pgpass-cli-setup`: `--host \"*\"` remove a entrada curinga `*:5432:mydb:postgres`",
                "o `remove` não valida host: o curinga entra na chave como texto",
            ],
            seed: ConfigSeed::Absent,
            pgpass_seed: PgPassSeed::Wildcard,
        },
    ]
}
