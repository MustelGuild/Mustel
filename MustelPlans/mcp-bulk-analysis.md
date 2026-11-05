# Plano — Servidor MCP do Mustel para análise em massa

> **Status:** rascunho
> **Criado em:** 2026-11-04
> **Atualizado em:** 2026-11-04
> **Complexidade:** alta
> **Depende de (outros planos):** nenhum

> Este plano foi escrito para ser **refinado antes de executar**. As decisões de produto
> não confirmadas estão na seção 8 (Q1–Q18); a seção 4 é uma **proposta técnica** e a seção 5
> descreve tarefas condicionais a essas decisões. Nenhuma premissa de produto deve ser
> tratada como fechada sem resposta explícita do dono.

---

## 1. Visão Geral

Expor o Mustel como **servidor MCP** para que um cliente (Claude) rode **uma query em vários
bancos** e leia o **CSV** resultante para análise em massa. O motor de execução já existe no
`query_run`: descoberta de bancos, paralelismo em dois níveis, retry, CSV por servidor,
consolidado, `_erros.csv`, `_log.csv`, progresso por evento e cancelamento. O trabalho é,
portanto, um **adaptador de protocolo** em volta desse motor — não um novo motor de execução.

O valor é permitir perguntas analíticas (“quantos pedidos órfãos por banco?”, “comparar o
mesmo indicador entre tenants”) sem abrir a GUI e sem colocar connection strings no arquivo
do cliente MCP: o cadastro em `mustel.jsonc` e o keyring do sistema continuam sendo a fonte
de servidores e senhas. O CSV gerado é o artefato de análise; o cliente precisa conseguir
lê-lo de forma controlada (prévia, recurso e/ou paginação — a decidir em Q5).

Não é um cliente MCP dentro do Mustel, nem um substituto do `query run`; é uma porta de
entrada para o mesmo motor.

---

## 2. Escopo

**Inclui:**
- Servidor MCP (transporte a confirmar em Q12; proposta: stdio), exposto como
  `mustel mcp serve` (nome a confirmar em Q18).
- Uma execução = **uma query + uma lista de bancos**; queries diferentes = execuções
  separadas.
- Resolução de alvos por lista explícita e, se confirmado (Q7), por Query Selector
  (`gui.selections`).
- Reuso do `query_run`/`executor`/`csv_writer` existentes; sem novo motor de fan-out.
- Manifesto de execução, resumo por banco e artefatos CSV.
- Leitura do CSV pelo cliente: prévia embutida, recursos e paginação (mecanismo a confirmar
  em Q5).
- Read-only e limites, se confirmados (Q1/Q6).
- Testes, contrato, documentação e hardening.

**Exclui explicitamente:**
- Escrita (DML/DDL) sem decisão explícita em Q1; se a resposta for “não”, **não implementar**.
- Alterar o formato do CSV (BOM, CRLF, `Server,Database,...`); é contrato.
- GUI nova ou mudanças de UI.
- Backup/restore via MCP.
- `generate_sql` na v1, salvo decisão em Q13.
- Transporte HTTP/SSE na v1, salvo decisão em Q12.
- Credencial por linha do Query Selector, salvo decisão em Q7.
- Substituir o catálogo/DDL por outra fonte (o DDL continua vindo do catálogo, sem
  `pg_dump`).
- Coordenação entre múltiplos clientes/processos; cada processo tem seu próprio registry.

> Se durante a execução surgir algo na lista “Exclui”, **NÃO implementar**. Registrar na
> seção 8 e perguntar ao usuário.

---

## 3. Decisões

Decisões travadas durante a exploração. O agente **não reabre** estas escolhas sem
justificativa explícita e aprovação do usuário.

| # | Decisão | Escolha | Razão |
|---|---|---|---|
| D1 | Superfície | Servidor MCP que expõe o Mustel a um cliente (Claude); **não** um cliente MCP dentro do Mustel | Decisão do dono na conversa |
| D2 | Modelo de execução | Uma execução = uma query + uma lista de bancos; queries diferentes = execuções separadas | Decisão do dono na conversa |
| D3 | Artefato | O resultado é o CSV gerado pelo `query run`; o cliente precisa conseguir lê-lo | Decisão do dono na conversa |
| D4 | Formato do CSV | O CSV existente é contrato (BOM, CRLF, `Server,Database,...`) e não muda | `AGENTS.md` + `docs/contrato-de-interface.md`; mudar exigiria revisão de contrato |
| D5 | Fronteira de crates | O protocolo MCP vive num crate adaptador; `mustel-core` não conhece MCP/UI | `AGENTS.md` (sem UI no core; comandos finos) |

> **Não decidido aqui:** read-only, SDK, empacotamento, cliente alvo, limites, forma de
> leitura do CSV, Query Selector, persistência etc. — ver seção 8. O agente **não** deve
> tratar as propostas da seção 4 como decisões.

