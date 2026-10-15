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
| D12 | Driver Postgres                          | Decidido por spike (T19): `sqlx` preferido, `tokio-postgres` como fallback | Tipos dinâmicos → CSV e opções de pool exigem validação empírica     |
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
└── .github/workflows/         ← ci.yml, compat.yml, release.yml
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
- `.github/workflows/` — `build.yml`→`ci.yml` (Rust), novo `compat.yml` (oráculo .NET vs mustel), `publish.yml`→`release.yml` (matriz 4 alvos + SHA256).
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

- [ ] **T7** — Credenciais no `mustel-core` (keyring)
  - Depende de: T2
  - Paralelizável com: T3, T4, T5, T6, T8
  - Concluído quando: `trait CredentialStore` + impl keyring (service = `mustel`, user = nome do servidor) com testes de round-trip; ausência de Secret Service em Linux retorna erro claro e documentado (fallback registrado em `docs/`); nenhuma senha é escrita em arquivo
  - Arquivos: `crates/mustel-core/src/credentials/**`, `crates/mustel-core/Cargo.toml`

- [ ] **T8** — Primitivas de UI do terminal (paridade de prompts)
  - Depende de: T2
  - Paralelizável com: T3, T4, T5, T6, T7
  - Concluído quando: existem seleção simples, multi-seleção com itens pré-selecionados, confirmação (default y/n), texto livre, senha com máscara, tabela e progresso/spinner; comportamento de teclas equivalente ao Spectre; snapshots (insta) e teste PTY (`expectrl`) de cada widget verdes; TTY ausente → mensagem clara em vez de bloqueio
  - Arquivos: `crates/mustel/src/ui/**`, `crates/mustel/tests/pty_prompts.rs`

- [ ] **T9** — Oráculo: captura de baseline do binário .NET
  - Depende de: T1
  - Paralelizável com: T2, T3, T4, T5, T6, T7, T8, T10, T11
  - Concluído quando: `cargo run -p mustel-testkit --bin oracle-capture` publica o `legacy/fur` e grava, para cada comando do escopo, `--help`, execuções com fixtures não destrutivas, exit codes e artefatos em `oracle/baseline/`; fixtures versionados; README do harness em `oracle/README.md`
  - Nota (descoberto ao implementar T6): incluir também **argumentos inválidos** (ex.: `database pgpass remove` sem o posicional) para fixar o exit code de erro de parse — o clap devolve `2`, e o Spectre pode devolver outro valor (teste `missing_required_argument_is_a_usage_error`)
  - Nota (isolamento): o legado fixa a config em `%LOCALAPPDATA%\FurLab\furlab.jsonc` via `Environment.SpecialFolder.LocalApplicationData`, que **não** respeita variável de ambiente no Windows. O harness precisa salvar/restaurar esse arquivo em volta das capturas que escrevem (ou capturar só leitura) para não tocar a config real do usuário
  - Arquivos: `crates/mustel-testkit/**`, `oracle/baseline/**`, `oracle/README.md`

- [ ] **T10** — Harness de comparação + CI
  - Depende de: T2, T9
  - Paralelizável com: T3, T4, T5, T6, T7, T8, T11
  - Concluído quando: `oracle-compare` compara exit code, stdout/stderr normalizados (`FurLab|fur` ↔ `mustel`) e artefatos byte a byte, classifica divergências N1–N3 (falha) e N4 (aviso), gera `oracle/reports/<ts>/report.md` e sai != 0 em falha; `ci.yml` (fmt/clippy/test em 3 SOs) e `compat.yml` (oráculo em windows + ubuntu) rodam verdes com o esqueleto stub
  - Arquivos: `crates/mustel-testkit/src/{oracle.rs,normalize.rs}`, `.github/workflows/{ci.yml,compat.yml}`

- [ ] **T11** — Baseline de performance do .NET
  - Depende de: T9
  - Paralelizável com: T3, T10
  - Concluído quando: `oracle/perf/baseline.md` registra, com metodologia e desvio: startup de `--help` (mediana de 20 execuções, `hyperfine --warmup 3`), `settings db-servers ls`, export CSV de 100k linhas, `database backup` de um dump de fixture e tamanho do `fur.exe` self-contained
  - Arquivos: `oracle/perf/baseline.md`, `oracle/perf/scripts/**`

- [ ] **T12** — Gate — fundação
  - Depende de: T1–T11
  - Paralelizável com: —
  - Concluído quando: `cargo fmt --check`, `cargo clippy -D warnings` e `cargo test --workspace` verdes; `mustel --help` cobre a árvore completa; `oracle-compare` roda e reporta 0 divergências N1–N3 nos comandos ainda não implementados (stubs), com exceções listadas em `known-divergences.md`
  - Arquivos: —

### Fase 1 — Configuração e credenciais (`settings db-servers`)

