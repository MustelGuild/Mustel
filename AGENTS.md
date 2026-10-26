# Mustel — Guia de desenvolvimento e de agentes

> Trabalho ainda **não executado** vive em `MustelPlans/`; o que já foi concluído é consolidado em
> `docs/` e o plano é removido. O contrato de interface do CLI vive em `docs/contrato-de-interface.md`
> e é verificado por `cargo test`.

## Idioma

Todo output para o usuário (explicações, resumos, mensagens de erro de ferramentas, perguntas) deve ser
em **português do Brasil (pt-BR)**. Exceção: não traduza nomes de skills, identificadores de código,
nomes de arquivos ou comandos de terminal.

Todas as **mensagens exibidas ao usuário pelo CLI** (output de comandos, prompts interativos, erros,
confirmações) são escritas em **inglês** — é o contrato de paridade herdado do FurLab (níveis N1–N3 da
escada de fidelidade). O binário fala inglês; a documentação, o plano e a conversa com o usuário do
repositório falam pt-BR.

## Estado do repositório

| Área | O que é |
|------|---------|
| `crates/` | **Mustel** (Rust) — o produto ativo |
| `fixtures/` | baseline **congelado**: as capturas que o gate reexecuta |
| `MustelPlans/` | planos de implementação **em andamento** (consolidados em `docs/` ao concluir) |
| `docs/` | documentação do Mustel em quatro quadrantes (Diátaxis) e o manifesto `contrato-de-interface.md` |

Escopo do produto: grupos **`query`**, **`database`** e **`settings`**, mais a GUI em egui. O código
.NET do produto anterior foi **removido** do repositório em 2026-10-22; o comportamento dele sobrevive
como contrato em `fixtures/baseline/` e em `docs/contrato-de-interface.md`.

## Stack

- Rust 1.98 (pinado em `rust-toolchain.toml`), edition 2024, resolver 3
- CLI: `clap` (derive) · prompts `inquire` · tabelas `tabled` · progresso `indicatif` · estilo `console` + `owo-colors`
- Async: `tokio` (+ `tokio-util` para `CancellationToken`) — **somente para I/O**
- Postgres: `tokio-postgres`, com `with-serde_json-1` · dados exportados com `csv`
- Config: `serde` + `jsonc-parser`, caminho resolvido por `dirs` · credenciais: `keyring` — **com os
  backends nativos habilitados por plataforma** (`windows-native`, `apple-native`,
  `sync-secret-service`); sem essas features o crate usa um store mock que aceita `set` e não persiste
- Erros: `thiserror` no core, `anyhow` apenas no binário · logs: `tracing`
- GUI: `eframe`/`egui` (+ `egui_extras`), atrás da feature `gui`
- Testes: `insta` (snapshots), `expectrl` (PTY), `tempfile`, `tokio-postgres` (integração Postgres)

## Estrutura

```text
crates/
├── mustel-core/     lib: config, credentials, postgres, pgpass, casos de uso, eventos (sem UI)
├── mustel/          bin `mustel`: clap, UI de terminal, comandos finos
├── mustel-gui/      lib egui usada pelo subcomando `gui`
└── mustel-testkit/  fixtures do baseline + comparação de contrato (bins de captura/diagnóstico)
legacy/              (removido em 2026-10-22; recuperável pelo histórico do Git)
fixtures/            baseline/ — as 47 capturas congeladas que o gate reexecuta
docs/                contrato-de-interface.md, tutorials/, how-to/, reference/, explanation/
```

## Comandos

```bash
# Rust (na raiz do repositório)
cargo build --workspace
cargo test --workspace          # é o GATE: inclui a suíte de contrato
cargo fmt --all                 # CI roda com --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p mustel -- --help
./target/release/mustel --version

# Suítes gated (só rodam com a variável)
MUSTEL_KEYRING_TESTS=1 cargo test --workspace
MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-core --test postgres_integration

# Diagnóstico de paridade (não é gate)
cargo run -p mustel-testkit --bin oracle-compare -- --report --scope query,database,settings

# Release (ver docs/how-to/publicar-release.md)
cargo release patch             # ensaio: mostra o bump e o commit/tag que faria (não altera nada)
cargo release patch --execute   # bump + commit + tag v<versao>
```

## Verificar a GUI na janela

O gate automatizado não cobre teclado, foco nem clique de verdade. Quando a mudança for de GUI (e
sobretudo quando um bug só aparecer na janela), siga a skill
[`mustel-gui-smoke-test`](.agents/skills/mustel-gui-smoke-test/SKILL.md): janela real no Windows,
entrada sintética, captura em `scratch://` e o que **não** dá para provar assim.

## Convenções de código

- **Formatação**: `cargo fmt` (default do rustfmt, 100 colunas). O CI reprova qualquer diff.
- **Lint**: `clippy -D warnings` com o `[workspace.lints]` (rustc `unused = deny`, `unreachable_pub`/`missing_docs`; clippy de duplicação e placeholder) e `clippy.toml` para thresholds; `#[allow]` exige `reason`. Decisão e medições em `MustelPlans/lints-fase-1.md`.
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

## Paridade e contrato de interface (o contrato do projeto)

O comportamento do CLI é **contrato**, não sugestão. Ele está em três lugares, nesta ordem de
precedência:

1. `docs/contrato-de-interface.md` — o **manifesto**: uma linha por decisão de superfície, com ID
   (`IF-NNN`), origem e o teste que a prova.
2. `fixtures/baseline/*.json` — as **47 capturas congeladas** do comportamento antigo; o binário .NET
   **não** é necessário para comparar (o `oracle-compare` apenas relê o baseline).