---

## 4. Arquitetura / Design

> **Proposta técnica, sujeita à revisão do dono.** As escolhas de produto estão na seção 8.
> Se uma decisão de Q1–Q18 mudar, esta seção é revisada antes da execução.

```mermaid
flowchart TD
    C[Cliente MCP, ex. Claude] -->|JSON-RPC 2.0 em stdio| M[mustel mcp serve]
    M --> A[Crate mustel-mcp]
    A --> U[Novos casos de uso no mustel-core]
    U --> Q[query_run existente]
    U --> R[resolucao servidor para conexao]
    U --> P[policy de leitura]
    U --> CSV[csv_reader]
    Q --> PG[(PostgreSQL)]
    Q --> ART[CSV por servidor + consolidado + erros + log]
    A --> MAN[manifesto e registry de runs]
    CSV --> ART
    C -->|get_bulk_query e read_bulk_csv| A
```

Fluxo de uma execução:

```mermaid
sequenceDiagram
    participant C as Cliente
    participant M as mustel mcp serve
    participant R as RunRegistry
    participant E as query_run
    participant P as PostgreSQL
    C->>M: start_bulk_query query + targets
    M->>M: valida alvos e politica
    M->>R: cria run_id e manifesto
    M->>E: dispara execucao
    M-->>C: run_id e status running
    E->>P: descobre e executa em paralelo
    E->>R: DatabaseFinished
    C->>M: get_bulk_query run_id
    M-->>C: resumo + preview + artefatos
    C->>M: read_bulk_csv run_id, offset, limit
    M-->>C: pagina do CSV
```

### Tipos / arquivos novos

- `crates/mustel-mcp/` — crate adaptador: protocolo, ferramentas, recursos, registry de
  runs, limites e auditoria. É `lib` (como `mustel-gui`) quando o empacotamento for o
  binário único; vira `[[bin]]` se Q3 escolher binário separado.
- `crates/mustel-mcp/src/lib.rs` — ponto de entrada do servidor (`serve(token)`), sem
  `clap`, `inquire`, `indicatif` ou `egui`.
- `crates/mustel-core/src/usecases/connection.rs` — resolução
  `ServerConfigEntry` + senha → `ServerRun` (`server_run`, `server_run_marked`,
  `stored_password`), extraída do CLI e da GUI.
- `crates/mustel-core/src/query/policy.rs` — allowlist de leitura e rejeição de múltiplos
  statements (condicional a Q1).
- `crates/mustel-core/src/query/csv_reader.rs` — paginação/filtro/projeção do CSV,
  reusando o `RecordReader`.
- `crates/mustel-mcp/src/runs.rs` — `RunManifest`, `RunStatus`, `RunTarget`, registry em
  memória + manifesto em disco.
- `crates/mustel-mcp/src/tools/bulk.rs` — `start_bulk_query`, `get_bulk_query`,
  `cancel_bulk_query`, `list_bulk_runs`.
- `crates/mustel-mcp/src/tools/csv.rs` — `read_bulk_csv`, prévia e `get_bulk_errors`
  (se Q5/Q15).
- `crates/mustel-mcp/src/tools/context.rs` — `list_servers`, `list_databases`,
  `get_schema`, `get_ddl` (se Q13).
- `crates/mustel-mcp/src/tools/selectors.rs` — `list_selectors`, `resolve_selector`
  (se Q7).
- `crates/mustel-mcp/src/resources.rs` — `mustel://runs/...` (se Q5).
- `crates/mustel-mcp/src/limits.rs` — caps de servidores, bancos, linhas, tempo e
  concorrência (se Q6).
- `crates/mustel-mcp/src/audit.rs` — JSONL append-only (se Q11).

### Mudanças em tipos existentes

- `crates/mustel-core/src/usecases/query_run.rs` — `QueryRunRequest` ganha `read_only`
  (ou política equivalente) se Q1 confirmar. `label` e outros metadados ficam no
  adaptador, não no core.
- `crates/mustel-core/src/query/executor.rs` — `ExecutionRequest` ganha `read_only`; o
  executor passa a flag para o `QueryStream`.
- `crates/mustel-core/src/postgres/client.rs` — `QueryStream::open` aceita a política e
  abre a sessão com `options=-c default_transaction_read_only=on` e/ou `BEGIN READ ONLY`.
- `crates/mustel-core/src/config/model.rs` — bloco opcional `mcp`
  (include/exclude, limites, retenção), com `skip_serializing_if` para o arquivo
  continuar mínimo.
- `crates/mustel/src/cli.rs` e `crates/mustel/src/commands/mod.rs` — subcomando `mcp`
  (se o empacotamento for no binário único). Com a feature desligada, o comando existe na
  árvore e explica a ausência, como o `gui`.
