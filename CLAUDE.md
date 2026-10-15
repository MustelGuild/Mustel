# Mustel — Guia de Desenvolvimento

> Reescrito durante a migração para Rust (branch `feat/rust-migration`). O plano que rege a migração é
> `MustelPlans/migracao-rust.md` — ele é a fonte de verdade de escopo, decisões e tarefas.

## Idioma

Todo output para o usuário (explicações, resumos, mensagens de erro de ferramentas, perguntas) deve ser
em **português do Brasil (pt-BR)**. Exceção: não traduza nomes de skills, comandos do OpenSpec,
identificadores de código, nomes de arquivos ou comandos de terminal.

Todas as **mensagens exibidas ao usuário pelo CLI** (output de comandos, prompts interativos, erros,
confirmações) são escritas em **inglês** — é o contrato de paridade com o FurLab (níveis N1–N3 da
escada de fidelidade). O binário fala inglês; a documentação, o plano e a conversa com o usuário do
repositório falam pt-BR.

## Estado do repositório

| Área | O que é |
|------|---------|
| `crates/` | **Mustel** (Rust) — o produto ativo |
| `legacy/` | **FurLab** (.NET 10) — **congelado**: produto dos grupos fora do escopo e oráculo de paridade |
| `oracle/` | baseline, relatórios de divergência e números de performance |
| `MustelPlans/` | planos de implementação (fonte de verdade do trabalho em andamento) |
| `openspec/` | specs canônicas (pt-BR) — contrato comportamental herdado |
| `docs/` | documentação pt-BR (primária) e en (secundária) |

Escopo da migração: grupos **`query`**, **`database`** e **`settings`** completos, mais a GUI em egui.
Os demais grupos (`file`, `claude`, `opencode`, `winget`, `windowsfeatures`, `docker`) permanecem no
`legacy/` até a segunda onda.

## Stack

- Rust 1.98 (pinado em `rust-toolchain.toml`), edition 2024, resolver 3
- CLI: `clap` (derive) · prompts `inquire` · tabelas `tabled` · progresso `indicatif` · estilo `console` + `owo-colors`
- Async: `tokio` (+ `tokio-util` para `CancellationToken`) — **somente para I/O**
- Postgres: driver definido no spike T19 · dados exportados com `csv`
- Config: `serde` + `jsonc-parser`, caminho resolvido por `dirs` · credenciais: `keyring`
- Erros: `thiserror` no core, `anyhow` apenas no binário · logs: `tracing`
- GUI: `eframe`/`egui` (+ `egui_extras`), atrás da feature `gui`
- Testes: `assert_cmd` + `predicates` + `insta` (CLI), `expectrl` (PTY), `tempfile`, `testcontainers` (Postgres)

## Estrutura

```text
crates/
├── mustel-core/     lib: config, credentials, postgres, pgpass, casos de uso, eventos (sem UI)
├── mustel/          bin `mustel`: clap, UI de terminal, comandos finos
├── mustel-gui/      lib egui usada pelo subcomando `gui`
└── mustel-testkit/  harness de paridade (bins oracle-capture / oracle-compare; não publicado)
legacy/              FurLab .NET (congelado)
oracle/              baseline/, reports/, perf/, known-divergences.md
```

## Comandos

```bash
# Rust (na raiz do repositório)
cargo build --workspace
cargo test --workspace
cargo fmt --all                              # CI roda com --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p mustel -- --help
./target/release/mustel --version

# Paridade com o legado (oráculo)
cargo run -p mustel-testkit --bin oracle-capture
cargo run -p mustel-testkit --bin oracle-compare -- --report --scope query,database,settings

# Legado (congelado) — apenas para manter o oráculo vivo
dotnet build legacy/FurLab.slnx
dotnet test  legacy/FurLab.slnx
```

## Convenções de código

- **Formatação**: `cargo fmt` (default do rustfmt, 100 colunas). O CI reprova qualquer diff.
- **Lint**: `clippy -D warnings`; `unsafe_code = "forbid"` no workspace.
- **Sem UI no core**: `mustel-core` não depende de `clap`, `inquire`, `indicatif` nem `egui`
  (`cargo tree -p mustel-core` deve ficar limpo).
- **Comandos finos**: o comando parseia argumentos, resolve entradas (flags + prompts) e chama o caso de
  uso do core. Nenhuma regra de negócio dentro do comando.
- **Erros**: `CoreError` (thiserror) no core; o CLI traduz para exit code e mensagem. Nunca
  `unwrap()`/`expect()` em caminho de execução — apenas em teste.
