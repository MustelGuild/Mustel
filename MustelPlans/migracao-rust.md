# Plano — Migração do FurLab (.NET) para Mustel (Rust)

> **Status:** em-andamento
> **Criado em:** 2026-10-14
> **Atualizado em:** 2026-10-14
> **Complexidade:** alta
> **Depende de (outros planos):** nenhum

---

## 1. Visão Geral

Reescrever em Rust os grupos **`query`, `database` e `settings`** do FurLab — CLI .NET 10 com 11.213 LOC,
24 comandos-folha em 13 branches, 18 specs OpenSpec e distribuição via NuGet tool + exe self-contained +
winget — renomeando o produto para **Mustel** (binário `mustel`), com **paridade 1:1 do CLI** e uma GUI em
**egui** sobre o mesmo núcleo (sem API HTTP).

Motivação declarada: **aprender Rust** e **performance** (startup, streaming de CSV, conversão de
encoding, distribuição em binários nativos cross-platform). O valor esperado é um único stack (CLI + GUI
+ build) em **um binário `mustel`** com subcomando `gui`, com comportamento idêntico ao atual para quem
já usa o CLI.

A estratégia central não é "reescrever de memória": o binário .NET publicado é preservado em
`legacy/` e usado como **oráculo executável**. Para cada comando, o comportamento atual é capturado
em baseline *antes* da implementação em Rust, e cada comando só é considerado pronto quando
diverge do oráculo apenas onde for explicitamente aceito (ver escada de fidelidade, D8).

**Escopo decidido (Q6):** migração **completa** dos grupos `query`, `database` e `settings`, com a GUI em
egui cobrindo `query run` + servidores. Os demais grupos (`file`, `claude`, `opencode`, `winget`,
`windowsfeatures`, `docker`) **não** entram nesta migração: seguem no FurLab .NET e estão registrados como
backlog na seção 10 — mantendo dois produtos vivos até que sejam migrados ou descartados.

---

## 2. Escopo

**Inclui:**

- Renomeação do projeto/produto/binário para **Mustel** (crates `mustel-*`, binário `mustel`).
- Workspace Cargo na raiz, com os projetos .NET movidos para `legacy/` via `git mv`.
- Port **completo** dos grupos `query` (`run`), `database` (`backup`, `restore`, `pgpass add|list|remove`)
  e `settings` (`db-servers ls|add|rm|test|set-password`), com paridade N1–N3 (superfície,
  comportamento/artefatos, fluxos interativos).
- Config `mustel.jsonc` (JSONC, no diretório de config do SO) começando **vazia** — sem import do
  `furlab.jsonc` legado (Q8).
- Credenciais via `keyring` nativo (Windows Credential Manager / macOS Keychain / Secret Service).
- Harness de oráculo + baseline versionado (`oracle/`) e infra de testes (unit, CLI, PTY, integração Postgres).
- CI (fmt/clippy/test em Windows, Linux, macOS) e release matrix (win-x64, linux-x64 musl, macos-x64, macos-arm64).
- Publicação no **crates.io** (`mustel-core`, `mustel-gui`, `mustel`), habilitando `cargo install mustel`,
  e no **winget** sob o novo id `Mustel.CLI`.
- GUI `mustel gui` em egui (fase 5), consumindo `mustel-core` em processo.
- Baseline e relatório de performance (.NET vs Rust) com metas verificáveis.
- Reescrita de `AGENTS.md`/`CLAUDE.md` para as convenções Rust e atualização de README/docs.

**Exclui explicitamente:**

- Features das branches pendentes `feature/api` (API HTTP + SSE), `feature/update` (auto-update),
  `feature/gui` (Tauri + Angular) e `quality`: **não** são migradas e não entram na branch de migração.
- Byte-match de `--help`, cores ANSI, tabelas e barras de progresso (ver D8, nível N4).
- Migração dos blobs DPAPI já gravados em `encryptedPassword` (D6).
- Publicação como `dotnet tool`/NuGet (deixa de existir ao fim da migração).
- Renomeação do repositório GitHub `liphvf/FurLab` (renomear **depois**, fora desta migração — Q3).
- Assinatura de código / notarização Apple (apenas documentar o contorno de quarentena).
- Grupos de comando **fora desta primeira onda**: `file`, `claude`, `opencode`, `winget`, `windowsfeatures`,
  `docker` (backlog na seção 10). Continuam no FurLab .NET, congelado, até a segunda onda de migração (D21).
- Tauri/Angular, API HTTP local, update check, telemetria, i18n do CLI.
- Features novas que não existem no `main`.

> Se durante a execução surgir algo na lista "Exclui", **NÃO implementar**.
> Registrar na seção 8 (Questões em Aberto) e perguntar ao usuário.

---

## 3. Decisões

| #   | Decisão                                   | Escolha                                                                 | Razão                                                                 |
|-----|-------------------------------------------|-------------------------------------------------------------------------|-----------------------------------------------------------------------|
| D1  | Motivação dominante                       | Aprender Rust + performance                                             | Define metas de perf (T11/T46) e tolerância a retrabalho              |
| D2  | Branch da migração                        | `feat/rust-migration` a partir de `main`                                | Escolha do usuário                                                    |
| D3  | Convivência .NET/Rust                     | Workspace Cargo na raiz; .NET em `legacy/` até o corte (fase 8)          | Permite usar o .NET como oráculo em todos os comandos                 |
| D4  | Branches pendentes (`api`, `gui`, `update`, `quality`) | Fora de escopo, não migradas                              | Decisão do usuário; evita alvo móvel                                  |
| D5  | Linguagem/UI da GUI                       | **egui** (immediate mode), sem Tauri/Angular                            | Usuário: foco em performance; Tauri traria webview + stack JS         |
| D6  | Armazenamento de credenciais              | `keyring` nativo; **sem** migração de senhas DPAPI; config começa vazia  | Usuário aceitou reentrada de senhas (Q8)                              |
| D7  | Alvos de distribuição                     | win-x64, linux-x64 (musl), macos-x64, macos-arm64                        | Usuário pediu Windows/mac/Linux                                        |
| D8  | Escada de fidelidade do CLI               | N1 superfície, N2 comportamento/artefatos, N3 interativo — obrigatórios; N4 formatação textual — não | Paridade testável sem perseguir byte-match de `--help`/ANSI |
| D9  | Integração CLI ↔ GUI                      | Núcleo compartilhado + eventos, ambos em processo; **binário único** `mustel` com subcomando `gui` (`eframe` atrás da feature `gui`, ligada por padrão) | Preferência do usuário (Q1); crates separados (`mustel-core`/`mustel-gui`) deixam a divisão em dois binários como uma linha de `[[bin]]` |
| D10 | Nome/paths do produto                     | Binário `mustel`; config `mustel.jsonc` no dir de config do SO           | Consistência com o rename                                             |
| D11 | Crates principais                         | `clap`, `tokio`, `inquire`, `indicatif`, `tabled`, `csv`, `chardetng`+`encoding_rs`, `globset`+`walkdir`, `serde`+`jsonc-parser`, `dirs`, `tracing`, `thiserror`/`anyhow`, `keyring`, `tokio-util`, `eframe`/`egui` | Equivalentes diretos das deps atuais |
| D12 | Driver Postgres                          | **`tokio-postgres`** (decidido pelo usuário em 2026-10-15; `sqlx` descartado) | Acesso direto a `Row`/`Column`/`Type` para mapear tipos arbitrários → CSV; `sqlx` empurra para tipos conhecidos em tempo de compilação e o `query run` executa SQL livre |
| D13 | Convenções do repositório                 | Output do CLI em **inglês**; plano/docs/comunicação em **pt-BR**; commits em inglês (Conventional Commits) | Regra vigente no `AGENTS.md` e histórico do repo |
| D14 | Mutações de Git                           | Exigem autorização explícita do usuário (guard rails do `AGENTS.md`)     | Regra vigente; inclui criar branch, `git mv`, `git add/commit`        |
| D15 | Versão inicial do workspace Rust          | `1.0.0` (Mustel é software novo); tags `v1.x.y` dirigem o release        | Decisão do usuário (Q4); NBGV sai de cena                             |
| D16 | Oráculo                                   | Binário .NET em `legacy/` é a fonte de verdade; baseline versionado em `oracle/baseline/` | Onde docs e código divergem, vale o **código observado** (ex.: `AGENTS.md` diz `-f/--servers`, o código diz `-i/--input`) |
| D17 | Canais de distribuição                    | crates.io (`mustel-core`, `mustel-gui`, `mustel`) + GitHub Releases + winget `Mustel.CLI` | Usuário (Q2); nomes verificados livres no crates.io em 2026-10-14 |
| D18 | Repositório GitHub                        | Mantém `liphvf/FurLab` nesta migração; rename depois                     | Usuário (Q3)                                                          |
| D19 | Escopo da migração                        | Grupos `query`, `database` e `settings` **completos**; demais grupos no backlog (seção 10), permanecendo no FurLab .NET | Usuário (Q6): "seria os comandos de forma completa: query, database, settings" |
| D20 | Licença                                   | `GPL-3.0-only` (o que está no `LICENSE`); README/docs corrigidos (afirmavam MIT) | Usuário (Q9): "vamos usar o que está no LICENSE"                        |
| D21 | Backlog                                   | Grupos fora do escopo serão migrados numa **segunda onda**, depois do 1.0 (não é descarte) | Usuário (Q11): "será feito depois"                                    |
| D22 | Pacote winget antigo                      | `FurLab.CLI` permanece como está; nenhuma ação sobre ele                 | Usuário (Q12): "não mexer no antigo pacote"                           |
| D23 | Notarização/code-signing macOS            | Fora do escopo; documentar `xattr -d com.apple.quarantine` no README     | Default mantido (não confirmado explicitamente em Q12)                |
| D24 | Automação de CI e scaffolding de agentes | **Removidos pelo dono do projeto** (commit `3c81ff1`, 2026-10-18): os workflows (`.github/workflows/*`) e o scaffolding de prompts/skills (`.github/prompts`, `.github/skills`) não existem mais | Decisão explícita do usuário, commitada e publicada por ele; **não reverter**. Consequências: o gate de paridade passa a rodar **à mão** (`oracle-compare` local), `cargo fmt/clippy/test` idem, e o release do T44 precisa de estratégia nova, a definir com o usuário |

---

## 4. Arquitetura / Design

### 4.1 Workspace

```
raiz do repositório
├── Cargo.toml                 ← workspace (version = 1.0.0, resolver = 3, lints deny)
├── rust-toolchain.toml        ← pin 1.98.1
├── rustfmt.toml
├── crates/
│   ├── mustel-core/           ← lib: regras de negócio, sem UI
│   │   ├── config/            ← mustel.jsonc (serde + jsonc-parser), por SO, sem import
│   │   ├── credentials/       ← trait CredentialStore + impl keyring
│   │   ├── process/           ← runner de processos (tokio), kill tree no cancel
│   │   ├── postgres/          ← locator (pg_dump/pg_restore), password handler, client, type mapping
│   │   ├── pgpass/            ← pgpass.conf (paths por SO, curingas)
│   │   ├── encoding/          ← chardetng + encoding_rs, extensões de texto
│   │   ├── events.rs          ← CoreEvent + EventSink (seam CLI/GUI)
│   │   ├── errors.rs          ← ExitCode + conversões
│   │   └── models/
│   ├── mustel/                ← pacote do binário `mustel` (publicado no crates.io)
│   │   ├── cli.rs             ← árvore clap (grupos migrados)
│   │   ├── ui/                ← prompts, tabelas, progresso, estilo
│   │   └── commands/          ← comandos finos: parse → resolve → core
│   ├── mustel-gui/            ← lib egui (eframe), ligada pelo bin `mustel` via feature `gui`
│   └── mustel-testkit/        ← fixtures + bins oracle-capture / oracle-compare (`publish = false`)
├── oracle/
│   ├── baseline/              ← capturas do binário .NET (versionado)
│   ├── reports/               ← relatórios de divergência (gitignored)
│   ├── perf/baseline.md       ← números do .NET (T11)
│   └── known-divergences.md   ← divergências N4 aceitas
├── legacy/                    ← FurLab.Core | CLI | Tests | slnx (temporário)
├── docs/  openspec/  MustelPlans/
└── .github/                  ← **sem workflows** (removidos pelo dono em D24)
```

### 4.2 Fluxo do oráculo (paridade)

```
                     +-----------------------------+
   fixtures  ------->|  oracle-capture (legacy/fur) |---> oracle/baseline/<cmd>/*.json
   (SQL, configs,    +-----------------------------+          |
    pgpass, dumps)   +-----------------------------+          |
                     |  oracle-compare (mustel)    |<---------+
                     +-----------------------------+
                              |  compara: exit code, stdout/stderr normalizados
                              |  (FurLab/fur <-> mustel), artefatos byte a byte
                              v
                     relatório: divergências N1-N3 = falha | N4 = aviso
```