- `crates/mustel-core/src/query/csv_writer.rs` — tornar o `RecordReader` `pub(crate)`
  ou movê-lo para o novo `csv_reader`.

### Pontos de atenção do código atual

- **Resolvedor duplicado:** o CLI tem `server_run`/`connection_settings`/
  `resolve_password` em `crates/mustel/src/commands/query/settings.rs`; a GUI tem
  `server_run`/`server_run_marked`/`stored_password` em `crates/mustel-gui/src/query.rs`.
  O MCP seria a terceira cópia — extrair para o core é pré-requisito (T4).
- **`fetchAllDatabases`:** o `plan` descobre bancos quando
  `all || fetch_all_databases`. Para uma lista explícita, o `ServerRun` precisa de
  `fetch_all_databases: false` e o `QueryRunRequest` de `all: false`; senão a lista é
  ignorada. É o que `server_run_marked` já faz na GUI.
- **Formato do consolidado:** `Server,Database,` + colunas; os arquivos por servidor
  mantêm as colunas originais. O merge do consolidado faz união de colunas e pode
  “tortear” linhas quando os bancos têm schemas diferentes — documentar.
- **Leitor de CSV:** o projeto não usa o crate `csv`; o `RecordReader` é próprio e cobre
  BOM, CRLF, aspas e newline dentro de campo. Não adicionar dependência nova: expor
  paginação em cima dele. Hoje o `RecordReader` é privado e `parse_records` carrega o
  arquivo inteiro.
- **`_log.csv` e `_erros.csv`:** uma linha por execução, com duração; o `_erros.csv` só
  existe quando há falha. O resumo do MCP pode vir dos eventos `DatabaseFinished`, sem
  reler o log.
- **`NoAccessibleDatabases`:** o `query_run::run` devolve
  `QueryRunOutcome::NoAccessibleDatabases` e exit 0 no CLI; no MCP deve ser um resultado
  “nada a executar”, não um erro de protocolo.
- **Falha parcial:** `query_run::run` emite `FilesWritten` antes de devolver erro quando
  nenhum servidor responde; o adaptador não pode perder os artefatos parciais.
- **Erro de parse no stdout:** `handle_parse_error` em `crates/mustel/src/main.rs` imprime
  erro de uso em **stdout**. Para `mcp`, isso corromperia o protocolo; redirecionar para
  stderr ou garantir que o cliente passe exatamente `mcp serve`.
- **`MUSTEL_CONFIG_DIR` e `MUSTEL_PASSWORD`:** o primeiro isola a config e já é usado pelos
  testes; o segundo é fallback de senha. Documentar o comportamento em Linux sem D-Bus.
- **Query Selector:** `custom_databases::load(client, sql)` roda no servidor/banco de
  origem do seletor; `validate(rows, servers)` devolve `Vec<ServerSelection>`. O caminho de
  execução atual usa a credencial do **servidor**, não a credencial por linha; o MCP não
  deve devolver connection strings nem senhas.

---

## 5. Tarefas

Cada tarefa tem **ID único** para referência cruzada. O agente marca `- [ ]` → `- [x]` ao
concluir e atualiza a seção 9. Tarefas marcadas **condicional** só começam depois da
decisão indicada; se a decisão for “não”, registrar “N/A” na seção 9 e não implementar.

> **Portão de decisão:** a T1 resolve Q1–Q18, priorizando as bloqueantes (Q1–Q8, Q12,
> Q17 e Q18). Nenhuma tarefa condicional começa antes da resposta correspondente; as
> questões de detalhe (Q9–Q11, Q13–Q16) são resolvidas antes da tarefa que delas depende.
> Se uma resposta mudar a arquitetura, revisar este plano antes de seguir.

### Fase 0 — Decisões e spike (bloqueante)

- [ ] **T1** — Resolver as questões abertas Q1–Q18 com o dono, priorizando as bloqueantes
  (Q1–Q8, Q12, Q17 e Q18), e registrar as respostas na seção 3. Questões adiadas ficam
  marcadas com a tarefa que depende delas.
  - Depende de: —
  - Paralelizável com: —
  - Concluído quando: todas as questões têm decisão ou adiamento explícito; as bloqueantes
    estão decididas e datadas; nenhuma tarefa condicional começa sem a decisão
    correspondente.
  - Arquivos: `MustelPlans/mcp-bulk-analysis.md`