3. [`docs/explanation/diferencas-deliberadas.md`](docs/explanation/diferencas-deliberadas.md) — as
diferenças deliberadas, com justificativa.

A escada de fidelidade (detalhe em
[`docs/explanation/escada-de-fidelidade.md`](docs/explanation/escada-de-fidelidade.md)):

| Nível | Exige | Bloqueia? |
|-------|-------|-----------|
| N1 | mesma árvore de comandos, flags, defaults; inválidos rejeitados do mesmo jeito | sim |
| N2 | mesmos artefatos byte a byte, mesmos exit codes | sim |
| N3 | mesmos fluxos interativos (perguntas, ordem, atalhos, defaults) | sim |
| N4 | formatação textual (`--help`, cores, tabelas, redação de erros) | não — registrar em [`docs/explanation/diferencas-deliberadas.md`](docs/explanation/diferencas-deliberadas.md) |

**O gate é `cargo test --workspace`.** Ele roda o replay das 47 capturas (`contract_replay`), os
snapshots de texto (`contract_text`), a cobertura do manifesto (`contract_manifest`) e a cobertura de
superfície (`contract_surface`). Nenhuma decisão é considerada pronta sem teste; adicionar
comportamento sem linha no manifesto quebra a suíte.

## Exit codes (herdados do oráculo)

`0` sucesso · `1` erro geral · `2` validação de comando · `10`/`11` Postgres (com/sem SQLSTATE) ·
`20`–`22` I/O · `30` acesso · `40`/`41` estado inválido / argumento inválido · `50` timeout ·
`130` cancelado pelo usuário.

Quirks herdados: binário ausente é **3** no `database backup` e **1** no `database restore`; erro de uso
sai em **stdout** com **1**.

## Legado .NET (removido)

- O diretório `legacy/` **não existe mais** (removido em 2026-10-22, sem tag: recuperável pelo
  histórico do Git, no commit anterior à remoção).
- O baseline em `fixtures/baseline/` é **contrato congelado** — não há como re-gravar. Mudar
  comportamento significa mudar um teste, com revisão explícita.
- As skills `furlab-command-manager` valiam apenas para o legado e foram removidas com ele.
- Os pacotes antigos (`FurLab` no NuGet, `FurLab.CLI` no winget) não são tocados. O Mustel **não**
  publica no winget: os canais são o **GitHub Releases** (executável) e o **crates.io**
  (`cargo install mustel`). `mustel-core` e `mustel-gui` vão ao crates.io como dependências do
  `mustel`, não como instrução de instalação.

## Guard Rails (Agent Safety Protocol)

> **Regra obrigatória:** agentes **não** fazem commit, **não** fazem staging (`git add`), **não** criam
> tags e **não** fazem push **sem permissão explícita do dono para aquela ação**. As mudanças ficam na
> árvore de trabalho para ele revisar e confirmar. Permissão dada uma vez serve para aquela ação — não
> vale como autorização permanente para as próximas.

### Pode fazer sem perguntar

Leituras (`git status`, `git log`, `git diff`, `git show`, `git branch -a`, `git rev-parse`), build,
testes, formatação e análise.

### Exige autorização explícita do dono

`git add`, `git commit`, `git tag`, `git push`, `git pull`, `git fetch`, `git merge`, `git rebase`,
`git reset`, `git checkout` (com arquivos modificados), `git stash`, e remoção de arquivos rastreados.

### Como remover arquivos rastreados

Use `rm`/`mv` do sistema de arquivos, **nunca** `git rm`/`git mv` (esses stageiam). A remoção fica
visível como mudança não-staged, e o dono decide o commit.

## Documentação (Diátaxis)

A documentação do produto vive em `docs/` e segue o **Diátaxis**. Antes de escrever qualquer página,
decida o quadrante — a diferença não é o assunto, é a intenção de quem lê:

| O conteúdo… | …e serve para… | …então é | Pasta |
|---|---|---|---|
| informa **ação** | **adquirir** habilidade | tutorial | `docs/tutorials/` |
| informa **ação** | **aplicar** habilidade | how-to | `docs/how-to/` |
| informa **cognição** | **aplicar** habilidade | referência | `docs/reference/` |
| informa **cognição** | **adquirir** habilidade | explicação | `docs/explanation/` |

Regras práticas:

- **Tutorial** ensina: uma lição com começo, meio e fim, que o leitor **executa** e termina com
  sucesso. Nada de "opções avançadas" no meio.
- **How-to** resolve: começa por uma situação concreta ("você precisa X") e vai direto ao caminho.
- **Referência** descreve: árvore de comandos, flags, exit codes, formatos. Fria, completa, sem
  opinião nem passo a passo. Deve bater com `mustel <cmd> --help`.
- **Explicação** justifica: por que keyring e não arquivo, por que o CSV sai como sai, o que é a
  escada N1–N4. Pode ter opinião e contexto.

O que **não** é documentação de produto: o **manifesto de contrato**
(`docs/contrato-de-interface.md`, uma linha por decisão com o teste que a prova) e os **planos**
(`MustelPlans/`). Esses são artefatos de engenharia, não páginas para o leitor.

**Consolidação:** quando um plano é concluído, o que ele documenta — decisões, porquês, o que foi
verificado — vira página em `docs/` **antes** de o plano ser removido, e a página é linkada em
`docs/README.md`. Decisões de engenharia já consolidadas estão em
[`docs/explanation/decisoes.md`](docs/explanation/decisoes.md). `MustelPlans/` não é arquivo
histórico: o histórico é o Git.

Toda página nova precisa de link a partir de `docs/README.md`, e `documentation_links_resolve`
(`cargo test`) falha se um link relativo apontar para arquivo inexistente.