### 4.3 Integração CLI ↔ GUI (D9, confirmar em Q1)

```
            +---------------------------------------------------+
            |  mustel-core                                      |
            |  casos de uso (query_run, db_servers_add, ...)     |
            |  + trait EventSink { Progress | Log | Row | Done } |
            +--------+---------------------------------+--------+
                     |                                 |
     mustel (cli) |                                 | mustel-gui (egui)
  (prompts, tabelas, TerminalSink)                | (AppState, canal, request_repaint)
                     v                                 v
        EventSink: TerminalSink                  EventSink: ChannelSink
```

- **Regra de ouro:** parsing de argumentos e *prompts* ficam no `mustel`; o core recebe entradas
  já resolvidas (`QueryPlan`, `BackupPlan`, ...). Isso é o que permite a GUI reusar o núcleo sem
  reimplementar UX de terminal — e é exatamente o desenho que o `feature/api` já validou
  (`QueryPlannerService` + `IProgressObserverService`).
- **Concorrência:** runtime tokio em background; a thread principal do egui só desenha.
  Resultados volumosos (CSV/rows) chegam por canal e são renderizados em tabela virtualizada
  (`egui_extras::TableBuilder`).
- **Cancelamento:** `tokio_util::sync::CancellationToken` disparado por botão na GUI ou Ctrl+C no CLI.
- **Distribuição (decidida em Q1):** **binário único** `mustel` com subcomando `mustel gui`; `eframe`/`egui`
  ficam atrás da feature `gui`, ligada por padrão. Custos aceitos de forma consciente: quem instala só o CLI
  compila a árvore do egui, o binário fica maior e o CI precisa de um job `--no-default-features` para
  garantir que o corte continua válido. Como o código segue dividido em `mustel-core`/`mustel-gui`,
  separar em dois binários depois é uma linha de `[[bin]]`, não um refactor.
- **Alternativa documentada (se Q1 = B):** GUI como subprocesso lendo NDJSON (`mustel <cmd> --output json`).
  Custo: novo contrato de saída para manter + building de argv; ganho: isolamento total. Só faz sentido
  se houver intenção de GUI sobre CLI remota/headless — não é o caso hoje.
- **Critério da escolha (por que A vence aqui):** B obriga a manter a superfície do CLI **e** um contrato
  de protocolo em paralelo, e resultados volumosos (1M linhas) pagariam serialização JSON — o oposto da
  motivação de performance.
- **Porta aberta:** como o core emite eventos, um adaptador `--output ndjson` pode ser somado depois
  (~1 dia) sem reescrita. O inverso não vale: escolher B fecha a porta para A.
- **Config compartilhada:** CLI e GUI editam o mesmo `mustel.jsonc`; gravar de forma atômica
  (temp + rename) e re-ler antes de escrever, para não perder alterações concorrentes.
- **Invariante de capacidade:** toda ação da GUI existe como comando do CLI — a GUI amplia a *UX*,
  nunca a capacidade; é isso que mantém o CLI como referência de paridade.
- **Nada de abstração de widgets:** a costura é (entradas resolvidas + eventos), não um `trait Ui`
  implementado por terminal e por egui — esse é o clássico beco sem saída que piora as duas UIs.
- **Eventos carregam dados, não texto formatado:** quem formata é cada frontend. O formatador do CLI
  é a peça crítica de paridade (N2); a GUI mostra os mesmos dados em widgets.

### 4.4 Escada de fidelidade (D8)

| Nível | O que exige | Como se testa |
|-------|-------------|----------------|
| N1 | Mesma árvore de comandos, nomes, aliases, args, flags, defaults; entradas inválidas rejeitadas como hoje | `oracle-compare` (exit 2/41 e mensagens equivalentes), testes `assert_cmd` por comando |
| N2 | Mesmos efeitos: arquivos byte-idênticos (CSV, `mustel.jsonc`, `pgpass.conf`, JSON de backup/features), mesmos exit codes, permissões/mtime preservados | testes de artefato + diff byte a byte contra baseline |
| N3 | Mesmos fluxos interativos: perguntas, ordem, defaults, atalhos e resultado final | testes PTY (`expectrl`) + revisão manual registrada no log |
| N4 | Formatação textual (`--help`, cores, tabelas, progresso, redação de erros livres) | **não bloqueia**; divergência registrada em `oracle/known-divergences.md` |

### 4.5 Arquivos / tipos novos (principais)

- `crates/mustel-core/src/config/{store.rs,model.rs,paths.rs}` — `ConfigStore`, `UserConfig`, `ServerConfigEntry`, `UserDefaults`.
- `crates/mustel-core/src/credentials/{store.rs,keyring_store.rs}` — `trait CredentialStore { get/set/delete }`; impl `keyring`; fallback Linux sem Secret Service (documentado).
- `crates/mustel-core/src/process/runner.rs` — `ProcessRunner` com `Vec<String>` de argumentos (nunca string interpolada), stdout/stderr capturados, kill tree no cancelamento.
- `crates/mustel-core/src/postgres/{locator.rs,password.rs,client.rs,types.rs}` — localização de `pg_dump`/`pg_restore`, resolução de senha, cliente e **mapeamento tipo→texto** para CSV.
- `crates/mustel-core/src/events.rs` — `enum CoreEvent`, `trait EventSink`.
- `crates/mustel/src/ui/*.rs` — prompts (`inquire`), tabelas (`tabled`), progresso (`indicatif`), estilo (`console`/`owo-colors`), detecção de TTY.
- `crates/mustel-testkit/src/{fixtures.rs,oracle.rs,normalize.rs}` + bins `oracle-capture`/`oracle-compare` (crate **não** publicada).

### 4.6 Mudanças em artefatos existentes

- `AGENTS.md` / `CLAUDE.md` — reescritos para convenções Rust (workspace, crates, `cargo fmt`/`clippy`,
  regras de paridade, guard rails de git preservados).
- `.github/` — **sem workflows nem scaffolding** desde `3c81ff1` (D24). O gate de paridade e os checks de qualidade rodam à mão; o release do T44 precisa de estratégia nova.
- `README.md` / `README.pt-BR.md` / `docs/**` — Mustel + Rust; docs de arquitetura e especificação reescritas no corte.
- `openspec/specs/**` — **preservadas** como contrato comportamental (pt-BR); specs novas só quando o fluxo OpenSpec for reaberto.

---

## 5. Tarefas

> **Ordem importa:** capture o baseline do comando (T9) **antes** de implementá-lo.
> **Git:** toda mutação (`checkout -b`, `mv`, `add`, `commit`) exige autorização explícita do usuário (D14).

### Sequenciamento

**Ordem de execução:** Fase 0 (fundação) → Fase 1 (`settings`) → Fase 2 (`pgpass`) → Fase 3 (`database`
backup/restore) → Fase 4 (`query run`) → Fase 5 (GUI egui) → Fase 6 (corte e release).

**Fora do escopo (não executar):** T13, T14, T15 (`file`), T24, T25 (`claude`/`opencode`), T30
(`docker`), T37, T38, T39 (`winget`/`windowsfeatures`). Continuam no FurLab .NET e estão descritos na
seção 10 (backlog).

**IDs não renumerados:** as faixas T13–T15, T24–T25, T30 e T37–T39 ficam reservadas ao backlog, para
não invalidar as referências entre tarefas.

### Fase 0 — Fundação (branch, workspace, rename, oráculo)

- [x] **T1** — Criar a branch `feat/rust-migration` e mover os projetos .NET para `legacy/`
  - Depende de: —
  - Paralelizável com: —
  - Concluído quando: branch existe a partir de `main`; `legacy/FurLab.slnx` + `legacy/FurLab.Core|CLI|Tests` presentes; `dotnet build legacy/FurLab.slnx` e `dotnet test legacy/FurLab.slnx` verdes; `git status` limpo
  - Arquivos: `legacy/**`, `.github/workflows/**`
  - Autorização: **pedir antes** (D14)

- [x] **T2** — Criar o workspace Cargo com os 4 crates
  - Depende de: T1
  - Paralelizável com: T3, T9
  - Concluído quando: `cargo build --workspace` verde; `cargo fmt --all -- --check` e `cargo clippy --workspace --all-targets -- -D warnings` limpos; `target/release/mustel --version` imprime `1.0.0`
  - Arquivos: `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `.gitignore`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs|main.rs`

- [x] **T3** — Renomear o produto para Mustel (build, scripts, instruções de agentes)
  - Depende de: T1
  - Paralelizável com: T2, T4, T5, T6, T7, T8, T9, T10, T11
  - Concluído quando: `AGENTS.md`/`CLAUDE.md` descrevem o workspace Rust + status congelado de `legacy/`; `winget-publish.ps1` usa `Mustel.CLI`/`mustel.exe`; `dotnet-tools.json` e `publish.ps1` movidos para `legacy/`; `format.ps1` removido (substituído por `cargo fmt`); READMEs atualizados; `grep -ri "furlab" --exclude-dir={legacy,openspec,docs,.git,MustelPlans}` retorna apenas ocorrências justificadas em lista branca
  - Arquivos: `AGENTS.md`, `CLAUDE.md`, `README.md`, `README.pt-BR.md`, `winget-publish.ps1`, `format.ps1`
  - Nota: `legacy/` **não** é removido nesta migração (ver T48) — os scripts .NET só mudam de lugar

- [x] **T4** — Config no `mustel-core`: `mustel.jsonc` + modelos (config começa vazia)
  - Depende de: T2
  - Paralelizável com: T3, T5, T6, T7, T8, T9
  - Concluído quando: testes cobrem leitura com comentários (`//`, `/* */`), escrita indentada camelCase, defaults/validações idênticos (`5432`, `Prefer`, `30`, `4`, `["template*","postgres"]`), busca case-insensitive, e criação da config vazia (com `Defaults`) no primeiro uso — **sem** ler o `furlab.jsonc` legado (Q8); `cargo test -p mustel-core` verde
  - Arquivos: `crates/mustel-core/src/config/**`, `crates/mustel-core/src/models/**`

- [x] **T5** — Erros, exit codes e cancelamento
  - Depende de: T2
  - Paralelizável com: T3, T4, T6, T7, T8
  - Concluído quando: exit codes reais extraídos do oráculo (`0,1,10,11,20,21,22,30,40,41,50,130`) implementados e testados; Ctrl+C aborta operação, mata árvore de processos e sai com 130
  - Arquivos: `crates/mustel-core/src/errors.rs`, `crates/mustel-core/src/signal.rs`, `crates/mustel/src/exit.rs`
  - Nota: `AGENTS.md` divergente do código atual (`2` para args inválidos vs `41`) — vale o oráculo (D16)

- [x] **T6** — Esqueleto do CLI: árvore clap dos grupos do escopo, como stubs
  - Depende de: T2, T5
  - Paralelizável com: T3, T4, T7, T8
  - Concluído quando: `mustel --help` e o `--help` de cada branch listam a mesma árvore dos grupos migrados (`query run`, `database backup|restore|pgpass`, `settings db-servers`); `mustel <cmd>` para stub retorna erro coerente; snapshot tests da árvore (insta) verdes
  - Arquivos: `crates/mustel/src/cli.rs`, `crates/mustel/src/commands/**`, `crates/mustel/src/main.rs`

- [x] **T7** — Credenciais no `mustel-core` (keyring)
  - Depende de: T2
  - Paralelizável com: T3, T4, T5, T6, T8
  - Concluído quando: `trait CredentialStore` + impl keyring (service = `mustel`, user = nome do servidor) com testes de round-trip; ausência de Secret Service em Linux retorna erro claro e documentado (fallback registrado em `docs/`); nenhuma senha é escrita em arquivo
  - Arquivos: `crates/mustel-core/src/credentials/**`, `crates/mustel-core/Cargo.toml`

- [x] **T8** — Primitivas de UI do terminal (paridade de prompts)
  - Notas de execução: (a) o crate `mustel` virou **lib + bin** (`src/lib.rs`) para que a árvore de comandos e as primitivas sejam testáveis pela biblioteca e para código ainda não consumido não cair em `dead_code`; (b) tabela usa `tabled::builder::Builder` (a API de dados dinâmicos do tabled 0.22) com `Style::rounded()`, próximo do box do Spectre; (c) `owo-colors` **não** foi usado — o estilo ficou com `console::Style::for_stdout()`, que já decide sozinho entre cor e texto puro, e a dependência foi removida; (d) os testes de PTY (`tests/pty_prompts.rs`) são **Unix-only** (no Windows o `expectrl` depende de `conpty`) e rodam no job ubuntu do CI — **não** foram executados localmente no Windows; lá a paridade interativa é smoke manual
  - Depende de: T2
  - Paralelizável com: T3, T4, T5, T6, T7
  - Concluído quando: existem seleção simples, multi-seleção com itens pré-selecionados, confirmação (default y/n), texto livre, senha com máscara, tabela e progresso/spinner; comportamento de teclas equivalente ao Spectre; snapshots (insta) e teste PTY (`expectrl`) de cada widget verdes; TTY ausente → mensagem clara em vez de bloqueio
  - Arquivos: `crates/mustel/src/ui/**`, `crates/mustel/tests/pty_prompts.rs`