- [ ] **T2** — Spike de protocolo: `mustel mcp serve` mínimo com `initialize`, `ping` e
  `tools/list` (uma ferramenta fake), usando o SDK escolhido em Q2 e o empacotamento de Q3.
  - Depende de: T1 (Q2, Q3, Q4, Q12, Q18)
  - Paralelizável com: T3
  - Concluído quando: o cliente escolhido em Q4 conecta, lista a ferramenta e o stdout
    contém **somente** JSON-RPC; logs vão para stderr; o término do cliente encerra o
    processo.
  - Arquivos: `crates/mustel-mcp/**`, `crates/mustel/src/commands/mcp.rs`,
    `crates/mustel/src/cli.rs`, `crates/mustel/Cargo.toml`, `Cargo.toml`

- [ ] **T3** — Atualizar o contrato e os snapshots de root help para o novo
  comando/superfície, conforme Q17.
  - Depende de: T1 (Q3, Q17, Q18)
  - Paralelizável com: T2
  - Concluído quando: `cargo test --workspace` verde; snapshots atualizados de forma
    deliberada; linhas `IF-NNN` com teste real ou `nao-coberta` justificado.
  - Arquivos: `docs/contrato-de-interface.md`,
    `crates/mustel/tests/snapshots/cli_tree__root_help_snapshot.snap`,
    `crates/mustel/tests/snapshots/contract_text__root_without_a_tty.snap`,
    `crates/mustel/tests/cli_tree.rs`

### Fase 1 — Contexto e alvos

- [ ] **T4** — Extrair a resolução servidor→conexão para `mustel-core` e fazer CLI e GUI
  usarem a mesma função, sem mudar comportamento.
  - Depende de: T2
  - Paralelizável com: T5
  - Concluído quando: `cargo test --workspace` verde; `query run` e a GUI mantêm o
    comportamento; não há terceira cópia da resolução.
  - Arquivos: `crates/mustel-core/src/usecases/connection.rs`,
    `crates/mustel-core/src/usecases/mod.rs`,
    `crates/mustel/src/commands/query/settings.rs`, `crates/mustel-gui/src/query.rs`

- [ ] **T5** — Implementar `query::policy` (allowlist de leitura) e testes. **Condicional a
  Q1.**
  - Depende de: T1 (Q1)
  - Paralelizável com: T4
  - Concluído quando: aceita `Query`, `EXPLAIN` sem `ANALYZE`, `SHOW`, `VALUES`; rejeita
    DDL/DML, `COPY`, `CALL`, `DO`, `SET`, `RESET`, `PREPARE`, `EXECUTE`, `VACUUM`,
    `REINDEX`, `CLUSTER`, `REFRESH`, `SELECT ... INTO` e múltiplos statements;
    `cargo test -p mustel-core` verde.
  - Arquivos: `crates/mustel-core/src/query/policy.rs`,
    `crates/mustel-core/src/query/mod.rs`

- [ ] **T6** — Adicionar `read_only` ao caminho de execução e abrir a sessão Postgres em
  read-only. **Condicional a Q1.**
  - Depende de: T5
  - Paralelizável com: T7
  - Concluído quando: CLI/GUI mantêm o default `false`; o caminho MCP abre read-only; um
    teste gated prova que `INSERT` falha e que uma função de escrita chamada por `SELECT`
    também falha; `cargo test --workspace` verde.
  - Arquivos: `crates/mustel-core/src/usecases/query_run.rs`,
    `crates/mustel-core/src/query/executor.rs`,
    `crates/mustel-core/src/postgres/client.rs`,
    `crates/mustel-core/src/postgres/settings.rs`,
    `crates/mustel-core/tests/postgres_integration.rs`

- [ ] **T7** — Expor `list_servers`, `list_databases`, `get_schema`, `get_ddl` como
  ferramentas MCP, sem segredos. **Condicional a Q13.**
  - Depende de: T1 (Q13), T2, T4
  - Paralelizável com: T8
  - Concluído quando: as ferramentas devolvem o esperado; nenhum output contém senha,
    connection string ou conteúdo do keyring; teste stdio cobre cada uma.
  - Arquivos: `crates/mustel-mcp/src/tools/context.rs`,
    `crates/mustel-mcp/src/lib.rs`

- [ ] **T8** — Expor `list_selectors`/`resolve_selector` e aceitar `selector` como
  alternativa a `targets`, reusando `custom_databases::{load, validate}`. **Condicional a
  Q7.**
  - Depende de: T1 (Q7), T2, T4
  - Paralelizável com: T7
  - Concluído quando: um seletor de `gui.selections` resolve para `{server, database}`;
    bases barradas por `excludePatterns` são reportadas; seletor sem servidor de origem dá
    erro acionável; nenhuma senha/connection string aparece no resultado.
  - Arquivos: `crates/mustel-mcp/src/tools/selectors.rs`,
    `crates/mustel-core/src/usecases/custom_databases.rs`,
    `crates/mustel-core/src/config/model.rs`

### Fase 2 — Execução em massa