- [ ] **T16** — `settings db-servers ls`
  - Depende de: T4, T6, T9, T10
  - Paralelizável com: T19
  - Concluído quando: lista igual à do oráculo para configs de fixture (inclusive config vazio e servidor sem senha); exit codes iguais; spec `openspec/specs/settings-db-servers` coberta
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/list.rs`

- [ ] **T17** — `settings db-servers add` (flags + interativo)
  - Depende de: T4, T6, T8, T16
  - Paralelizável com: T19
  - Concluído quando: flags (`-n/--name`, `-H/--host`, `-p/--port`, `-U/--username`, `-D/--database`...) e modo interativo com paridade N1/N3; validações de host/porta/usuário equivalentes (incluindo `*`); `mustel.jsonc` resultante byte-idêntico nos fixtures; spec `settings-user-config` coberta
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/add.rs`

- [ ] **T18** — `settings db-servers rm`
  - Depende de: T17
  - Paralelizável com: T19
  - Concluído quando: remoção por nome (case-insensitive), confirmação/mensagens e exit codes equivalentes; config resultante idêntica; remoção também limpa a credencial no keyring (comportamento documentado se o oráculo divergir)
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/remove.rs`

- [ ] **T19** — Spike do driver Postgres (decide D12)
  - Depende de: T2
  - Paralelizável com: T16, T17, T18
  - Concluído quando: `docs/rust-postgres-spike.md` registra a decisão (`sqlx` vs `tokio-postgres`) com evidência para: (a) mapeamento tipo→texto de resultados arbitrários (`int4`, `numeric`, `timestamptz`, `bool`, `json/jsonb`, `bytea`, arrays, `uuid`, `NULL`), (b) opções de pool equivalentes (`pooling`, `min/max pool size`, `keepalive`, `connection lifetime`), (c) `sslmode` `Disable|Allow|Prefer|Require|VerifyCA|VerifyFull`, (d) timeout de conexão/comando; testes de integração com Postgres em container cobrindo (a)–(d)
  - Arquivos: `docs/rust-postgres-spike.md`, `crates/mustel-core/src/postgres/**`, `crates/mustel-core/tests/postgres_integration.rs`

- [ ] **T20** — `settings db-servers test`
  - Depende de: T19
  - Paralelizável com: T16, T17, T18
  - Concluído quando: conecta com as mesmas credenciais/parâmetros; sucesso e falhas (host inválido, senha errada, banco inexistente, timeout) produzem as mesmas classes de exit code e mensagens equivalentes do oráculo
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/test.rs`

- [ ] **T21** — `settings db-servers set-password` (keyring)
  - Depende de: T7, T17
  - Paralelizável com: T19, T20
  - Concluído quando: prompt de senha com máscara, grava no keyring, `mustel.jsonc` não contém senha em claro nem blob DPAPI; `query run`/`database` passam a usar a senha do keyring; spec `credential-storage` coberta (com a divergência DPAPI→keyring registrada)
  - Arquivos: `crates/mustel/src/commands/settings/db_servers/set_password.rs`

- [ ] **T22** — Gate — `settings db-servers`
  - Depende de: T16–T21
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `settings` sem divergências N1–N3; verificação manual do fluxo `add` → `test` → `set-password` registrada no log
  - Arquivos: —

### Fase 2 — `database pgpass`

- [ ] **T23** — `database pgpass add|list|remove`
  - Depende de: T6, T9, T10
  - Paralelizável com: T27
  - Concluído quando: `pgpass.conf` no mesmo caminho por SO do oráculo, com curingas `*` aceitos para host/porta/usuário; arquivo resultante byte-idêntico; permissões `0600` em Unix; specs `pgpass-cli-setup` e `pgpass-wildcard-validation` cobertas
  - Arquivos: `crates/mustel/src/commands/database/pgpass/*.rs`, `crates/mustel-core/src/pgpass/**`

- [ ] **T26** — Gate — `pgpass`
  - Depende de: T23, T27
  - Paralelizável com: —
  - Concluído quando: `oracle-compare` com filtro `pgpass` sem divergências N1–N3
  - Arquivos: —

### Fase 3 — Processos externos: `database backup` e `restore`

- [ ] **T27** — Infra de processos e Postgres local no `mustel-core`
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
  - Concluído quando: `release.yml` produz `mustel.exe` (win-x64), `mustel` (linux-x64 musl) e `mustel` (macOS x64/arm64) com SHA256 no corpo da release e validação `winget validate` do manifesto (`Mustel.CLI`, installer `portable`, command `mustel`); download do artefato publicado executa `mustel --version`, `mustel --help` e abre `mustel gui` nas 3 plataformas; `cargo publish` (dry-run e real) de `mustel-core` → `mustel-gui` → `mustel`, com `license = "GPL-3.0-only"`, `repository` e `description`; `cargo install mustel` instala o binário `mustel` funcionando nas 3 plataformas
  - Arquivos: `.github/workflows/release.yml`, `winget-publish.ps1`, `packaging/winget/**`, `crates/*/Cargo.toml`

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