- [x] **T9** — Oráculo: captura de baseline do binário .NET
  - Depende de: T1
  - Paralelizável com: T2, T3, T4, T5, T6, T7, T8, T10, T11
  - Concluído quando: `cargo run -p mustel-testkit --bin oracle-capture` publica o `legacy/fur` e grava, para cada comando do escopo, `--help`, execuções com fixtures não destrutivas, exit codes e artefatos em `oracle/baseline/`; fixtures versionados; README do harness em `oracle/README.md`
  - Nota (descoberto ao implementar T6): incluir também **argumentos inválidos** (ex.: `database pgpass remove` sem o posicional) para fixar o exit code de erro de parse — o clap devolve `2`, e o Spectre pode devolver outro valor (teste `missing_required_argument_is_a_usage_error`)
  - Nota (isolamento): o legado fixa a config em `%LOCALAPPDATA%\FurLab\furlab.jsonc` via `Environment.SpecialFolder.LocalApplicationData`, que **não** respeita variável de ambiente no Windows. O harness precisa salvar/restaurar esse arquivo em volta das capturas que escrevem (ou capturar só leitura) para não tocar a config real do usuário
  - Arquivos: `crates/mustel-testkit/**`, `oracle/baseline/**`, `oracle/README.md`

- [x] **T10** — Harness de comparação + CI
  - Depende de: T2, T9
  - Paralelizável com: T3, T4, T5, T6, T7, T8, T11
  - Concluído quando: `oracle-compare` compara exit code, stdout/stderr normalizados (`FurLab|fur` ↔ `mustel`) e artefatos byte a byte, classifica divergências N1–N3 (falha) e N4 (aviso), gera `oracle/reports/<ts>/report.md` e sai != 0 em falha
  - **Ressalva (D24)**: o critério previa `ci.yml`/`compat.yml`, que o dono removeu em `3c81ff1`. O entregável que permanece é o comparador rodando **localmente**; automação de CI deixou de ser requisito
  - Nota (desenho derivado do T9): comparar **sempre** exit code e artefatos (SHA-256 + texto) — são o contrato duro; comparar stdout/stderr como **aviso N4** (a redação difere por framework), com normalização de CRLF→LF e sem comparar texto quando a saída do legado **não é UTF-8** (tabela do `ls` sai em codepage do console: 414 bytes inválidos em UTF-8) — nesse caso o relatório marca "texto pulado (codepage)". Manter no binário uma lista explícita de comandos já migrados: caso ainda seja stub, registrar como "pulado (não migrado)" em vez de falha, para o gate não mentir
  - Nota: o `oracle-capture` já grava `stdout_b64`/`stderr_b64`; o comparador deve usá-los para decidir se o texto é comparável
  - Arquivos: `crates/mustel-testkit/src/{oracle.rs,normalize.rs}`

- [x] **T11** — Baseline de performance do .NET
  - Notas de execução: (a) `hyperfine` **não** está instalado nesta máquina — a medição foi implementada no harness (`mustel-testkit::perf` + bin `oracle-perf`, mediana de 30 execuções do processo completo), o que também a torna reproduzível no CI; (b) o relatório mede **os dois lados** a cada execução (nada de número fixado no gerador); (c) métricas dependentes de Postgres (CSV de 100k linhas, `database backup`) ficam para T32/T28, que terão container; (d) o tamanho do oráculo publicado é **framework-dependent** e não é comparável ao binário auto-contido do Mustel — o relatório alerta sobre isso
  - Depende de: T9
  - Paralelizável com: T3, T10
  - Concluído quando: `oracle/perf/baseline.md` registra, com metodologia e desvio: startup de `--help` (mediana de 20 execuções, `hyperfine --warmup 3`), `settings db-servers ls`, export CSV de 100k linhas, `database backup` de um dump de fixture e tamanho do `fur.exe` self-contained
  - Arquivos: `oracle/perf/baseline.md`, `oracle/perf/scripts/**`

- [x] **T12** — Gate — fundação
  - Depende de: T1–T11
  - Paralelizável com: —
  - Concluído quando: `cargo fmt --check`, `cargo clippy -D warnings` e `cargo test --workspace` verdes; `mustel --help` cobre a árvore completa; `oracle-compare` roda e reporta 0 divergências N1–N3 nos comandos ainda não implementados (stubs), com exceções listadas em `known-divergences.md`
  - Arquivos: —

### Fase 1 — Configuração e credenciais (`settings db-servers`)

- [x] **T16** — `settings db-servers ls`
  - Depende de: T4, T6, T9, T10
  - Paralelizável com: T19
  - Concluído quando: lista igual à do oráculo para configs de fixture (inclusive config vazio e servidor sem senha); exit codes iguais; spec `openspec/specs/settings-db-servers` coberta
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/list.rs`

- [x] **T17** — `settings db-servers add` (flags + interativo)
  - Notas de execução: (a) o texto do critério citava `-D/--database`, que **não existe** no código legado — as flags reais são `-n/--name`, `-H/--host`, `-p/--port`, `-U/--username`, `-d/--databases`, `--ssl-mode`, `--timeout`, `--max-parallelism`, `--fetch-all`, `--exclude-patterns`, mais `-i/--interactive` e `-W/--password` acrescentados por decisão do usuário; (b) validações portadas em `mustel-core::validation` (porte fiel do `SecurityUtils`, inclusive a permissividade do legado com IPv4 e o curinga `*`); (c) **nome duplicado é erro 40**, não atualização — o comando verifica antes, apesar de o core ter `add_or_update`; (d) `-W` grava a senha no keyring (o legado gravava blob no arquivo)
  - Depende de: T4, T6, T8, T16
  - Paralelizável com: T19
  - Concluído quando: flags (`-n/--name`, `-H/--host`, `-p/--port`, `-U/--username`, `-D/--database`...) e modo interativo com paridade N1/N3; validações de host/porta/usuário equivalentes (incluindo `*`); `mustel.jsonc` resultante byte-idêntico nos fixtures; spec `settings-user-config` coberta
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/add.rs`