- [ ] **T9** — Implementar o `RunRegistry` e o manifesto de execução, com `run_id`
  validado e retenção.
  - Depende de: T2
  - Paralelizável com: T13
  - Concluído quando: criar/atualizar/ler/prunar funciona; `run_id` com `..` ou
    separadores é recusado; o manifesto faz round-trip; o estado sobrevive a reinício.
  - Arquivos: `crates/mustel-mcp/src/runs.rs`, `crates/mustel-mcp/src/lib.rs`

- [ ] **T10** — Implementar `start_bulk_query`: uma query + lista de alvos agrupada por
  servidor, reusando `query_run::run`; `fetch_all_databases: false` e `all: false` para
  alvos explícitos; `label`; `wait_seconds` conforme Q9; cancelamento e timeout.
  - Depende de: T1 (Q8, Q9), T4, T6 (se Q1), T9
  - Paralelizável com: T11
  - Concluído quando: teste gated com 2 bancos roda a mesma query, gera CSV por servidor +
    consolidado e devolve `run_id`; um servidor com `fetchAllDatabases: true` ainda roda
    **só** as bases listadas; `label` aparece no manifesto.
  - Arquivos: `crates/mustel-mcp/src/tools/bulk.rs`,
    `crates/mustel-core/src/usecases/query_run.rs`

- [ ] **T11** — Implementar `get_bulk_query`, `cancel_bulk_query`, `list_bulk_runs` e o
  `EventSink` que alimenta o manifesto/resumo.
  - Depende de: T1 (Q10, Q15), T9, T10
  - Paralelizável com: T12
  - Concluído quando: o resumo traz `successes`, `failures`, `total_rows` e
    `per_database` (status, linhas, duração, erro); cancelar vira `cancelled`;
    `list_bulk_runs` lê o disco após reinício; execução sem banco acessível vira resultado
    “nada a executar”, não erro.
  - Arquivos: `crates/mustel-mcp/src/tools/bulk.rs`, `crates/mustel-mcp/src/runs.rs`,
    `crates/mustel-mcp/src/events.rs`

- [ ] **T12** — Implementar limites e política de execução (servidores, bancos,
  paralelismo, linhas, tempo), com cancelamento e marcação `truncated_by_limit`.
  **Condicional a Q6.**
  - Depende de: T1 (Q6), T11
  - Concluído quando: teste gated com caps baixos prova truncamento/cancelamento e mantém
    os artefatos parciais; os limites vêm da configuração e têm defaults seguros.
  - Arquivos: `crates/mustel-mcp/src/limits.rs`,
    `crates/mustel-core/src/usecases/query_run.rs`

### Fase 3 — Leitura do CSV

- [ ] **T13** — Implementar `query::csv_reader` com paginação, filtro por
  `Server`/`Database` e projeção de colunas, reusando o `RecordReader` (sem o crate
  `csv`).
  - Depende de: T2
  - Paralelizável com: T9, T10
  - Concluído quando: testes cobrem BOM, CRLF, aspas, newline dentro de campo, linha
    torta, `offset`, `limit`, filtro e projeção; a leitura não carrega o arquivo inteiro.
  - Arquivos: `crates/mustel-core/src/query/csv_reader.rs`,
    `crates/mustel-core/src/query/csv_writer.rs`, `crates/mustel-core/src/query/mod.rs`

- [ ] **T14** — Expor `read_bulk_csv` e a prévia do `get_bulk_query`, com
  `next_offset`/`has_more` e teto de bytes. **Condicional a Q5.**
  - Depende de: T1 (Q5), T9, T13
  - Paralelizável com: T15
  - Concluído quando: um CSV de 1.000 linhas é lido em páginas; a prévia respeita
    `maxInlineBytes`; `run_id`/caminho malicioso é recusado; a opção por arquivo de
    servidor funciona.
  - Arquivos: `crates/mustel-mcp/src/tools/csv.rs`, `crates/mustel-mcp/src/runs.rs`

- [ ] **T15** — Expor os recursos `mustel://runs`, `mustel://runs/{run_id}/summary.json`,
  `.../consolidated.csv`, `.../servers/{server}.csv` e `.../errors.csv`, com MIME correto e
  percent-encoding. **Condicional a Q5.**
  - Depende de: T1 (Q5), T9
  - Paralelizável com: T14
  - Concluído quando: `resources/list` e `resources/read` devolvem os artefatos; nomes com
    `/`, espaço e `:` resolvem; `text/csv` e `application/json` corretos.
  - Arquivos: `crates/mustel-mcp/src/resources.rs`

- [ ] **T16** — Expor erros e o status `partial` para falhas parciais; nunca descartar
  artefatos. **Condicional a Q15.**
  - Depende de: T1 (Q15), T11
  - Concluído quando: uma execução com 1 banco falhando volta `partial` com a linha de
    erro; os CSVs e o `_erros.csv` continuam legíveis.
  - Arquivos: `crates/mustel-mcp/src/tools/bulk.rs`, `crates/mustel-mcp/src/runs.rs`