- **Processos externos**: argumentos sempre em `Vec<String>`; nunca shell, nunca string interpolada;
  stdout/stderr capturados; árvore de processos morta no cancelamento.
- **Credenciais**: sempre `keyring`; nenhum segredo em arquivo, log ou mensagem.
- **Async apenas para I/O**; CPU puro é síncrono (e roda em worker quando chamado pela GUI).
- **Documentação**: `//!` no módulo e `///` em itens públicos, em pt-BR; mensagens de CLI em inglês.
- **Um módulo por arquivo**, tipos agrupados por domínio (evitar `types.rs` genérico).

## Paridade (o contrato do projeto)

A fonte de verdade comportamental é o **binário legado**, não a documentação: já houve divergência entre
docs e código (ex.: `-i/--input` no código vs `-f/--servers` nos docs; exit code `41` no código vs `2`
nos docs). Numa reescrita, "documentado" não é evidência — "observado" é.

Escada de fidelidade (detalhe na seção 4.4 do plano):

| Nível | Exige | Bloqueia? |
|-------|-------|-----------|
| N1 | mesma árvore de comandos, flags, defaults; inválidos rejeitados do mesmo jeito | sim |
| N2 | mesmos artefatos byte a byte, mesmos exit codes, permissões e mtime preservados | sim |
| N3 | mesmos fluxos interativos (perguntas, ordem, atalhos, defaults) | sim |
| N4 | formatação textual (`--help`, cores, tabelas, redação de erros) | não — registrar em `oracle/known-divergences.md` |

Nenhum comando é considerado pronto sem `oracle-compare` verde nos níveis N1–N3.

## Exit codes (herdados do oráculo)

`0` sucesso · `1` erro geral · `10`/`11` Postgres (com/sem SQLSTATE) · `20`–`22` I/O · `30` acesso ·
`40`/`41` estado inválido / argumento inválido · `50` timeout · `130` cancelado pelo usuário.

## Legado .NET (congelado)

- **Não adicionar features** ao legado. Mudanças permitidas: as que mantêm o build e o oráculo
  funcionando (ex.: patch de segurança NU1903 subindo `Microsoft.AspNetCore.DataProtection` para 10.0.13).
- `legacy/` é a origem do oráculo: `oracle-capture` publica `legacy/fur` e executa os comandos migrados
  para gravar o baseline.
- `publish.ps1`, `format.ps1` e `dotnet-tools.json` vivem em `legacy/`; o NBGV continua versionando o
  legado a partir de `legacy/version.json`.
- As skills `furlab-command-manager` (em `.claude/` e `.opencode/`) valem **apenas** para o legado.
- Os pacotes existentes (`FurLab` no NuGet, `FurLab.CLI` no winget) não são tocados. O Mustel publica
  `Mustel.CLI` no winget e `mustel`, `mustel-core`, `mustel-gui` no crates.io.

## Guard Rails (Agent Safety Protocol)

> **Regra obrigatória:** este repositório exige **confirmação manual** para toda operação de Git que
> mude estado.

### 1. Mutações restritas (exigem permissão explícita)

`git add`, `git commit`, `git rm`, `git mv`, `git push`, `git pull`, `git fetch`, `git merge`,
`git rebase`, `git reset`, `git cherry-pick`, `git checkout` (com arquivos modificados),
`git branch -d`, `git stash`, e remoção ou modificação destrutiva de arquivos rastreados.

### 2. Operações de leitura autorizadas

`git status`, `git log`, `git diff`, `git show`, `git branch -a`, `git rev-parse`.

### 3. Fluxo quando uma mutação é necessária

1. **Analisar** os comandos exatos.
2. **Divulgar** o comando e o impacto no repositório.
3. **Pedir** confirmação explícita (ex.: "Confirmo executar `git commit`? [s/N]").
4. **Aguardar** a confirmação antes de executar.

### 4. Regras adicionais

- Commits em inglês, Conventional Commits (`feat:`, `fix:`, `chore:`, `test:`), um por tarefa ou fase —
  e **nunca** sem pedido explícito do usuário.
- Nunca `git push`, merge ou rebase sem autorização específica para aquele destino.

## OpenSpec

O fluxo OpenSpec continua sendo o lugar das especificações canônicas (`openspec/specs/<slug>/spec.md`,
em pt-BR). As specs dos grupos migrados são o contrato comportamental herdado; specs novas só quando o
fluxo for reaberto.