- [x] **T18** — `settings db-servers rm`
  - Notas de execução: (a) o caminho sem `-n` usa multi-seleção e imprime texto **diferente** do caminho com `-n` (`Server 'x' removed.` vs `removed successfully.`) — detalhe fácil de perder, copiado do código; (b) a remoção apaga a credencial do cofre em best-effort (adição do Mustel; o legado tinha o blob no arquivo); (c) novo caso `settings-db-servers-rm-ok` no baseline captura o artefato de config resultante; (d) o comparador ganhou mapa explícito de caso-de-erro → comando, porque `error-db-servers-rm-unknown` continuava "pulado" mesmo com o `rm` migrado
  - Depende de: T17
  - Paralelizável com: T19
  - Concluído quando: remoção por nome (case-insensitive), confirmação/mensagens e exit codes equivalentes; config resultante idêntica; remoção também limpa a credencial no keyring (comportamento documentado se o oráculo divergir)
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/remove.rs`

- [x] **T19** — Camada PostgreSQL com `tokio-postgres`
  - Notas de execução: decisão **já tomada pelo usuário** (D12: `tokio-postgres`), então o spike deixou de ser comparação e virou implementação com evidência. Pronto e testado: tradução dos 6 modos de `sslMode` (`postgres/tls.rs`), configuração de conexão com os defaults do legado (`postgres/settings.rs`), formatação de célula para CSV isolada e testável (`postgres/value.rs`), conexão e listagem de bancos (`postgres/client.rs`) e classificação de erro em `CoreError::Postgres { sqlstate }`
  - **Integração executada** contra Postgres 16.15 em container (porta 55432): 4 testes verdes — conexão/escalar, leitura dos tipos (bool, `int8` no máximo, `float8`, texto, `NULL`, `jsonb`), **senha errada classificada com SQLSTATE `28P01` → exit 10** (a regra do legado é `SqlState: not null` → 10; minha suposição anterior de "sem SQLSTATE → 11" estava errada) e servidor inalcançável falhando sem pendurar no timeout
  - **Ainda pendente (registrado, não escondido)**: TLS não ligada — `Require`/`VerifyCA`/`VerifyFull` devolvem erro claro em vez de degradar para texto puro; separar `VerifyCA` (que no Npgsql **não** valida hostname) de `VerifyFull` exige `ServerCertVerifier` próprio no rustls. Pool também não existe ainda (campos prontos; biblioteca a escolher no T34). Formato exato do texto do CSV (`bytea`/`timestamptz`/`numeric`) fica para o T32, comparando com o oráculo
  - Depende de: T2
  - Paralelizável com: T16, T17, T18
  - Concluído quando: `docs/rust-postgres-spike.md` registra a decisão (`sqlx` vs `tokio-postgres`) com evidência para: (a) mapeamento tipo→texto de resultados arbitrários (`int4`, `numeric`, `timestamptz`, `bool`, `json/jsonb`, `bytea`, arrays, `uuid`, `NULL`), (b) opções de pool equivalentes (`pooling`, `min/max pool size`, `keepalive`, `connection lifetime`), (c) `sslmode` `Disable|Allow|Prefer|Require|VerifyCA|VerifyFull`, (d) timeout de conexão/comando; testes de integração com Postgres em container cobrindo (a)–(d)
  - Arquivos: `docs/rust-postgres-spike.md`, `crates/mustel-core/src/postgres/**`, `crates/mustel-core/tests/postgres_integration.rs`

- [x] **T20** — `settings db-servers test`
  - Notas de execução: (a) **o legado não usa o driver** — lista bancos executando o **`psql` externo** (`SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname;`), exigindo o binário instalado; o Mustel usa o driver (D12) com a **mesma query**, divergência registrada; (b) falha de conexão sai com exit **40**, não 10/11 — o legado embrulha o erro em `InvalidOperationException` e troca o prefixo entre `Authentication failed:` e `Connection failed:`; (c) senha vem do cofre, com o mesmo aviso `No password found for 'x'. Type to test (will not be saved):` seguido de prompt secreto
  - Verificado ponta a ponta contra o container: sucesso imprime `Connection to localhost:55432 successful`, `Authenticated as postgres` e `Databases found: mustel_test, postgres` (exit 0); senha errada → `Authentication failed: FATAL: password authentication failed...` (exit 40); servidor inexistente → 41
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/test.rs`

- [x] **T21** — `settings db-servers set-password` (keyring)
  - Depende de: T7, T17
  - Paralelizável com: T19, T20
  - Concluído quando: prompt de senha com máscara, grava no keyring, `mustel.jsonc` não contém senha em claro nem blob DPAPI; `query run`/`database` passam a usar a senha do keyring; spec `credential-storage` coberta (com a divergência DPAPI→keyring registrada)
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/set_password.rs`

- [x] **T22** — Gate — `settings db-servers`
  - Verificação (executada em 2026-10-15): `oracle-compare --scope settings` → **11 casos, 0 falhas** (2 em profundidade, 9 avisos N4, **0 pulados**); fluxo manual `add -n fluxo -H localhost -p 55432 --ssl-mode Disable` → `set-password fluxo -W dev` → `test -n fluxo` (conectou de verdade: `Connection to localhost:55432 successful` / `Authenticated as postgres` / `Databases found: mustel_test, postgres`) → `ls` (tabela com `fluxo`) → `rm -n fluxo`, com o cofre verificado limpo no fim (`cmdkey /list` sem resíduo)
  - Depende de: T16–T21
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `settings` sem divergências N1–N3; verificação manual do fluxo `add` → `test` → `set-password` registrada no log
  - Arquivos: —

### Fase 2 — `database pgpass`

- [x] **T23** — `database pgpass add|list|remove`
  - Notas de execução: (a) **o legado não aceita o banco como posicional** — `fur database pgpass add mydb` responde `Error: Unknown command 'mydb'.`; a forma posicional é a adição já registrada no item 5 de `known-divergences.md`, e por isso nenhum caso do baseline a usa (o argv roda nos dois binários); (b) `add` com banco/host/porta inválidos sai com **exit 2** vindo do **corpo** do comando (`return Task.FromResult(2)`), não do parser: o mustel devolve `Ok(2)` ali, sem passar por `CoreError` (que daria 41); (c) `--password` **vazio** conta como ausente (`string.IsNullOrEmpty`) e dispara o prompt, enquanto `--password " "` é senha válida; (d) `add` **anexa** a linha: arquivo sem quebra de linha no fim faz a entrada nova entrar **colada** na última — paridade com `File.AppendAllText`, provada por caso de baseline byte a byte; (e) `remove` **reescreve** o arquivo inteiro (comentários e linhas em branco preservados, quebra do SO no fim, arquivo vazio quando sai a última entrada) e "não encontrado" é **exit 0**; (f) o caminho por SO segue `SpecialFolder.ApplicationData` do .NET, o que **inclui macOS** em `~/.config` (não `~/Library/Application Support` nem `~/.pgpass`); (g) no Unix o arquivo fica `0600` — divergência deliberada, item 8 de `known-divergences.md`, verificada em execução
  - **Verificação**: `oracle-compare --scope pgpass` → **16 casos, 0 falhas, 0 pulados** (12 em profundidade, 4 N4 de ajuda); a artefato do `pgpass` é comparado **byte a byte** (39, 94, 78, 21 e 0 bytes nos casos de `add`/`remove`, e "ausente nos dois lados" nos três de validação, provando que a recusa acontece antes de criar o arquivo). O caso `database-pgpass-add-appends-to-a-file-without-final-newline` fixa o comportamento "colado"
  - **Sandbox**: o legado resolve o `pgpass.conf` pelo **registro** do Windows (ignora `%APPDATA%`, verificado na prática), então o harness faz backup/semear/restaurar do arquivo real — que guarda senhas de verdade — e ainda grava uma cópia em `oracle/workdir/legacy-pgpass.backup` para o caso de morte violenta do processo. O Mustel usa outro arquivo (`MUSTEL_PGPASS_FILE` em `oracle/workdir/`), e o `oracle-capture` **recusa** um caso de `pgpass` sem semear (senão o arquivo real iria para o baseline versionado)
  - **Specs**: todos os cenários de comportamento de `pgpass-cli-setup` (curinga em host/porta/usuário, adicionar e remover) e de `pgpass-wildcard-validation` (curinga aceito; ` * ` e `* ` recusados; faixa numérica intacta) têm teste e/ou caso de baseline. **Ressalva**: o requisito de documentação das duas specs aponta para `pgpass-command.md`, arquivo que **não existe** neste repositório (a doc original não foi migrada); o equivalente verificável é o texto de ajuda, que menciona `*` nas três opções (`--host`, `--port`, `--username`)
  - **Teste Unix executado**: o teste `the_file_is_0600_on_unix` é `#[cfg(unix)]` e **não roda** no Windows. Rodado em container (`docker run --rm -v <worktree>:/repo:ro rust:1-bookworm cargo test --lib`, com um crate de verificação que aponta `#[path]` para `crates/mustel-core/src/pgpass/mod.rs`): **28 testes verdes**, incluindo o dos 0600 — foi assim que a permissão foi confirmada em execução, e não só compilada
  - Depende de: T6, T9, T10
  - Paralelizável com: T27
  - Concluído quando: `pgpass.conf` no mesmo caminho por SO do oráculo, com curingas `*` aceitos para host/porta/usuário; arquivo resultante byte-idêntico; permissões `0600` em Unix; specs `pgpass-cli-setup` e `pgpass-wildcard-validation` cobertas
  - Arquivos: `crates/mustel/src/commands/database/pgpass/*.rs`, `crates/mustel-core/src/pgpass/**`

- [x] **T26** — Gate — `pgpass`
  - Verificação (executada em 2026-10-18): `oracle-compare --scope pgpass` → **16 casos, 0 falhas, 0 pulados**. A suíte completa (`oracle-compare` sem filtro) ficou em **40 casos, 0 falhas, 22 N4, 3 pulados** — os 3 pulados são as duas divergências deliberadas do prompt de senha (`error-database-restore-file-missing`, `error-db-servers-test-offline`) e o `error-query-input-missing`, que só fecha quando o `query run` migrar (T32)
  - **Ressalva de dependência**: o plano listava T27 como pré-requisito deste gate; ele **não é** (o `pgpass` não executa processo externo nem fala com Postgres). O gate rodou e fechou sem T27
  - Depende de: T23, T27
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `pgpass` sem divergências N1–N3
  - Arquivos: —

### Fase 3 — Processos externos: `database backup` e `restore`

- [x] **T27** — Infra de processos e Postgres local no `mustel-core`
  - Notas de execução: (a) **o executor do legado não era usado** pelos comandos de banco — o `DatabaseBackupCommand` monta um `ProcessStartInfo` na mão (duas vezes), e a classe `ProcessExecutor` só existia para outros usos; o porte ficou com o contrato de **argumentos como `Vec<String>`/sem shell**, que é o que o `ArgumentList` + `UseShellExecute = false` do legado garantia; (b) `stdout` e `stderr` são drenados **em paralelo**: o `ProcessExecutor` do legado lia os eventos assíncronos, mas o `DatabaseBackupCommand` fazia `ReadToEnd()` de um cano antes do outro, o que trava quando o processo enche o outro (há teste que enche os dois com 40 000 linhas); (c) o legado embrulhava falha de partida em `ExitCode = -1` com o texto em `stderr`; aqui a falha é um **tipo** (`ProcessError::Spawn`/`Io`/`Cancelled`), porque o comando precisa distinguir "não achou o binário" (exit 3) de "pg_dump falhou" (exit 1); (d) no cancelamento a **árvore** morre: `taskkill /T /F` no Windows e, no Unix, o filho abre o próprio grupo (`process_group(0)`) e o grupo leva `SIGKILL` — o utilitário `kill` é usado em vez de `libc::kill` porque o workspace tem `unsafe_code = "forbid"`; (e) `PostgresBinaryLocator` mantém as três etapas de busca do legado na mesma ordem (PATH → `C:\Program Files\PostgreSQL`/`C:\PostgreSQL` com subpasta de versão + `bin\<nome>.exe` → `/usr/bin`, `/usr/local/bin`, `/usr/local/pgsql/bin`), todas rodando em qualquer SO como no original; a única diferença é visitar as subpastas de versão em ordem **alfabética** (o `Directory.GetDirectories` devolve a ordem do FS, que no NTFS também é alfabética) — determinismo pelo mesmo resultado; (f) o handler de senha virou `postgres/password.rs` com a precedência **flag → senha guardada → prompt**, com o prompt injetado como closure (D9: UI não entra no núcleo) e o texto `Enter password: ` fixado em constante — o legado usa esse texto **só** no backup/restore, diferente do `Password: ` do `pgpass add` e do `set-password`; (g) o casamento do servidor é por **host (sem diferenciar maiúsculas) + porta como texto**, sem olhar o nome — é o que `ResolvePasswordFromUserConfig` faz, e é o que permite dois servidores no mesmo host/porta serem indistinguíveis
  - **Verificação**: `cargo test -p mustel-core` → 9 testes do runner (**7 rodando no Windows**, incluindo o de **árvore** com `tasklist` conferindo que o neto morreu, e 2 que só existem no Unix), **8** do localizador (ordem de prioridade, `bin\<nome>.exe` das versões, diretórios do Unix, diretório inexistente, `\*.exe` antes do nome puro) e **8** do handler de senha (flag vence, guardada vence o prompt, prompt por último e usado como veio, branco = ausente, casamento por host/porta sem olhar o nome, cofre pelo nome do servidor, config ausente). `fmt --check` e `clippy -D warnings` limpos; **187 testes** no workspace
  - **Ressalva honesta**: dois testes do runner são `#[cfg(unix)]` (argumentos não interpretados por shell; `SIGKILL` no grupo de processos) e **não** foram executados — o Docker Desktop estava **parado** nesta máquina. O que foi feito: `cargo check --target x86_64-unknown-linux-gnu` do módulo (por um crate de verificação, sem o keyring) compila os dois caminhos; a execução fica para quando o Docker voltar, junto do T31
  - Depende de: T2
  - Paralelizável com: T23
  - Concluído quando: `ProcessRunner` (argumentos como `Vec<String>`, stdout/stderr, kill tree no cancelamento, sem shell) + `PostgresBinaryLocator` (mesmos diretórios de busca/prioridade do oráculo) + `PostgresPasswordHandler` (mesma precedência flag → keyring/config → prompt) com testes unitários verdes
  - Arquivos: `crates/mustel-core/src/process/**`, `crates/mustel-core/src/postgres/{locator.rs,password.rs}`
  - Nota: `PostgresBinaryLocator` serve a T28/T29; `ProcessRunner`/`PostgresPasswordHandler` também são pré-requisito de T32 (`query run`)

- [ ] **T28** — `database backup`
  - Depende de: T19, T27
  - Paralelizável com: T29
  - Concluído quando: `pg_dump` invocado com os mesmos argumentos do oráculo (ordem incluída); `-o`, `--all`, conexão por flags/servidor configurado equivalentes; progresso e mensagens equivalentes; exit codes 10/11/20–22 iguais; dump gerado abre em `pg_restore --list`
  - Arquivos: `crates/mustel/src/commands/database/backup/**`

- [ ] **T29** — `database restore`
  - Depende de: T19, T27
  - Paralelizável com: T28
  - Concluído quando: `--all`, `-d/--directory`, `-v`, `--no-clean`, `--no-owner`, `--no-acl`, `--single-transaction`, `-j` mapeados e repassados igual ao oráculo; resolução de `<db>.dump` no diretório atual equivalente; exit codes iguais; teste de integração restore→dump em container
  - Arquivos: `crates/mustel/src/commands/database/restore/**`

- [ ] **T31** — Gate — `database`
  - Depende de: T27, T28, T29
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `database` sem divergências N1–N3; ciclo real `database backup` → `database restore` executado e registrado no log
  - Arquivos: —

### Fase 4 — `query run` (maior superfície, 4 sub-entregas)

- [ ] **T32** — 5a: execução single-server/single-database com export CSV
  - Depende de: T19, T27, T8
  - Paralelizável com: —
  - Concluído quando: `-i/--input` e `-c/--command` equivalentes (inclusive exclusão mútua e mensagens de erro); overrides `-H`, `-p`, `-d`, `-U`, `-W`, `--ssl-mode`, `--timeout`, `--command-timeout`, `--pooling`, `--min-pool-size`, `--max-pool-size`, `--keepalive`, `--connection-lifetime`, `--npgsql-connection-string` com paridade de comportamento; CSV consolidado (`Server,Database,<colunas>`) byte-idêntico ao oráculo nos fixtures de tipos; arquivos nomeados igual; exit codes iguais
  - Arquivos: `crates/mustel/src/commands/query/{run.rs,settings.rs}`, `crates/mustel-core/src/query/{planner.rs,executor.rs,csv_writer.rs}`

- [ ] **T33** — 5b: descoberta de bancos (`--all`, `--exclude`, `fetchAllDatabases`, `excludePatterns`)
  - Depende de: T32
  - Paralelizável com: —
  - Concluído quando: lista de bancos descobertos igual à do oráculo (mesma query de catálogo e mesmos filtros/curingas); `--exclude` somando aos padrões do servidor; specs `server-auto-discover-databases` coberta
  - Arquivos: `crates/mustel-core/src/query/discovery.rs`, `crates/mustel/src/commands/query/run.rs`

- [ ] **T34** — 5c: multi-servidor em paralelo + `--separate-files` + streaming
  - Depende de: T33
  - Paralelizável com: —
  - Concluído quando: paralelismo respeita `maxParallelism` por servidor com semântica equivalente; `--separate-files` gera `<server>_<timestamp>.csv` com mesmo padrão; escrita incremental com memória limitada (RSS estável em 1M linhas); ordem/consistência do CSV consolidado equivalentes; specs `query-run-multi-server-execution` e `query-run-streaming-csv` cobertas
  - Arquivos: `crates/mustel-core/src/query/executor.rs`, `crates/mustel/src/commands/query/run.rs`

- [ ] **T35** — 5d: interativo, detecção destrutiva, progresso e log de execução
  - Depende de: T34
  - Paralelizável com: —
  - Concluído quando: seleção multi-servidor com todos pré-selecionados e mesmos atalhos; detecção de `INSERT/UPDATE/DELETE/ALTER/DROP/CREATE/TRUNCATE/MERGE/GRANT/REVOKE/SET ROLE` com confirmação e `--no-confirm` equivalentes (sem falsos negativos em fixtures comentados/strings); progresso e log de execução equivalentes; specs `query-run-*` restantes cobertas
  - Arquivos: `crates/mustel/src/commands/query/{analyzer.rs,run.rs}`, `crates/mustel/src/ui/progress.rs`

- [ ] **T36** — Gate — `query run` + benchmark
  - Depende de: T32–T35
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `query` sem divergências N1–N3; benchmark de export 100k linhas ≤ baseline .NET (T11) com RSS registrado em `oracle/perf/query.md`
  - Arquivos: `oracle/perf/query.md`

### Fase 5 — GUI em egui

- [ ] **T40** — Extrair casos de uso + `EventSink` no core
  - Depende de: T32–T36
  - Paralelizável com: —
  - Concluído quando: `query_run`, `db_servers_*`, `database_backup`/`database_restore` e `pgpass_*` são funções de core que recebem entrada resolvida e emitem `CoreEvent`; o CLI passa a consumi-las via `TerminalSink` sem mudança de comportamento (oráculo segue verde); nenhuma dependência de UI no `mustel-core` (`cargo tree -p mustel-core` sem `inquire`/`indicatif`/`eframe`)
  - Arquivos: `crates/mustel-core/src/{events.rs,usecases/**}`, `crates/mustel/src/commands/**`

- [ ] **T41** — Subcomando `gui`: egui sobre o mesmo núcleo, em binário único
  - Depende de: T40
  - Paralelizável com: —
  - Concluído quando: `mustel gui` abre a janela (eframe) a partir de `crates/mustel-gui`; runtime tokio em background; `ChannelSink` entrega eventos com `ctx.request_repaint()`; cancelamento via `CancellationToken`; `eframe`/`egui` estão atrás da feature `gui` (ligada por padrão) e `cargo build --no-default-features` produz CLI sem egui (`cargo tree -p mustel --no-default-features` limpo), validado em CI
  - Arquivos: `crates/mustel-gui/**`, `crates/mustel/src/commands/gui.rs`, `crates/mustel/Cargo.toml`

- [ ] **T42** — Telas MVP da GUI
  - Depende de: T41
  - Paralelizável com: —
  - Concluído quando: telas de **servidores cadastrados** (CRUD no `mustel.jsonc` + teste de conexão) e **query run** (SQL, seleção de servidores, execução com progresso, resultados em tabela virtualizada, export CSV) funcionam — `query run` é a feature principal; telas de `database backup`/`restore` e `pgpass` ficam para depois (backlog, seção 10); toda operação usa o core (nenhuma regra duplicada); smoke manual registrado no log com screenshot
  - Arquivos: `crates/mustel-gui/src/{app.rs,views/**}`

- [ ] **T43** — Gate — GUI egui
  - Depende de: T40–T42
  - Paralelizável com: —
  - Concluído quando: `cargo test --workspace` verde; fluxos da GUI executados manualmente e registrados; `mustel` (CLI) segue com oráculo verde após a refatoração de T40
  - Arquivos: —

### Fase 6 — Corte, release e limpeza

- [ ] **T44** — Release matrix + empacotamento
  - Depende de: T36, T43
  - Paralelizável com: T45, T46
  - Concluído quando: o release entrega `mustel.exe` (win-x64), `mustel` (linux-x64 musl) e `mustel` (macOS x64/arm64) com SHA256, validação `winget validate` do manifesto (`Mustel.CLI`, installer `portable`, command `mustel`) e publicação no crates.io (`mustel-core` → `mustel-gui` → `mustel`, com `license = "GPL-3.0-only"`, `repository`, `description`); `cargo install mustel` funciona nas 3 plataformas
  - **Ressalva (D24)**: com os workflows removidos pelo dono, **como** o release é produzido (script local, GitHub Actions recriado, outra automação) é decisão em aberto — perguntar antes de implementar
  - Arquivos: `winget-publish.ps1`, `packaging/winget/**`, `crates/*/Cargo.toml`

- [ ] **T45** — Documentação final
  - Depende de: T36, T43
  - Paralelizável com: T44, T47
  - Concluído quando: `README.md`/`README.pt-BR.md` descrevem instalação (`cargo install mustel`, release e winget), os grupos migrados e o rename, com a licença corrigida para GPL-3.0 (o README dizia MIT por engano) e uma seção sobre o que permaneceu no FurLab .NET; `docs/pt-BR|en/{ARCHITECTURE,FEATURE_SPECIFICATION}.md` refletem a arquitetura Rust; `AGENTS.md`/`CLAUDE.md` descrevem o workspace Rust **e** o `legacy/` congelado; `oracle/README.md` explica como rodar a paridade
  - Arquivos: `README*.md`, `docs/**`, `AGENTS.md`, `CLAUDE.md`, `oracle/README.md`

- [ ] **T46** — Relatório de performance final
  - Depende de: T44
  - Paralelizável com: T45, T47
  - Concluído quando: `oracle/perf/final.md` compara baseline .NET vs Rust (startup `--help` ≤ 1/3 do baseline, export CSV de 100k linhas ≤ baseline, `database backup` ≤ baseline, tamanho de binário ≤ `fur.exe`, RSS pico), com metodologia, desvios e veredito
  - Arquivos: `oracle/perf/final.md`

- [ ] **T47** — Gate final de paridade (escopo migrado)
  - Depende de: T36, T43, T45
  - Paralelizável com: T46
  - Concluído quando: `oracle-compare` completo, filtrado pelos grupos migrados (`query`, `database`, `settings`), sem divergências N1–N3, com `known-divergences.md` revisado e aprovado pelo usuário; `cargo fmt/clippy/test` verdes; smoke manual dos fluxos interativos principais registrado
  - Arquivos: `oracle/known-divergences.md`

- [ ] **T48** — Congelar `legacy/` e limpar artefatos mortos
  - Depende de: T47 (obrigatoriamente **depois** do gate final)
  - Paralelizável com: —
  - Concluído quando: `legacy/` **preservado** (é o produto .NET dos grupos fora do escopo e a fonte do oráculo) com `legacy/README.md` explicando o status congelado; workflow de publicação NuGet removido; `publish.ps1`, `format.ps1` e `dotnet-tools.json` sob `legacy/`; `version.json`/NBGV mantidos para o build do legado; `grep -ri fur --exclude-dir={.git,openspec,docs,legacy}` só retorna ocorrências justificadas; `cargo build --workspace` e `cargo test --workspace` verdes
  - Arquivos: `legacy/README.md`, `.github/workflows/**`, `legacy/**`
  - Autorização: **pedir antes** (remoção de arquivos rastreados, D14)

---

## 6. Estratégia de Verificação

Executar sempre que mudanças forem feitas no workspace Rust:

- **Formatação:** `cargo fmt --all -- --check` — sem diffs
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero warnings
- **Build:** `cargo build --workspace --release` — verde; `./target/release/mustel --version` = `1.0.0`
- **Testes:** `cargo test --workspace` — verde
- **Paridade (obrigatório por comando entregue):**
  `cargo run -p mustel-testkit --bin oracle-compare -- --report --scope query,database,settings` —
  0 divergências N1–N3; N4 registradas em `oracle/known-divergences.md`
- **Interativo:** `cargo test -p mustel --test pty_prompts` — verde (widgets e fluxos com PTY)
- **Integração Postgres:** `docker compose -f oracle/compose.postgres.yml up -d` e
  `cargo test -p mustel-core --test postgres_integration` — verde
- **Publicação (crates.io):** `cargo publish --dry-run -p mustel-core -p mustel-gui -p mustel` sem erro;
  `cargo install --path crates/mustel --force` seguido de `mustel --version`
- **Licença:** `cargo deny check licenses` (ou equivalente) sem incompatibilidade; `license = "GPL-3.0-only"`
  nos três crates publicados
- **Diagnostics do editor:** rodar os diagnostics (rust-analyzer/clippy) nos arquivos alterados
- **Grep de resíduos:** `grep -ri "furlab" --exclude-dir={.git,legacy,openspec,docs,MustelPlans}` deve retornar
  apenas as ocorrências em lista branca documentada em `oracle/known-divergences.md`
- **Smoke manual (interativo, 1x por gate de fase):** fluxo real do comando no terminal, registrado na seção 9
- **Performance (fases 4 e 6):** `hyperfine --warmup 3` no binário .NET (baseline) e no Rust, comparando com `oracle/perf/*.md`

> O plano só pode ser marcado como `concluído` após **todas** as verificações acima passarem.
> Se algo falhar, registrar na seção 9 e corrigir antes de devolver ao usuário.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|-------|------|---------|-----------|
| Comportamento não documentado se perde (docs divergem do código: `-f/--servers` no `AGENTS.md` vs `-i/--input` real) | alta | alto | Oráculo é a fonte de verdade (D16); capturar baseline antes de cada comando |
| Mapeamento de tipos Postgres → CSV divergir do Npgsql (numeric/bytea/json/timestamptz/arrays/NULL) | média | alto | Spike T19 com fixtures por tipo + diff byte a byte contra baseline |
| `sslmode` `VerifyCA`/`VerifyFull` sem equivalente direto (rustls verifica hostname sempre) | média | médio | Spike T19 item (c); se necessário, validador customizado; divergência documentada |
| `--npgsql-connection-string` depende do parser do Npgsql | média | médio | Decidido **manter** (Q5): implementar parser `keyword=value` compatível + testes |
| Licença inconsistente: `LICENSE` é GPL-3.0 e o README diz MIT | alta | médio | Resolvido em Q9 (`GPL-3.0-only`): corrigir README/docs em T45 antes do primeiro `cargo publish` |
| Dois produtos vivos (Mustel Rust + FurLab .NET) até a segunda onda | alta | médio | .NET congelado com `legacy/README.md` de estado; segunda onda planejada após o 1.0 (Q11/D21); oráculo compartilhado mantém a paridade verificável |
| Nome de crate indisponível no crates.io | baixa | médio | Verificado em 2026-10-14: `mustel`, `mustel-core`, `mustel-gui`, `mustel-cli`, `mustel-testkit` livres (HTTP 404); revalidar antes do primeiro publish |
| Paridade interativa difícil de testar automaticamente | média | médio | Escada N3 + `expectrl`/PTY + smoke manual registrado por fase |
| `keyring` indisponível em Linux headless (sem Secret Service) | média | médio | Detectar e falhar com mensagem clara; fallback documentado (env/arquivo 0600) |
| Migração compete com features novas e vira alvo móvel | alta | médio | Features do .NET congeladas durante a migração; branches pendentes fora de escopo (D4) |
| Regressão de streaming/memória no CSV | baixa | médio | Escrita incremental + teste com 1M linhas + bench (T34/T36) |
| Binário único com egui infla distribuição do CLI | baixa | baixo | Feature `gui` (default-on) + build `--no-default-features` validado em CI |
| Rust inexperiente → sobre-engenharia (abstrações, macros, async onde não precisa) | média | médio | Crates idiomáticas e conservadoras; async só em I/O; clippy `-D warnings`; código simples > esperto |
| Perda de paridade por confiar em `README`/`AGENTS.md` em vez do binário | alta | alto | Toda tarefa de comando exige `oracle-compare` verde (seção 6) |
| Distribuição: winget/macOS sem code-signing (Gatekeeper bloqueia binário baixado) | média | médio | Documentar `xattr -d com.apple.quarantine` no README; assinatura/notarização fora de escopo (D23) |

---

## 8. Questões em Aberto

Itens **não decididos**. O agente **não adivinha**: ou resolve com informação do codebase, ou pergunta ao usuário.

**Nenhuma questão aberta** — todas foram respondidas pelo usuário antes do início da execução.

### Resolvidas

- [x] **Q1** — Integração CLI ↔ GUI: costura por *entradas resolvidas + `EventSink`* em processo, em **um binário** `mustel` com subcomando `gui` (feature `gui` default-on, código dividido em crates).
- [x] **Q2** — winget `Mustel.CLI` + publicação no crates.io (nomes verificados livres em 2026-10-14).
- [x] **Q3** — Renomear o repositório GitHub: **depois**, fora desta migração.
- [x] **Q4** — Versão inicial `1.0.0`.
- [x] **Q5** — `--npgsql-connection-string`: mantida, com parser compatível.
- [x] **Q6** — Escopo: grupos `query`, `database` e `settings` completos; demais grupos no backlog (seção 10).
- [x] **Q7** — NuGet `FurLab` para de ser publicado.
- [x] **Q8** — Sem import do `furlab.jsonc`; config nova começa vazia.
- [x] **Q9** — Licença: `GPL-3.0-only` (o que está no `LICENSE`).
- [x] **Q10** — `mustel-testkit` com `publish = false`.
- [x] **Q11** — Backlog: **segunda onda de migração**, depois do 1.0 ("será feito depois") → D21.
- [x] **Q12** — Pacote winget `FurLab.CLI`: não mexer (D22). Notarização de macOS: default mantido — fora do escopo, com instrução de quarentena (D23).

---

## 9. Log de Execução

Append-only. Registrar início, conclusão, bloqueio, paralelização e falhas.

- `[2026-10-14]` plano criado (rascunho) a partir da exploração em thread; nenhuma linha de código escrita
- `[2026-10-14]` Q2–Q8 respondidas pelo usuário; plano atualizado: pacote publicável no crates.io, versão `1.0.0`, sem import do `furlab.jsonc`, `--npgsql-connection-string` mantida, winget `Mustel.CLI`, NuGet descontinuado
- `[2026-10-14]` achados de exploração: nomes `mustel*` livres no crates.io (HTTP 404); `LICENSE` é **GPL-3.0** enquanto o README afirma MIT — corrigir e confirmar SPDX (Q9)
- `[2026-10-14]` Q1 (integração GUI) e Q6 (escopo do 1.0) seguem abertas — discussão em andamento na thread
- `[2026-10-14]` após discussão da integração: recomendação revisada para **dois binários** (`mustel` sem egui + `mustel-gui`); 4.3, D9, T41 e T44 atualizados; Q1 passa a pedir confirmação do par (costura + split de binários)
- `[2026-10-14]` Q1/Q6/Q9/Q10 respondidas: **binário único** com subcomando `gui` (Q1, revertendo a recomendação anterior), escopo = grupos `query`, `database` e `settings` completos (Q6), licença `GPL-3.0-only` (Q9), `mustel-testkit` com `publish = false` (Q10)
- `[2026-10-14]` plano reestruturado para o escopo decidido: 7 fases (0–6), 39 tarefas em escopo e 9 no backlog (seção 10); `legacy/` deixa de ser removido — T48 reescrito como congelamento
- `[2026-10-14]` correção de fato verificada no código: o FurLab tem **24** comandos-folha em 13 branches (não 25); em escopo ficam 11 (`query run`, `database backup|restore|pgpass ×3`, `settings db-servers ×5`)
- `[2026-10-14]` Q11/Q12 respondidas: backlog é **segunda onda** após o 1.0 (D21); pacote winget antigo `FurLab.CLI` não é tocado (D22); notarização macOS segue fora com instrução de quarentena (D23). **Nenhuma questão aberta restante** — plano pronto para execução, aguardando autorização do T1
- `[2026-10-14 23:02]` Execução iniciada pelo mustel-apply (autorização do usuário). T1: branch `feat/rust-migration` criada a partir de `main`; `FurLab.Core|CLI|Tests|slnx` + `version.json` movidos para `legacy/` com `git mv` (125 arquivos, todos como renames — sem perda de histórico)
- `[2026-10-14 23:03]` **T1 concluída** — workflows `build.yml` e `publish.yml` apontados para `legacy/`; `dotnet build` + `dotnet test legacy/FurLab.slnx` verdes (137 testes). Desvio registrado: `git status` **não** está limpo porque nenhum commit foi feito (guard rail da skill `mustel-apply`: commit só sob pedido explícito do usuário); a árvore contém apenas as mudanças pretendidas
- `[2026-10-14 23:03]` achado bloqueante no legado, não previsto no plano: **NU1903** (advisory GHSA-r955-8q5j-3h83 / CVE-2026-96958, publicada em 13/10/2026) reprovava o build porque `TreatWarningsAsErrors=true`. Corrigido subindo `Microsoft.AspNetCore.DataProtection` 10.0.7 → **10.0.13** em `FurLab.Core` e `FurLab.Tests` (versão corrigida do transitivo `System.Security.Cryptography.Xml`). Exceção consciente: o legado é congelado, mas precisa compilar — é o oráculo da migração
- `[2026-10-14 23:05]` **T2 concluída** — workspace Cargo (resolver 3, edition 2024, `rust-version 1.98`, `license = GPL-3.0-only`), `rust-toolchain.toml` pinando 1.98.1, `rustfmt.toml`, crates `mustel` (bin), `mustel-core`, `mustel-gui`, `mustel-testkit` (`publish = false`); `.gitignore` com `/target/`. Verificado: `cargo build --workspace`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (0 warnings) e `target/release/mustel --version` → `mustel 1.0.0`
- `[2026-10-14 23:12]` **T3 concluída** — `CLAUDE.md` reescrito para o stack Rust (com seção do legado congelado, escada de paridade e exit codes) e `AGENTS.md` resumido; `winget-publish.ps1` agora usa `Mustel.CLI`/`mustel.exe`/`Publisher: Mustel`/`- mustel` (sintaxe validada com `Parser::ParseFile`); `mustel.example.jsonc` com o schema real (sem senha em claro, sem `outputFormat`/`commandTimeout`, que o modelo nem tinha); aviso de migração nos dois READMEs; `.vscode/launch.json`+`tasks.json` apontando para `legacy/`. Ressalva: `format.ps1`/`publish.ps1`/`dotnet-tools.json` foram **movidos** para `legacy/` em vez de removidos/format.ps1 — reconcilição com o T48 (escrito depois, já com o legado preservado); registrado aqui por ser desvio do texto original do T3
- `[2026-10-14 23:12]` lista branca do grep de resíduos criada em `oracle/known-divergences.md` (o critério da seção 6 aponta para esse arquivo); resíduos `furlab` restantes são apenas: URL do repositório (D18), docs de módulo, workflows/VS Code do legado, skill `furlab-command-manager` (declarada legacy-only no `CLAUDE.md`) e as specs/docs fora do escopo do grep
- `[2026-10-14 23:20]` **T4 concluída** — `mustel-core::config` (`model.rs`, `paths.rs`, `jsonc.rs`, `mod.rs`): JSONC com stripper consciente de strings, defaults/validações idênticos ao `UserConfigService`, busca sem diferenciar maiúsculas, escrita atômica (temp + rename) e `MUSTEL_CONFIG_DIR` para isolar testes/oráculo. 18 testes unitários + 6 de integração; `cargo test -p mustel-core`, `fmt --check` e `clippy -D warnings` verdes. Decisão nova: `config_dir_with()` puro + acesso ao env só na borda, porque `unsafe_code = "forbid"` impede `env::set_var` em teste
- `[2026-10-14 23:26]` **T5 concluída** — `mustel-core::error` com `CoreError` (thiserror) e os exit codes observados no oráculo (`0,1,10,11,20,21,22,30,40,41,50,130`, tabela testada caso a caso) + `mustel-core::signal` com `CancellationToken` e listener de Ctrl+C; `mustel/src/exit.rs` traduz erro → mensagem + código, e o `main` usa `tokio::select!` para sair com `130` quando Ctrl+C chega
- `[2026-10-14 23:35]` **T6 concluída** — árvore clap completa em `crates/mustel/src/cli.rs` (11 comandos do escopo + `gui`), com flags curtas/longas copiadas das classes `*Settings` do legado; despacho em `commands/{mod,query,database,settings}.rs` com stubs que falham com exit `1`; 8 testes de CLI (help de cada comando, flags do contrato, stubs, versão, 2 snapshots `insta`). Desvio registrado: testes usam `std::process::Command` + `CARGO_BIN_EXE_mustel` em vez de `assert_cmd`/`predicates` (D11) — evita API depreciada e uma dependência
- `[2026-10-14 23:35]` ressalva para o próximo agente: os snapshots `crates/mustel/tests/snapshots/*.snap` são gerados por `insta` e **precisam ser versionados** (commit); se sumirem, regenerar com `INSTA_UPDATE=always cargo test -p mustel`. Nada foi commitado ainda (guard rail da skill)
- `[2026-10-14 23:35]` divergência doc×código confirmada mais uma vez: o README documenta `settings db-servers add -i`/`-W mypass` e `database pgpass add <banco>`, mas o código não tem `-i`/`-W` em `add` (senha entra por `set-password`) e usa `--database` (não posicional) em `pgpass add` — a árvore clap seguiu o **código**
- `[2026-10-14 23:50]` **correção de rota do usuário sobre a divergência acima**: os docs descrevem a interface **pretendida** — `-i` = interativo, `-W` = senha, e `pgpass add <banco>` posicional é a forma correta. Aplicado em `cli.rs`: `settings db-servers add` ganhou `-i/--interactive` e `-W/--password`, e `database pgpass add` aceita o banco como posicional (com `--database` mantido por compatibilidade, formas mutuamente exclusivas). Registrado em `oracle/known-divergences.md` §2 item 5 como **adição aceita** — o harness não deve tratar isso como divergência N1
- `[2026-10-14 23:50]` commits feitos a pedido do usuário: `dbaaed7` na branch (`feat(migration): bootstrap Mustel (Rust) with FurLab frozen as the parity oracle`, 170 arquivos) e `328acdc` no `main` (`fix(deps): bump Microsoft.AspNetCore.DataProtection to 10.0.13 (NU1903)`) — a correção de segurança do legado foi isolada no `main` para o binário .NET voltar a compilar; **nada foi enviado para o remoto** (sem autorização de push)
- `[2026-10-14 23:55]` **T7 concluída** — `mustel-core::credentials` com `trait CredentialStore` (`set`/`get`/`delete`), `SystemKeyring` (serviço `mustel`, chave = nome do servidor) e `InMemoryCredentialStore` para teste/dublê; erros classificados em `Unavailable` (sem cofre) vs `Store`; `docs/pt-BR/credenciais.md` explica onde a senha vive, que o blob DPAPI **não** é migrado e que sem cofre o caminho é `-W/--password` por execução
- `[2026-10-14 23:55]` **achado silencioso que o teste gated pegou**: `keyring` 3.x **não habilita backend por padrão** — sem features por plataforma ele usa um store mock que aceita `set` e devolve `None` no `get` (o teste de round-trip falhou e revelou isso). Corrigido com dependências por alvo: `windows-native`, `apple-native`, `sync-secret-service`+`crypto-rust`; sem backend nativo, a operação falha com mensagem clara. Reexecutado com `MUSTEL_KEYRING_TESTS=1`: round-trip real contra o Windows Credential Manager **verde**, sem resíduo no cofre. Também anotado no `CLAUDE.md`
- `[2026-10-14 23:55]` estado da árvore: 37 testes passando (`mustel-core` 21 unit + 6 integração; `mustel` 1 unit + 9 CLI), `fmt --check` e `clippy -D warnings` limpos. Próximas tarefas da fase: T8 (primitivas de UI), T9/T10 (harness do oráculo), T11 (baseline de perf), T12 (gate)
- `[2026-10-15 00:20]` **T9 concluída** — harness de captura em `mustel-testkit` (`capture.rs`, `cases.rs`, `config_sandbox.rs`, `paths.rs`) + bin `oracle-capture`, que publica o legado em `oracle/bin` (com `-p:GeneratePackageOnBuild=false`) e grava **27 capturas** em `oracle/baseline/`. Cada captura guarda argv, exit code, stdout/stderr como texto lossy **e em base64** (a saída de tabela do legado não é UTF-8: 414 bytes em codepage do console), e artefatos com SHA-256 + conteúdo. A config real do usuário é salva/semeada/restaurada (`Drop` garante a restauração) — verificado: o `furlab.jsonc` do usuário continua com o servidor `dev2` e o blob DPAPI dele
- `[2026-10-15 00:20]` **fatos de paridade revelados pela captura** (registrados em `oracle/README.md` e usados na implementação): (1) o legado escreve **tudo em stdout**, inclusive erros — `stderr` vazio em 7/7 casos; (2) erro de uso sai com **1**, não 2 (clap remapeado em `handle_parse_error`); (3) `query run` com arquivo ausente sai com **1** e mensagem `SQL input file not found:` (exceção própria do legado — o `Program.cs` mapearia `FileNotFoundException` para 22); (4) `rm` inexistente: `Error: Server 'x' not found.` exit **41** (mensagem do `ConfigError` alinhada); (5) a config criada na primeira execução usa **CRLF** no Windows → `native_newlines` na escrita; (6) senha ausente em modo não-interativo faz o legado **quebrar** (`Failed to read input in non-interactive mode`) com exit 1/40 — divergência deliberada: o Mustel falha claro e sugere `-W`/`set-password`
- `[2026-10-15 00:20]` regressão do T1 corrigida: mover o `.NET` para `legacy/` quebrou `..\icon.png`/`..\LICENSE`/`..\README.md` no `FurLab.CLI.csproj`, e como o Release empacota (`GeneratePackageOnBuild`), isso quebrava o `dotnet build -c Release` — inclusive o CI. Caminhos corrigidos para `..\..\` e publish do oráculo sem packing. Verificado: `dotnet build legacy/FurLab.slnx -c Release` verde
- `[2026-10-15 00:20]` 43 testes passando no workspace; `fmt --check` e `clippy -D warnings` limpos. Ressalva de método: o harness usa heredoc/anchors de uma linha ao editar porque blocos multi-linha falharam sem causa aparente — registrado para o próximo agente
- `[2026-10-15 01:05]` **T10 concluída** — `mustel-testkit::compare` + `report` + bin `oracle-compare`, com a política: exit code e artefatos são contrato (N1/N2); stdout/stderr é aviso N4 (pulado quando a saída do oráculo não é UTF-8); casos de ajuda comparam o conjunto de flags longas (flag do oráculo ausente = falha, extra = informativo); comando ainda stub é pulado. Roda com `--scope <texto>`, gera `oracle/reports/<ts>/report.md` e sai != 0 em falha. Workflows `ci.yml` (fmt/clippy/test em ubuntu/windows/macos + job do legado em Release, que é o que pega quebra de empacotamento) e `compat.yml` (captura + comparação em windows e ubuntu, com upload do relatório)
- `[2026-10-15 01:05]` **o baseline estava dependente da máquina** e foi corrigido: (a) casos marcados como `Unchanged` rodavam com a **config real do usuário** presente — o comportamento do `restore` mudava conforme houvesse senha salva; agora todo caso fixa o estado (`config_seed` = `absent`/`fixture`); (b) mensagens carregavam `C:\Users\<usuário>\...`; o capture passou a redigir para `<WORKDIR>`/`<REPO>`/`<HOME>`/`<LEGACY_CONFIG>` (os dois do meio são necessários porque o console do legado **quebra a linha em 80 colunas no meio do caminho**, furando a substituição literal). Verificado: duas capturas seguidas geram o mesmo hash e não sobra caminho da máquina no baseline
- `[2026-10-15 01:05]` passo de CI "baseline não pode mudar" foi **removido** de propósito: quebra de linha do console depende de largura/comprimento de caminho, então estabilidade byte a byte entre SOs diferentes não é uma afirmação verdadeira; o que vale entre máquinas é o contrato, e o relatório fica como artefato
- `[2026-10-15 01:05]` estado: **53 testes** passando, `fmt --check` e `clippy -D warnings` limpos, `oracle-compare` com **0 falhas** (20 avisos N4 + 7 pulados). Faltam na Fase 0: T8 (primitivas de UI), T11 (baseline de perf), T12 (gate)
- `[2026-10-15 01:40]` **T8 concluída** — `crates/mustel/src/ui/` com `prompts` (seleção simples, multi-seleção com pré-selecionados, confirmação, texto, senha com máscara — todos sobre `inquire`, com `Ctrl+C`/`Esc` virando `Cancelled` = exit 130), `table` (tabela arredondada via `tabled`), `progress` (spinner/barra; **oculto** fora de terminal, em vez de sujar stdout redirecionado) e `style` (cor só em terminal). Regra central: nada trava sem terminal — `ensure_interactive` devolve `InvalidState` (exit 40) com mensagem dizendo o que fazer, enquanto o legado **quebrava** nesse cenário
- `[2026-10-15 01:40]` decisão estrutural tomada no T8: o crate `mustel` passou a ser **lib + bin** (`src/lib.rs` + `main.rs` fino). Motivo: os testes passam a exercitar a árvore e as primitivas pela biblioteca (sem depender de processo) e código ainda não usado pelos comandos não vira erro de `dead_code` com `clippy -D warnings`. `exit::format_error` foi separado de `report_error` pelo mesmo motivo (mensagem testável)
- `[2026-10-15 01:40]` ressalvas honestas do T8: (a) `owo-colors` (listado em D11) **não** foi usado — o estilo ficou com `console::Style::for_stdout()`, que decide cor/texto puro sozinho, e a dependência foi removida para não virar peso morto; (b) os 4 testes de PTY são **Unix-only** (no Windows o `expectrl` depende de `conpty`) e **não** foram executados nesta máquina — a validação real acontece no job ubuntu do CI, e no Windows a cobertura interativa é smoke manual; (c) o `tabled` 0.22 exige `Builder` para dados dinâmicos (`Table::new` não aceita `Vec<Vec<String>>`)
- `[2026-10-15 01:40]` estado: **66 testes** passando (`mustel-core` 22 unit + 6 integração + 11 testkit; `mustel` 8 lib + 2 main + 9 CLI + 4 UI), `fmt --check` e `clippy -D warnings` limpos. Faltam na Fase 0: T11 (baseline de performance) e T12 (gate da fase)
- `[2026-10-15 02:10]` **T11 concluída** — `mustel-testkit::perf` + bin `oracle-perf` (substitui o `hyperfine`, ausente na máquina) mede o processo completo, mediana de 30 execuções, e grava `oracle/perf/baseline.md`. Números do oráculo: `--help` **158,8 ms**, `settings db-servers ls` **206,1 ms**, erro de uso **121,2 ms**. O relatório mede **os dois lados** a cada execução (nada fixado no gerador) e traz o comparativo parcial: `--help` **158,8 ms → 4,6 ms (34,8×)**, muito abaixo da meta de ≤ 1/3. Métricas que dependem de Postgres (CSV de 100k, `database backup`) ficaram para T32/T28
- `[2026-10-15 02:10]` alerta metodológico registrado no relatório de perf: o executável publicado do oráculo é **framework-dependent** (158 KB) e exige o runtime .NET instalado — comparar tamanho com o binário auto-contido do Mustel (1,1 MB) seria maçã com laranja; o comparativo justo é contra o artefato self-contained
- `[2026-10-15 02:15]` **T12 concluída (gate da Fase 0)** — verificações da seção 6 rodadas: `fmt --check` OK; `clippy --all-targets --all-features -D warnings` OK; `cargo build --workspace --release` OK e `mustel --version` = `1.0.0`; **68 testes** passando; `oracle-compare` com **0 falhas** (20 N4 + 7 pulados); grep de resíduos restrito à lista branca de `oracle/known-divergences.md` (atualizada com `ci.yml` e `oracle/**`)
- `[2026-10-15 02:15]` limpeza encontrada pelo gate: sobraram `FurLab.CLI/bin`, `FurLab.Core/bin`, `FurLab.Tests/bin` (+ `obj`, `TestResults`) **na raiz**, resíduos de build anteriores ao `git mv` do T1 (eram ignorados pelo Git, então não foram movidos junto). Nenhum arquivo rastreado ali (confirmado com `git ls-files`), então foram removidos — regeneráveis, e poluíam busca. Ponto de atenção para quem revisar: a limpeza foi feita por serem artefatos ignorados e não rastreados
- `[2026-10-15 02:45]` **T16 concluída** — primeiro comando migrado de verdade: `settings db-servers ls` em `crates/mustel/src/commands/settings/db_servers/list.rs`, com o grupo reorganizado em diretório (`settings/mod.rs` + `db_servers/`) conforme a convenção do plano. Paridade herdada do oráculo: colunas `Name|Host|Port|Username|Databases|Auto-DB`; célula de bancos `(auto)`/lista com `, `/`(default)`; `Auto-DB` `Yes`/`No`; **config vazia não é erro** (exit 0) e imprime orientação
- `[2026-10-15 02:45]` **o comparador passou a ter cobertura real de tabela**: como a saída do legado não é UTF-8, o texto era pulado e o caso mais rico ficava sem verificação. Foi adicionada a checagem **conteúdo (tokens ASCII)** — multiconjunto de tokens imprimíveis dos dois lados, que ignora bordas/largura/codepage e ainda pega "faltou um servidor". Resultado no `ls` semeado: exit code OK, **17 tokens conferem**, artefato de config equivalente (sem `encryptedPassword`) — primeira linha da Fase 1 verificada ponta a ponta
- `[2026-10-15 02:45]` `oracle-compare` passou a comparar em profundidade só comandos de `MIGRATED_COMMANDS` (hoje `settings db-servers ls`) — antes só ajuda/erros determinísticos eram comparados. Ao migrar cada comando, adicione-o lá e remova-o de `STUB_INVOCATIONS` no teste da árvore
- `[2026-10-15 02:45]` divergência nova registrada (item 7 de `known-divergences`): a mensagem de config vazia orienta `'mustel ...'` em vez de `'fur ...'` (nome do binário mudou)
- `[2026-10-15 02:45]` estado: **74 testes** passando, `fmt --check` e `clippy -D warnings` limpos, paridade com **0 falhas** (1 caso em profundidade passando). Performance regerada: `--help` 167,8 ms → 4,4 ms (**37,8×**)
- `[2026-10-15 03:20]` **push feito** (autorizado pelo usuário): branch `feat/rust-migration` publicada em `origin` com tracking. O commit de segurança do legado (`328acdc`) segue **apenas local no `main`** — publicar `main` é decisão do usuário e foi deixada explícita
- `[2026-10-15 03:35]` **T17 concluída** — `settings db-servers add` com os dois caminhos: direto (quando `-n` e `-H` vêm preenchidos) e interativo (quando falta um deles, ou com `-i`), reproduzindo a sequência de prompts do legado (nome, host, porta `[5432]`, usuário `[postgres]`, bancos, confirm de auto-descoberta, padrões de exclusão, seleção de SSL, timeout, paralelismo) e o **menu de ação** de 4 opções (`Save and set password`, `Save and test connection`, `Save without password`, `Cancel`). `-W/--password` guarda no keyring (D6)
- `[2026-10-15 03:35]` **a config escrita pelo Mustel é idêntica à do oráculo**, campo a campo — a única diferença é o `encryptedPassword` que deixamos de ter por decisão (D6), e que o comparador normaliza. Exit codes conferidos: 0 sucesso, **40** nome duplicado, **41** host/porta inválidos, `*` aceito em host/usuário como no legado. `oracle-compare` marca `settings-db-servers-add-flags` como OK em exit code e artefato, com aviso N4 apenas na redação (`mustel` vs `fur`)
- `[2026-10-15 03:35]` novo módulo `mustel-core::validation` (porte fiel de `SecurityUtils`: identificador PostgreSQL, host, usuário, porta, mais `is_port_in_range`) com 6 testes, incluindo a permissividade deliberada com IPv4 (`999.999.999.999` passa, como no legado)
- `[2026-10-15 03:55]` **T18 concluída** — `settings db-servers rm` com os quatro caminhos do legado (sem servidores → 0; `-n` inexistente → 41; `-n` válido → 0; sem `-n` → multi-seleção com mensagem diferente por item). A remoção apaga a credencial do cofre em best-effort (adição do Mustel). Novo caso de baseline `settings-db-servers-rm-ok` comparado: exit OK, **stdout idêntico byte a byte** (primeiro caso em que a redação bate, porque a mensagem não cita o nome do binário) e artefato equivalente
- `[2026-10-15 03:55]` correção no comparador: `error-db-servers-rm-unknown` ficava "pulado" mesmo com o `rm` migrado, porque o casamento por prefixo de slug só cobria casos nomeados a partir do caminho do comando. Agora há mapa explícito erro→comando e motivo explícito para os dois casos que **nunca** serão comparados (divergência deliberada do prompt de senha em modo não-interativo). Resultado: erro do `rm` passa com oráculo=41/mustel=41
- `[2026-10-15 03:55]` estado: **90 testes** passando, `fmt --check` e `clippy -D warnings` limpos, paridade com **0 falhas** (3 comandos comparados em profundidade; 22 avisos N4 e 3 pulados). Próximas tarefas: T19 (spike do driver Postgres), T20 (`test`), T21 (`set-password`), T22 (gate da fase)
- `[2026-10-15 03:55]` instrução do usuário registrada: **não** publicar `main`; o FurLab fica **parado** (a nova versão é o Mustel) e serve apenas como referência/oráculo. A branch `feat/rust-migration` é publicada a cada commit
- `[2026-10-15 04:20]` decisão **D12 tomada pelo usuário: `tokio-postgres`** (não `sqlx`). O T19 deixou de ser spike comparativo e passou a ser implementação com evidência; `docs/rust-postgres-spike.md` registra a decisão, o que está verificado e o que falta
- `[2026-10-15 04:20]` camada Postgres implementada em `mustel-core::postgres`: `tls.rs` (6 modos de `sslMode` do Npgsql, com a diferença sutil `VerifyCA` não valida hostname), `settings.rs` (defaults do legado e tradução para `tokio_postgres::Config`), `value.rs` (`TextValue` + `format_cell`, a parte sensível a paridade do CSV, isolada de propósito) e `client.rs` (conexão sem TLS + erro classificado em `CoreError::Postgres { sqlstate }`, que é o que separa os exit codes 10 e 11). **105 testes** no workspace, `fmt`/`clippy` limpos
- `[2026-10-15 04:20]` **bloqueio de ambiente**: o Docker Desktop está **parado** nesta máquina, então o Postgres de teste não subiu e os testes de integração (escritos em `crates/mustel-core/tests/postgres_integration.rs`, gateados por `MUSTEL_PG_TEST_URL`) **não** foram executados. T19 fica **aberta** por isso — não marco o critério como atendido sem ter rodado. Para destravar: subir o Docker e rodar `MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-core --test postgres_integration`
- `[2026-10-15 04:20]` pendências registradas para o T19/T32: TLS não ligada (`Require`/`VerifyCA`/`VerifyFull` falham com mensagem clara em vez de degradar para texto puro), pool ainda inexistente (campos prontos; biblioteca em T34) e **formato exato do texto do CSV não confirmado contra o oráculo** — `bytea`, `timestamptz` e `numeric` precisam do T32 para fechar
- `[2026-10-15 04:45]` **T21 concluída** — `settings db-servers set-password` com o contrato do legado: sem `name` → seleção de servidor (sem servidores cadastrados → mensagem e exit 0); `name` inexistente → **41** `Server 'x' not found.`; sucesso → `Password saved securely for 'x'.` e exit 0. A senha vai para o **cofre** (D6) e o comando ganhou `-W/--password` para uso em script (o legado só sabia ler do console)
- `[2026-10-15 04:45]` detalhe de paridade que **só o código mostrava** e que meu primeiro teste errou: com um nome informado e config vazia o legado **não** imprime "No servers configured" — ele vai direto ao `GetServer` e devolve 41. A mensagem de config vazia vive dentro da seleção interativa, ou seja, só aparece quando **nenhum** nome é passado. Corrigi o teste (o código já estava certo)
- `[2026-10-15 04:45]` verificação do caminho que escreve no cofre: com `MUSTEL_KEYRING_TESTS=1` o teste passou, confirmando leitura direta sob `mustel/<servidor>` e limpeza depois (`cmdkey /list` sem resíduo). Estado: **110 testes**, `fmt`/`clippy` limpos. Smoke manual conferiu os três caminhos (0, 41 e o prompt abrindo em terminal real)
- `[2026-10-15 05:15]` **T19 e T20 concluídas** — o usuário trouxe o banco de volta e os testes de integração rodaram contra **Postgres 16.15 em container** (porta 55432): 4 verdes, cobrindo conexão, leitura dos tipos que o CSV usa (bool, `int8` no máximo, `float8`, texto, `NULL`, `jsonb`), **senha errada com SQLSTATE `28P01` → exit 10** e servidor inalcançável falhando sem pendurar no timeout. Achado que **corrigiu minha suposição**: falha de autenticação **tem** SQLSTATE, então cai no 10 (regra `SqlState: not null`) — o 11 é para falhas sem SQLSTATE
- `[2026-10-15 05:15]` `settings db-servers test` implementado e verificado ponta a ponta contra o container: sucesso imprime `Testing connection to ...`, `Connection to localhost:55432 successful`, `Authenticated as postgres` e `Databases found: mustel_test, postgres` (exit 0); senha errada → `Authentication failed: ...` (**exit 40**, não 10/11 — o legado embrulha em `InvalidOperationException`); servidor inexistente → 41. **Descoberta de paridade**: o legado **não usa o driver** nesse comando — lista bancos com o **`psql` externo**, exigindo o binário instalado; o Mustel usa o driver com a mesma query (divergência registrada)
- `[2026-10-15 05:15]` **vazamento de credencial encontrado e corrigido**: um `dev.mustel` órfão ficou no Windows Credential Manager porque o teste do `add -W` gravava no cofre real sem limpar. O teste agora apaga o que cria, com asserção de limpeza, e o órfão foi removido à mão (`cmdkey /delete:dev.mustel`). Vale como lembrete: **teste que toca o cofre precisa limpar**
- `[2026-10-15 05:15]` **T22 (gate da Fase 1) concluída** — `oracle-compare --scope settings`: **11 casos, 0 falhas, 0 pulados** (2 em profundidade, 9 N4), e o fluxo manual `add` → `set-password` → `test` → `ls` → `rm` executado contra o container com o cofre limpo no fim. Estado: **114 testes**, `fmt`/`clippy` limpos. Próxima fase: pgpass (T23/T26), depois `database` (T27–T31) e `query run` (T32–T36)
- `[2026-10-18]` **mudança do dono do projeto detectada no repositório**: o commit `3c81ff1` ("Remove GitHub automation and AI prompt scaffolding"), **do próprio usuário e já publicado**, apagou os quatro workflows (`.github/workflows/*`, incluindo o `ci.yml` e o `compat.yml` criados no T10) e o scaffolding de prompts/skills do OpenSpec. Registrado como **D24** e **não revertido** — foi decisão explícita do dono. Efeitos no plano: o gate de paridade e os checks de qualidade passam a rodar **à mão**; os critérios de T10 e T44 foram ajustados (o T44 agora tem uma decisão em aberto: **como** o release é produzido, já que não há mais matrix de Actions)
- `[2026-10-18]` **T23 concluída** — `database pgpass add|list|remove` portado para `mustel-core::pgpass` (`entry.rs` com parse/serialize/escape, `paths.rs` com o caminho por SO, `store.rs` com add/list/remove) e `mustel/src/commands/database/pgpass/`. O grupo `database` virou diretório (`database/mod.rs` + `pgpass/`), como o plano previa. 28 testes novos no core, 28 no CLI (validação extraída em funções puras para os testes **não** tocarem o arquivo real)
- `[2026-10-18]` **fatos de paridade que só o oráculo mostrou** (todos com caso no baseline): (a) o legado **não** aceita `pgpass add <banco>` posicional — responde `Error: Unknown command 'mydb'.`; a forma posicional é a adição do item 5 de `known-divergences.md`, então nenhum caso do baseline a usa; (b) os três erros de validação do `add` saem com **exit 2** vindo do corpo do comando (o parser sai com 1, e o mapeamento de `CoreError` daria 41); (c) `add` **anexa** e um arquivo sem quebra de linha no fim deixa a entrada nova **colada** na anterior — o caso `database-pgpass-add-appends-to-a-file-without-final-newline` prova isso byte a byte (78 bytes); (d) `remove` de entrada inexistente sai com **0**, e a remoção da última entrada deixa o arquivo **vazio** (0 bytes, não uma linha em branco); (e) `--password ""` é ausência e dispara o prompt, enquanto `--password " "` é senha válida
- `[2026-10-18]` **o legado ignora `%APPDATA%`**: verificado rodando `pgpass list` duas vezes com `%APPDATA%` apontando para dois diretórios semeados diferentes — as duas vezes ele leu o arquivo **real** do usuário, porque `SpecialFolder.ApplicationData` vem do registro do Windows. O harness precisou de um sandbox próprio: `LegacyPgPassSandbox` faz backup do arquivo real, semeia o fixture por caso, captura e restaura (e grava uma cópia em `oracle/workdir/legacy-pgpass.backup` para o caso de morte violenta). O `oracle-capture` agora **falha** se um caso de `pgpass` não semear o arquivo, senão o `pgpass.conf` real do usuário (que tem credencial de verdade) iria para o baseline versionado. Verificado depois das capturas: hash do arquivo real **igual** ao de antes
- `[2026-10-18]` novo artefato no comparador: `pgpass` é comparado **byte a byte** (o critério do T23 é o arquivo resultante idêntico), com detalhe mostrando quebras de linha escapadas quando difere. `compare_case` passou a receber `MustelArtifacts { config, pgpass }` em vez de um caminho único
- `[2026-10-18]` **T26 (gate do pgpass) concluída** junto: `oracle-compare --scope pgpass` → **16 casos, 0 falhas, 0 pulados** (12 em profundidade com artefato byte a byte, 4 N4 de ajuda). Suíte inteira: **40 casos, 0 falhas, 22 N4, 3 pulados** — os pulados são as duas divergências deliberadas do prompt de senha e o `error-query-input-missing`, que só fecha no T32. **Correção de plano**: T27 não é pré-requisito deste gate (o `pgpass` não executa processo externo); o gate fechou sem ele
- `[2026-10-18]` **teste Unix executado em container** (o único jeito de provar o `0600` numa máquina Windows): crate de verificação apontando `#[path]` para `crates/mustel-core/src/pgpass/mod.rs`, rodado com `docker run --rm -v <worktree>:/repo:ro rust:1-bookworm cargo test --lib` → **28 testes verdes**, incluindo `the_file_is_0600_on_unix`. Antes disso o `cargo check --target x86_64-unknown-linux-gnu` do workspace era impossível nesta máquina (o `libdbus-sys` do keyring exige `libdbus-1-dev`), por isso a verificação contornou o keyring com o crate de verificação
- `[2026-10-18]` **vazamento de credencial evitado no caminho de teste**: `crates/mustel/tests/cli_tree.rs` invocava o binário sem isolar nada, então os novos comandos de `pgpass` (e os de `settings`) escreveriam nos arquivos reais. O helper passou a exportar `MUSTEL_PGPASS_FILE` e `MUSTEL_CONFIG_DIR` para um diretório temporário; os três comandos de `pgpass` saíram de `STUB_INVOCATIONS` e entraram em `IMPLEMENTED_COMMANDS`
- `[2026-10-18]` estado: **164 testes** passando no workspace (68 no `mustel-core`, 28 no `mustel`, 28 no `mustel-testkit`, mais 40 de integração/CLI — fora os de PTY, que são Unix-only — e a integração do Postgres, que segue gateada por `MUSTEL_PG_TEST_URL`), `fmt --check` e `clippy -D warnings` limpos, paridade com **0 falhas**. Próxima fase: Fase 3 (`database backup`/`restore`, T27–T31)
- `[2026-10-18]` **T27 concluída** — infra de processos em `mustel-core::process` (runner) e `postgres/{locator,password}.rs`, com 25 testes novos (9 do runner, 8 do localizador, 8 da senha). O runner recebe `Vec<String>` de argumentos e **nunca** passa por shell; `stdout`/`stderr` são drenados em paralelo (o `DatabaseBackupCommand` do legado lia um cano antes do outro, o que trava — há teste que enche os dois com 40 000 linhas); falha de partida virou **tipo** (`ProcessError`) em vez de `ExitCode = -1`, porque o backup precisa separar "binário não encontrado" (exit 3) de "pg_dump falhou" (exit 1)
- `[2026-10-18]` **kill tree no cancelamento, verificado em execução no Windows**: o teste sobe um filho `powershell` que cria um neto `ping` e grava o pid dele em arquivo, cancela o token e confere com `tasklist` que o neto **morreu** (`taskkill /T /F`). No Unix o filho abre o próprio grupo (`process_group(0)`) e o grupo leva `SIGKILL` via utilitário `kill` — `libc::kill` não é opção porque o workspace tem `unsafe_code = "forbid"`
- `[2026-10-18]` **achados de ambiente nesta rodada**: (a) o `cmd /c` se comporta de forma imprevisível com saída redirecionada (trava esperando entrada) — os testes do runner passaram a usar **PowerShell** como programa filho no Windows, sempre com o script como **um** argumento; (b) o Docker Desktop estava **parado**, então os 2 testes `#[cfg(unix)]` do runner (argumentos não interpretados por shell; `SIGKILL` no grupo) **não** foram executados — o caminho `cfg(unix)` foi verificado por `cargo check --target x86_64-unknown-linux-gnu` (crate de verificação apontando `#[path]` para os módulos reais, sem o keyring) e a execução fica pendente para quando o Docker voltar
- `[2026-10-18]` detalhes de paridade fixados nos testes: o prompt do backup/restore é `Enter password: ` (**diferente** do `Password: ` do `pgpass add` e do `set-password`); o servidor é casado por **host sem diferenciar maiúsculas + porta como texto**, sem olhar o nome (dois servidores no mesmo host/porta são indistinguíveis, como no legado); branco conta como ausente nas duas primeiras fontes e o valor do prompt é usado **como veio**; a busca de binários mantém as três etapas do legado na mesma ordem, com as subpastas de versão visitadas em ordem alfabética (o `Directory.GetDirectories` devolve a ordem do FS, que no NTFS também é alfabética)

---

## 10. Backlog — grupos de comando fora desta migração

Decisão do usuário (Q6): a migração cobre `query`, `database` e `settings` **completamente**. Os grupos
abaixo **não** entram nesta primeira onda: continuam no FurLab .NET (`legacy/`, congelado) e serão migrados
numa **segunda onda, depois do 1.0** (Q11/D21). Ficam registrados aqui com o critério já definido, para que
essa segunda onda comece do zero em vez de redescoberta.

| ID | Tarefa | Critério resumido |
|----|--------|-------------------|
| T13 | `file combine` | `-i` (glob) e `-o`; saída byte-idêntica ao oráculo |
| T14 | `file convert-encoding` | `--from/--to/--backup/--text-only/-o`; detecção de encoding equivalente; mtime/permissões preservados |
| T15 | Gate — `file` | `oracle-compare` filtrado em `file` sem divergências N1–N3 |
| T24 | `claude install` + `claude settings mcp-database\|win-env` | JSON resultante byte-idêntico |
| T25 | `opencode settings mcp-database\|default-model` | Precedência `--global`/local equivalente |
| T30 | `docker postgres` | Imagem, porta, volume e variáveis iguais às atuais |
| T37 | `winget backup\|restore` | JSON de backup idêntico; argumentos equivalentes |
| T38 | `windowsfeatures export\|import\|list` | Parsing do `dism.exe` equivalente; JSON idêntico |
| T39 | Gate — Windows | `oracle-compare` filtrado em `winget\|windowsfeatures` |

Consequências registradas: (a) o FurLab .NET permanece publicado e congelado para esses comandos até a
segunda onda;
(b) telas da GUI para esses grupos ficam fora do MVP (T42); (c) `legacy/` não é removido (T48);
(d) as specs OpenSpec desses grupos (`docker-postgres`, `opencode-default-model`) permanecem intactas.