### Fase 4 — Contrato, testes, docs e hardening

- [ ] **T17** — Testes de integração stdio: `initialize`, `tools/list`,
  `start_bulk_query`, `get_bulk_query`, `read_bulk_csv`, `cancel_bulk_query`, `ping` e
  `cancelled`; provar que o stdout é só JSON.
  - Depende de: T10, T11, T14
  - Concluído quando: `cargo test -p mustel-mcp` verde; um `println!` acidental no caminho
    do MCP quebra o teste.
  - Arquivos: `crates/mustel-mcp/tests/stdio.rs`,
    `crates/mustel-mcp/tests/support/mod.rs`

- [ ] **T18** — Testes gated Postgres: fan-out em 2 bancos, read-only, timeout,
  truncamento, cancelamento e falha parcial. **Condicional a Q1/Q6.**
  - Depende de: T10, T12, T16
  - Concluído quando:
    `MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-mcp --test postgres_integration`
    verde.
  - Arquivos: `crates/mustel-mcp/tests/postgres_integration.rs`

- [ ] **T19** — Fechar o contrato: linhas `IF-NNN` com testes, snapshots de root help,
  cobertura de superfície e manifesto.
  - Depende de: T2, T17
  - Concluído quando: `cargo test --workspace` verde; `contract_manifest` e
    `contract_surface` verdes; `mcp` está registrado como superfície nova, fora dos 11
    comandos do legado.
  - Arquivos: `docs/contrato-de-interface.md`, `crates/mustel/tests/**`, snapshots

- [ ] **T20** — Escrever a documentação Diátaxis: how-to de análise em massa com Claude,
  referência das ferramentas/limites/CSV e explicação do read-only e dos artefatos; linkar
  em `docs/README.md`.
  - Depende de: T14, T19
  - Paralelizável com: T21
  - Concluído quando: `documentation_links_resolve` verde; exemplos batem com os schemas
    das ferramentas e com o `--help`.
  - Arquivos: `docs/how-to/analise-em-massa-com-claude.md`, `docs/reference/mcp.md`,
    `docs/explanation/mcp.md`, `docs/README.md`

- [ ] **T21** — Hardening: auditoria (se Q11), redaction de segredos, bloco `mcp` na
  config, retenção de runs, `deny.toml`/SBOM se `rmcp` (Q2), mensagens acionáveis sem
  keyring/D-Bus.
  - Depende de: T1 (Q2, Q11, Q14, Q16), T10, T11
  - Concluído quando: `cargo deny check` verde; nenhum segredo em output/log; config faz
    round-trip e fica ausente quando default; retenção pruna runs antigos.
  - Arquivos: `deny.toml`, `sbom.cdx.json`,
    `crates/mustel-core/src/config/model.rs`, `crates/mustel-mcp/src/audit.rs`,
    `crates/mustel-mcp/src/config.rs`

### Fase 5 — Opcional (não executar sem decisão)

- [ ] **T22** — `run_query` single-DB para drill-down. **Condicional a Q13.**
  - Depende de: T1 (Q13), T6, T13
  - Concluído quando: uma query em um banco devolve colunas/linhas com teto e
    `truncated`, sem arquivo por padrão.
  - Arquivos: `crates/mustel-mcp/src/tools/single.rs`

- [ ] **T23** — `generate_sql` reusando `ai::generate`, sem executar. **Condicional a
  Q13.**
  - Depende de: T1 (Q13), T7
  - Concluído quando: devolve SQL limpo e nunca chama o executor; a chave do perfil vem do
    keyring.
  - Arquivos: `crates/mustel-mcp/src/tools/ai.rs`

- [ ] **T24** — Escrita com aprovação humana (`propose_write` → `mustel mcp approve` →
  `execute_write`). **Condicional a Q1.**
  - Depende de: T1 (Q1), T6, T10
  - Concluído quando: token single-use com TTL, hash de `server+db+sql`, auditoria e
    recusa por padrão.
  - Arquivos: `crates/mustel-mcp/src/tools/write.rs`,
    `crates/mustel-mcp/src/approval.rs`, `crates/mustel/src/commands/mcp.rs`

- [ ] **T25** — Transporte Streamable HTTP com bind em loopback e token. **Condicional a
  Q12.**
  - Depende de: T1 (Q12), T2
  - Concluído quando: cliente HTTP conecta; sem bind em `0.0.0.0`; token obrigatório.
  - Arquivos: `crates/mustel-mcp/src/transport/http.rs`, `Cargo.toml`

---

## 6. Estratégia de Verificação

O agente só marca o plano como `concluído` após **todas** as verificações aplicáveis
passarem.

- **Formatação:** `cargo fmt --all -- --check`
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Gate:** `cargo test --workspace`
  - Se `mcp` for feature não-default (Q3), rodar também
    `cargo test --workspace --all-features` e registrar a mudança de gate no plano.
- **Keyring:** `MUSTEL_KEYRING_TESTS=1 cargo test --workspace` (se o ambiente tiver cofre)
- **Postgres:**
  `MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-mcp --test postgres_integration`
- **Dependências:** `cargo deny check` (se `rmcp` ou qualquer dependência nova)
- **Diagnostics:** rodar o diagnostics do editor nos arquivos alterados
- **Stdio puro:** subir `cargo run -p mustel -- mcp serve` com um script de
  `initialize`/`tools/list` e provar que **toda** linha de stdout é JSON; logs só em
  stderr
- **Grep de resíduos:** `grep -rn "println!" crates/mustel-mcp/src` → zero;
  `grep -rn "unwrap()\\|expect(" crates/mustel-mcp/src` → zero fora de `#[cfg(test)]`
- **Segredos:** grep por `password`/`connection_string` nos outputs de ferramentas e no
  manifesto → nenhum valor secreto
- **Links:** `cargo test --workspace` inclui `documentation_links_resolve`
- **Smoke manual:** conectar o cliente escolhido em Q4, rodar uma query em 2 bancos, ler o
  CSV e conferir o resumo

> Se algo falhar, registrar na seção 9 e tentar correção antes de devolver ao usuário.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|---|---|---|---|
| CSV grande estourar o contexto do cliente | alta | alto | Prévia + paginação + orientação para agregar no Postgres; nunca devolver tudo por padrão |
| `println!`/help do CLI corromper o stdio | média | alto | Crate isolado, logs em stderr, teste de pureza do stdout, erro de parse do `mcp` em stderr |
| Bypass do read-only pelo analyzer | média | alto | Allowlist + sessão read-only no servidor + role restrito; não confiar só no analyzer |
| Segredo em output, log ou config do cliente | média | alto | Keyring; redaction; nunca devolver connection string/senha; config do cliente só com o comando |
| Terceira cópia do resolvedor CLI/GUI/MCP | alta | médio | T4 extrai para o core antes de implementar o MCP |
| `fetchAllDatabases` ignorar a lista explícita | média | alto | `fetch_all_databases: false` e `all: false`; teste com servidor que descobre |
| Falha parcial perder artefatos | média | alto | Capturar `FilesWritten`/`DatabaseFinished`; status `partial`; manter CSVs e `_erros.csv` |
| Registry/CSV crescerem sem limite | média | médio | Retenção, `keepRuns`, prune; manifesto em disco |
| `rmcp`/dependência nova quebrar `deny`/licença | baixa | médio | Features mínimas; `cargo deny`; SBOM; plano B JSON-RPC à mão |
| Versão do protocolo incompatível com o cliente | média | médio | Spike com clientes reais; negociar versão; testes gravados |
| `run_id`/caminho malicioso | baixa | alto | Validar formato; resolver caminhos só pelo registry/manifesto |
| Fan-out saturar produção | média | alto | Allowlist de servidores, caps de servidores/bancos/paralelismo, run timeout |
| CSV consolidado com colunas diferentes por banco | média | médio | Documentar a união/linha torta; oferecer arquivo por servidor |
| Keyring/D-Bus ausente no cliente | média | médio | Erro acionável; documentar `MUSTEL_CONFIG_DIR`/`MUSTEL_PASSWORD`; testar em Linux headless |
| Concorrência/races no registry | média | médio | `Arc<Mutex>`/`RwLock`; uma entrada por run; testes de cancelamento |
| Snapshot de root help mudar sem intenção | alta | baixo | Atualização deliberada em T3/T19; revisão do diff |
| `EXPLAIN ANALYZE` executar a query | baixa | alto | Política rejeita `ANALYZE` |
| `SELECT` com função de escrita | média | alto | Read-only no servidor bloqueia; teste dedicado |
| Retry do `query_run` repetir falha read-only 3× | média | baixo | Aceitar na v1; registrar se incomodar |

---

## 8. Questões em Aberto

Itens **não decididos**. O agente **não adivinha**: ou resolve com info do codebase, ou
pergunta ao usuário, ou deixa explícito no entregável. As recomendações abaixo são
propostas, **não** decisões.

- [ ] **Q1 — Postura de escrita na v1.** (a) Somente leitura; (b) leitura + escrita com
  aprovação humana em duas fases; (c) leitura + escrita livre. *Recomendação: (a).*
  Impacto: T5, T6, T18, T24.
- [ ] **Q2 — SDK/protocolo.** (a) `rmcp` (SDK oficial, features mínimas); (b) JSON-RPC
  2.0 à mão. *Recomendação: (a), com (b) como plano B se a política de dependências
  bloquear.* Impacto: T2, T21.
- [ ] **Q3 — Empacotamento.** (a) `mcp` default-on no binário único; (b) feature
  default-off; (c) binário separado `mustel-mcp`. *Recomendação: (a), pelo gate e pelo
  canal `cargo install`.* Impacto: T2, T3, verificação.
- [ ] **Q4 — Cliente(s) alvo.** Claude Desktop, Zed, Cursor, todos? *Recomendação: Claude
  Desktop como principal; Zed/Cursor como exemplos de doc.* Impacto: T2, T20.
- [ ] **Q5 — Como o cliente lê o CSV.** (a) Prévia embutida; (b) recurso
  `mustel://runs/...`; (c) ferramenta paginada; (d) os três. *Recomendação: (d), com
  prévia por default e paginação para volume.* Impacto: T14, T15.
- [ ] **Q6 — Limites.** Valores de linhas por página, bytes inline, linhas totais,
  servidores, bancos, paralelismo, timeout de comando/run e retenção. *Recomendação
  inicial: 200/64 KB/1M/20/200/4/30 s/15 min/50 runs; confirmar.* Impacto: T12, T18.
- [ ] **Q7 — Query Selector.** (a) Expor `list_selectors`/`resolve_selector` e aceitar
  `selector`; (b) só lista explícita de alvos. *Recomendação: (a), reusando
  `gui.selections` e `custom_databases`.* Confirmar semântica de escopo, seletor global,
  bases barradas e credencial por linha. Impacto: T8.
- [ ] **Q8 — Universo de alvos.** Só lista explícita, ou também `servers: "all"`/
  `databases: "all"` (descoberta)? *Recomendação: lista explícita na v1; `all` depois, se
  pedido.* Impacto: T10.
- [ ] **Q9 — Execução.** (a) Assíncrona com `run_id` + polling; (b) síncrona com
  `wait_seconds`; (c) as duas. *Recomendação: (c), com `wait_seconds` default 0.*
  Impacto: T10, T11.
- [ ] **Q10 — Persistência.** Onde ficam os runs (`<config>/mcp/runs` vs `results/`), se
  `list_bulk_runs` sobrevive a reinício e qual a retenção. *Recomendação:
  `<config>/mcp/runs`, manifesto em disco, retenção.* Impacto: T9, T11.
- [ ] **Q11 — Auditoria.** Ligada por padrão? Guarda SQL completo ou hash? Onde?
  *Recomendação: JSONL opt-in com hash + metadados, sem dados de linha.* Impacto: T21.
- [ ] **Q12 — Transporte.** Só stdio na v1? Streamable HTTP depois? *Recomendação: stdio
  na v1.* Impacto: T25.
- [ ] **Q13 — Superfície da v1.** Só bulk + CSV, ou também `run_query` single-DB,
  `get_schema`, `get_ddl`, `generate_sql`? *Recomendação: bulk + CSV + contexto
  (schema/DDL) na v1; single-DB e `generate_sql` depois.* Impacto: T7, T22, T23.
- [ ] **Q14 — Config `mustel.jsonc`.** Forma do bloco `mcp`, include/exclude de
  servidores, defaults e o que fica fora do arquivo. *Recomendação: bloco opcional,
  omitido quando default.* Impacto: T21.
- [ ] **Q15 — Falha parcial.** Um run com alguns bancos falhando é `partial`, `succeeded`
  ou erro? *Recomendação: `partial` com `per_database` e `_erros.csv`.* Impacto: T16.
- [ ] **Q16 — Segurança de credenciais.** Role dedicado read-only? Permitir superuser?
  Fallback `MUSTEL_PASSWORD`? *Recomendação: documentar role read-only e não exigir
  superuser; keyring como caminho principal.* Impacto: T21, docs.
- [ ] **Q17 — Contrato.** Quais linhas `IF-NNN` novas, quais snapshots mudam e como
  registrar `mcp` fora dos 11 comandos do legado. *Recomendação: `mcp` como superfície
  nova, como `gui`.* Impacto: T3, T19.
- [ ] **Q18 — Nomenclatura.** `mustel mcp serve` vs `mustel mcp` vs binário separado;
  futuro `approve`/`doctor`. *Recomendação: `mustel mcp serve`, com espaço para
  subcomandos.* Impacto: T2, T3.

---

## 9. Log de Execução

Append-only. O agente registra cada avanço relevante (início, conclusão, bloqueio,
paralelização, falhas). Mantém o “Atualizado em” sincronizado.

- `[2026-11-04 21:04]` Plano criado como `rascunho`; diretório `MustelPlans/` recriado
  (não existia); aguardando as decisões Q1–Q18. Nenhuma tarefa iniciada.
