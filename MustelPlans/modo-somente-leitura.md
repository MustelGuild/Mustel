# Plano — Modo somente leitura (`BEGIN TRANSACTION READ ONLY`)

> **Status:** bloqueado
> **Criado em:** 2026-11-11
> **Atualizado em:** 2026-11-12
> **Complexidade:** média
> **Depende de (outros planos):** nenhum

---

## 1. Visão Geral

A aba **Settings** ganha a opção **“Only run read-only queries”**, gravada em
`gui.readOnlyQueries` e **ligada por padrão na primeira execução**. Quando ela está ligada, a
execução do `Run query` abre a conexão de cada banco com `BEGIN TRANSACTION READ ONLY`, e o alerta
de consulta destrutiva deixa de oferecer
`Run anyway`: ele passa a informar que a consulta **não pode rodar** enquanto o modo estiver
ligado.

O valor é impedir escrita acidental na janela sem depender só do classificador. O `analyzer`
continua sendo a primeira barreira e é o que alimenta o alerta, mas o PostgreSQL é a autoridade:
na transação read-only ele recusa `INSERT`, `UPDATE`, `DELETE`, DDL, `SELECT ... FOR UPDATE`,
`nextval()` e funções que escrevem em tabela com SQLSTATE `25006`. **`COPY ... TO` não é
bloqueado nesta fase**, porque é leitura; bloquear `COPY` exige uma whitelist no cliente e fica
para a fase seguinte.

---

## 2. Escopo

**Inclui:**
- Novo campo `GuiPrefs.read_only_queries: bool` (`gui.readOnlyQueries`), default `true` (ligado na
  primeira execução), omitido do JSON quando `true`; `false` é gravado quando o dono desliga.
- Nova categoria **Query** na aba Settings, com checkbox, ícone próprio e texto explicativo.
- Selo `Read-only` ao lado do `Run query` quando o modo está ligado.
- Alerta de consulta destrutiva transformado em **alerta bloqueante** no modo somente leitura:
  mesmos detalhes (tipo, servidores, bancos, preview), sem `Run anyway`, com `Close`.
- Flag `read_only` propagada por `Request::RunQuery` → `QueryRunRequest` → `ExecutionRequest` →
  `QueryStream::open`.
- `BEGIN TRANSACTION READ ONLY` na conexão de execução quando `read_only` é `true`; a transação é
  abortada quando o `Client` é largado no fim do fluxo (não há `COMMIT`).
- Testes de unidade (modelo, Settings, tela Query), teste de integração gated do `25006` e
  atualização do manifesto de contrato.
- Documentação em `docs/reference/settings-aba.md`, `docs/explanation/classificador-de-query.md`
  e `mustel.example.jsonc`.

**Exclui explicitamente:**
- **Bloqueio de `COPY` em qualquer forma** (inclusive `COPY ... TO STDOUT`). O `BEGIN READ ONLY`
  não bloqueia `COPY TO` porque é leitura, e o `analyzer` atual classifica `COPY TO STDOUT` como
  `Safe`. Isso exige a whitelist de instruções (fase seguinte).
- Whitelist de instruções no cliente (só `SELECT`/`SHOW`/`EXPLAIN`) e bloqueio de `SET`, `COPY`,
  `CALL`, `DO`, `VACUUM` etc. por análise da AST.
- Mudança de comportamento do **CLI**: `query run` continua passando `read_only: false`. O modo é
  só da janela; não entra em `defaults` e não ganha `--read-only`/`--no-read-only`.
- `pg_read_all_data`, `SET default_transaction_read_only`, `options=-c ...` ou credenciais
  read-only por servidor. O mecanismo desta fase é o `BEGIN` explícito.
- Alterar `ConnectionSettings` ou os caminhos de descoberta, schema, DDL, teste de conexão e
  backup. A flag vive no pedido de execução, não na configuração de conexão.
- Botão `Open Settings` no alerta bloqueante; ele terá só `Close`.
- Resetar o modo no `Restore defaults` da categoria Appearance.
- Teste manual na janela do Windows como tarefa obrigatória; fica como verificação condicional
  (T10).

> Se durante a execução surgir algo na lista “Exclui”, **NÃO implementar**. Registrar na seção 8
> (Questões em Aberto) e perguntar ao usuário.

---

## 3. Decisões

Decisões travadas durante a exploração. O agente **não reabre** estas escolhas sem justificativa
explícita e aprovação do usuário.

| #  | Decisão | Escolha | Razão |
|----|---------|---------|-------|
| D1 | Onde o modo vive | `gui.readOnlyQueries`, só a GUI | Foi pedido como parâmetro da aba Settings; menor raio de mudança e nenhum impacto no contrato do CLI. O CLI pode ganhar `--read-only` depois, reusando a flag do núcleo. |
| D2 | Como o servidor é forçado | `BEGIN TRANSACTION READ ONLY` em `QueryStream::open` | Uma instrução por conexão de execução, sem `ConnectionSettings`; funciona atrás de PgBouncer em transaction pooling, e o drop do `Client` aborta a transação. |
| D3 | O alerta de destrutiva no modo | Alerta bloqueante, sem `Run anyway` | É o pedido explícito: em vez de confirmar, a janela avisa que a consulta não pode rodar. Reaproveita os detalhes que o usuário já conhece. |
| D4 | `COPY` nesta fase | Não é bloqueado | `COPY ... TO` é leitura para o PostgreSQL; `READ ONLY` e `pg_read_all_data` não o barram. A whitelist de instruções é a fase seguinte. |
| D5 | Default e serialização | `true` (primeira execução); omitido do JSON quando `true`; `false` é gravado ao desligar | A primeira execução já protege; o opt-out é explícito e o arquivo só muda para quem desligou. |
| D6 | `Restore defaults` | **Não** desliga o modo | Desligar uma proteção deve ser ação explícita; o botão é da Appearance e não deve apagar uma escolha de segurança. |
| D7 | Confirmação pendente | `set_read_only` limpa a confirmação quando o valor muda | Impede que um `Run anyway` aberto antes da mudança escape da nova política. |
| D8 | Onde a flag viaja | `QueryRunRequest` e `ExecutionRequest`, não `ConnectionSettings` | Descoberta, schema, DDL, teste de conexão e backup continuam normais; só a execução da consulta abre a transação read-only. |

---

## 4. Arquitetura / Design

```mermaid
flowchart TD
    S[Settings › Query: Only run read-only queries] --> P[gui.readOnlyQueries no mustel.jsonc]
    P --> A[App: settings.read_only]
    A --> Q[Aba Query: read_only]
    Q --> R[Run query]
    R --> D{analyzer: destrutiva?}
    D -- não --> X[Request::RunQuery read_only=true]
    D -- sim --> W[Alerta bloqueante: não pode rodar]
    X --> U[QueryRunRequest.read_only=true]
    U --> E[ExecutionRequest.read_only=true]
    E --> O[QueryStream::open: BEGIN TRANSACTION READ ONLY]
    O --> PG[(PostgreSQL)]
    PG -- escrita --> Z[SQLSTATE 25006]
```

Fluxo detalhado:
1. A aba Settings edita `gui.readOnlyQueries`; o `App` lê o valor de `settings.read_only()` e o
   entrega à tela Query a cada quadro (mesmo padrão de `set_page_size`).
2. No `Run query`, se o `analyzer` classifica a consulta como `Destructive`:
   - modo desligado: abre o modal atual, com `Cancel`/`Run anyway`;
   - modo ligado: abre o alerta bloqueante, sem `Run anyway`, e não emite `Request`.
3. Consulta `Safe` roda normalmente; o `Request::RunQuery` leva `read_only`.
4. O `App` monta o `QueryRunRequest` com a flag; o núcleo copia para `ExecutionRequest`.
5. `QueryStream::open` conecta, executa `BEGIN TRANSACTION READ ONLY` (sob o mesmo
   `command_timeout`), faz `prepare` e `query_raw`.
6. Se a consulta tentar escrever, o servidor devolve `25006`; o erro entra no CSV de erros e no log
   como qualquer falha de banco. **Não há `COMMIT`**: o `Client` é largado no fim de `stream_once`
   e a conexão fecha, abortando a transação.

### Tipos / arquivos novos
- Nenhum módulo novo. A mudança é em tipos e telas existentes.

### Mudanças em tipos existentes
- `crates/mustel-core/src/config/model.rs` — `GuiPrefs.read_only_queries` + `is_false`.
- `crates/mustel-gui/src/screens/settings.rs` — `Category::Query`, `State.read_only`,
  `with_prefs`/`prefs`/`read_only()` e o checkbox.
- `crates/mustel-gui/src/icons.rs` — `Icon::LockClosed` (`lock-closed`).
- `crates/mustel-gui/src/screens/query.rs` — `State.read_only`, `set_read_only`,
  `ConfirmationKind`, lógica de `request_run`, alerta bloqueante e selo.
- `crates/mustel-gui/src/request.rs` — `Request::RunQuery.read_only`.
- `crates/mustel-gui/src/query.rs` — `request_for_runs(..., read_only)`.
- `crates/mustel-gui/src/app.rs` — dispatch da flag e `set_read_only` por quadro.
- `crates/mustel-core/src/usecases/query_run.rs` — `QueryRunRequest.read_only` e repasse.
- `crates/mustel-core/src/query/executor.rs` — `ExecutionRequest.read_only` e repasse.
- `crates/mustel-core/src/postgres/client.rs` — `QueryStream::open(..., read_only)` com o
  `BEGIN TRANSACTION READ ONLY`.

---

## 5. Tarefas

Cada tarefa tem **ID único** para referência cruzada. O agente marca `- [ ]` → `- [x]` ao concluir
e atualiza a seção 9 (Log).

### Fase 1 — Núcleo

- [x] **T1** — Propagar `read_only` e abrir a transação read-only no `QueryStream`
  - Depende de: —
  - Paralelizável com: T3
  - Concluído quando:
    - `QueryRunRequest`, `ExecutionRequest` e `QueryStream::open` carregam `read_only: bool`;
    - `query_run::run` repassa a flag e `stream_once` a entrega ao `open`;
    - com `read_only = true`, `open` executa `BEGIN TRANSACTION READ ONLY` sob o
      `command_timeout`, antes do `prepare`;
    - `cargo check -p mustel-core --all-targets` passa com os construtores de teste atualizados
      (`usecases/query_run.rs` helper, testes de `executor.rs` e chamadas existentes de
      `postgres_integration.rs` com `false`);
    - o caminho de execução não ganha `unwrap()`/`expect()`.
  - Arquivos: `crates/mustel-core/src/usecases/query_run.rs`,
    `crates/mustel-core/src/query/executor.rs`,
    `crates/mustel-core/src/postgres/client.rs`,
    `crates/mustel-core/tests/postgres_integration.rs` (só as chamadas existentes).

- [x] **T2** — Testes de integração gated do `BEGIN READ ONLY`
  - Depende de: T1
  - Paralelizável com: T4, T5, T6
  - Concluído quando:
    - `a_read_only_stream_rejects_a_write_with_sqlstate_25006` abre um `QueryStream` com
      `read_only = true` e um `CREATE TABLE mustel_read_only_blocked_<pid>` e confere
      `CoreError::Postgres { sqlstate: Some("25006"), .. }` (o erro pode vir do `open` ou do
      primeiro `next_row`);
    - `a_read_only_stream_still_reads` abre com `read_only = true` e lê `SELECT 1`;
    - sem `MUSTEL_PG_TEST_URL`, os testes fazem skip e imprimem a mensagem;
    - com `MUSTEL_PG_TEST_URL`, os dois passam;
    - o teste de escrita tem o marcador `/// CONTRACT: IF-271`.
  - Arquivos: `crates/mustel-core/tests/postgres_integration.rs`.

- [x] **T3** — Preferência `gui.readOnlyQueries` no modelo
  - Depende de: —
  - Paralelizável com: T1
  - Concluído quando:
    - `GuiPrefs.read_only_queries: bool` existe, default `true`, com
      `#[serde(skip_serializing_if = "is_true")]`;
    - `the_default_read_only_is_on` prova que a primeira execução nasce ligada;
    - `the_read_only_preference_round_trips` prova a ida e volta do opt-out (`false`);
    - `the_default_read_only_stays_out_of_the_file` prova que o default não escreve a chave;
    - `a_config_without_the_read_only_key_reads_as_true` prova que um arquivo antigo carrega `true`;
    - os construtores explícitos de `GuiPrefs` em `config/model.rs` e
      `usecases/gui_prefs.rs` compilam (campo novo ou `..Default::default()`);
    - `cargo test -p mustel-core --lib` passa.
  - Arquivos: `crates/mustel-core/src/config/model.rs`,
    `crates/mustel-core/src/usecases/gui_prefs.rs`.

### Fase 2 — GUI

- [x] **T4** — Categoria Query na aba Settings
  - Depende de: T3
  - Paralelizável com: T2, T6
  - Concluído quando:
    - `Icon::LockClosed` resolve em `lock-closed` na fonte embutida e entra na lista `ALL` do
      teste de ícones (25 → 26);
    - `Category::Query` aparece no menu (ordem: Appearance, Database list, Query, AI profiles,
      Activity, About), com rótulo, dica e ícone únicos;
    - `State.read_only` é lido por `with_prefs`, gravado por `prefs()` e exposto por
      `read_only()`;
    - o checkbox **“Only run read-only queries”** vem marcado por padrão na primeira execução,
      marca `outcome.changed` ao mudar e mostra o texto
      `On by default. Blocks queries that write. The PostgreSQL session is opened as read-only.`;
    - `every_category_has_its_own_icon_and_label` passa com `ALL.len() == 6` e a mensagem lista a
      categoria nova;
    - `the_state_keeps_every_saved_preference` inclui `read_only_queries: false` e confere o
      opt-out;
    - `a_fresh_state_starts_read_only` prova que `with_prefs(None)` começa ligado;
    - um teste novo prova que `restore_defaults` **não** altera a escolha (com `false`);
    - `cargo test -p mustel-gui screens::settings` passa.
  - Arquivos: `crates/mustel-gui/src/screens/settings.rs`,
    `crates/mustel-gui/src/icons.rs`.

- [x] **T5** — Alerta bloqueante, selo e plumbing da aba Query
  - Depende de: T1, T4
  - Paralelizável com: T2, T6
  - Concluído quando:
    - `State.read_only` e `set_read_only` existem; mudar o valor limpa a confirmação pendente;
    - com o modo ligado e SQL destrutivo, `request_run` devolve `None`, cria
      `Confirmation { kind: ReadOnly, request: None, .. }` e não abre o modal de confirmação;
    - com o modo ligado e SQL `Safe`, `request_run` devolve
      `Some(Request::RunQuery { read_only: true, .. })`;
    - com o modo desligado, o comportamento destrutivo atual continua (modal com
      `Run anyway`);
    - o modal `ReadOnly` mostra o título **`READ-ONLY MODE: QUERY NOT RUN`**, a mensagem
      `Read-only mode is on, so this query cannot run. Turn it off in Settings › Query to run
      destructive queries.`, os mesmos detalhes e só o botão **`Close`**; ao fechar, deixa o erro
      inline `Read-only mode: the query was not run.`;
    - o selo **`Read-only`** aparece ao lado de `Run query` quando o modo está ligado, com tooltip
      `Settings › Query: only read-only queries run.`;
    - `Request::RunQuery` carrega `read_only`; `request_for_runs(..., read_only)` repassa para
      `QueryRunRequest`;
    - `app.rs` passa a flag no dispatch e chama `self.query.set_read_only(...)` antes de
      `self.query.ui(...)`;
    - testes novos: `read_only_blocks_a_destructive_query_with_an_alert` (com
      `/// CONTRACT: IF-271`), `read_only_runs_a_select_with_the_flag` e
      `enabling_read_only_clears_a_pending_confirmation`; os testes existentes de confirmação são
      atualizados para `ConfirmationKind::Destructive` e `request: Some(..)`;
    - `cargo test -p mustel-gui` passa.
  - Arquivos: `crates/mustel-gui/src/screens/query.rs`,
    `crates/mustel-gui/src/request.rs`, `crates/mustel-gui/src/query.rs`,
    `crates/mustel-gui/src/app.rs`.

- [x] **T6** — Construções fora do núcleo
  - Depende de: T1
  - Paralelizável com: T2, T4, T5
  - Concluído quando:
    - `crates/mustel/src/commands/query/run.rs` compila com `read_only: false` (CLI inalterado);
    - `crates/mustel-gui/tests/plumbing.rs` compila com `read_only: false` nos dois
      `QueryRunRequest`;
    - `cargo check -p mustel -p mustel-gui --all-targets` passa.
  - Arquivos: `crates/mustel/src/commands/query/run.rs`,
    `crates/mustel-gui/tests/plumbing.rs`, `crates/mustel-gui/tests/query_flow.rs`.

### Fase 3 — Documentação e contrato

- [x] **T7** — Atualizar documentação e exemplo de configuração
  - Depende de: T5
  - Paralelizável com: T8
  - Concluído quando:
    - `docs/reference/settings-aba.md` passa a listar **seis** categorias, descreve a categoria
      Query, o default ligado, o campo `readOnlyQueries` (gravado só no opt-out) e a regra de que
      `Restore defaults` não altera a escolha;
    - `docs/explanation/classificador-de-query.md` ganha a seção “Modo somente leitura”, com o
      `BEGIN TRANSACTION READ ONLY`, o `25006`, os casos em que o servidor é mais rigoroso que o
      `analyzer` (`FOR UPDATE`, `nextval`) e a limitação explícita de `COPY ... TO STDOUT`;
    - `mustel.example.jsonc` mostra `"readOnlyQueries": false` sob `gui`, com comentário de que é o
      opt-out (o padrão é ligado) e que o CLI não é afetado;
    - nenhum link relativo aponta para arquivo inexistente.
  - Arquivos: `docs/reference/settings-aba.md`,
    `docs/explanation/classificador-de-query.md`, `mustel.example.jsonc`.

- [x] **T8** — Manifesto de contrato
  - Depende de: T2, T5
  - Paralelizável com: T7
  - Concluído quando:
    - `docs/contrato-de-interface.md` ganha a linha **IF-271** (próximo livre; o maior atual é
      IF-270), origem `GUI`, classe `B`, status `coberta`, com a decisão do checkbox, do
      alerta bloqueante e do `BEGIN TRANSACTION READ ONLY`, e a limitação de `COPY TO`;
    - a coluna `Teste` cita testes que existem de fato:
      `crates/mustel-gui/src/screens/query.rs::read_only_blocks_a_destructive_query_with_an_alert`,
      `crates/mustel-gui/src/screens/query.rs::read_only_runs_a_select_with_the_flag`,
      `crates/mustel-gui/src/screens/query.rs::enabling_read_only_clears_a_pending_confirmation`,
      `crates/mustel-gui/src/screens/settings.rs::the_state_keeps_every_saved_preference`,
      `crates/mustel-core/src/config/model.rs::the_read_only_preference_round_trips`,
      `crates/mustel-core/tests/postgres_integration.rs::a_read_only_stream_rejects_a_write_with_sqlstate_25006`;
    - a linha IF-248 é atualizada para incluir a categoria Query na lista de categorias;
    - `cargo test -p mustel --test contract_manifest` passa.
  - Arquivos: `docs/contrato-de-interface.md`.

### Fase 4 — Verificação

- [ ] **T9** — Rodar o gate completo
  - Depende de: T1, T2, T3, T4, T5, T6, T7, T8
  - Paralelizável com: —
  - Concluído quando todos os comandos da seção 6 passam e o log registra o resultado.
  - Arquivos: nenhum (correções pontuais, se necessárias).

- [ ] **T10** — Smoke condicional da GUI
  - Depende de: T5
  - Paralelizável com: T7, T8
  - Concluído quando:
    - em ambiente com janela, abrir a GUI e conferir que `Settings › Query` já vem marcado; rodar
      um `DELETE` e ver o alerta bloqueante sem `Run anyway`; rodar um `SELECT` e ver o resultado;
      conferir que `gui.readOnlyQueries` **não** aparece no `mustel.jsonc` (default ligado);
      desmarcar, rodar o `DELETE` e ver o modal destrutivo; conferir `"readOnlyQueries": false`;
    - no Windows, seguir a skill `.agents/skills/mustel-gui-smoke-test/SKILL.md`;
    - se não houver ambiente gráfico, registrar “não executado” com o motivo no log — o gate
      automatizado (T9) continua valendo.
  - Arquivos: nenhum.

---

## 6. Estratégia de Verificação

- **Formatação:** `cargo fmt --all`; depois `cargo fmt --all -- --check` — sem diff.
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` — sem warnings.
- **Gate:** `cargo test --workspace` — verde; inclui `contract_manifest`, `documentation_links_resolve`
  e os testes de tela.

> **Plataforma:** o QA Gate oficial roda em `windows-latest` (`.github/workflows/qa-gate.yml`). No
> Linux, dois grupos de testes **pré-existentes** são específicos de Windows e falham por
> plataforma: os snapshots de `--help` (`mustel.exe` vs `mustel`) e os replays de `pgpass` que
> comparam CRLF. Fora do Windows, rode com
> `-- --skip root_help_snapshot --skip query_run_help_snapshot --skip replay_database_pgpass`; no
> Windows, rode sem skips.
- **Integração Postgres (gated):**
  ```bash
  docker run -d --name mustel-ro-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=mustel_test -p 55432:5432 postgres:16
  MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test \
    cargo test -p mustel-core --test postgres_integration
  ```
  Esperado: `a_read_only_stream_rejects_a_write_with_sqlstate_25006` e
  `a_read_only_stream_still_reads` verdes; os demais também.
- **Build:** `cargo build --workspace` — verde.
- **Grep de resíduos:** `grep -rn "TODO\|FIXME"` nos arquivos alterados — nenhum resultado novo.
- **Grep de contrato:** `grep -rn "BEGIN TRANSACTION READ ONLY" crates/` — aparece só em
  `postgres/client.rs` e nos testes; nenhuma cópia em outro caminho de execução.
- **Grep de configuração:** `grep -rn "readOnlyQueries" docs/ mustel.example.jsonc` — exemplo e
  referência presentes.
- **Smoke (T10):** abrir a janela, alternar o checkbox, rodar `DELETE`/`SELECT` e conferir o
  `mustel.jsonc`.

> O agente só marca o plano como `concluído` após **todas** as verificações passarem. Se algo
> falhar, registrar na seção 9 e tentar correção antes de devolver ao usuário.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|-------|------|---------|-----------|
| `COPY ... TO STDOUT` continua permitido, embora o pedido original fosse bloquear `COPY` | alta | alto | Escopo explícito na seção 2 e na doc; a whitelist de instruções é a fase seguinte. O alerta bloqueante só cobre o que o `analyzer` chama de destrutivo. |
| O modo vale só na GUI; o CLI ainda escreve | certa | médio | D1 e doc; se o dono quiser o produto inteiro, migrar para `defaults.readOnlyQueries` e adicionar a recusa no `query run` (exit 40). |
| Consulta `Safe` que escreve (função, `nextval`, `FOR UPDATE`) chega ao servidor e falha por banco, com CSV de erro em vez do alerta bonito | média | médio | É o backstop do servidor; documentar em `classificador-de-query.md`; a whitelist futura melhora o aviso no cliente. |
| Depender do drop do `Client` para abortar a transação | baixa | médio | Doc comment no `QueryStream::open` + teste gated; se um refactor futuro mantiver o cliente vivo, adicionar `ROLLBACK` explícito. |
| Usuário desliga o modo sem perceber que o alerta mudou | baixa | médio | Selo `Read-only` visível ao lado do `Run query`; `Restore defaults` não desliga; doc da categoria. |
| `Icon::LockClosed` não existir no pacote embutido | baixa | baixo | `every_icon_resolves_in_the_embedded_font` falha; fallback é usar `Icon::Warning` (também único entre as categorias). |
| Categoria nova quebra testes de geometria/menu | baixa | baixo | O menu é fixo e rolável; atualizar `ALL.len()` e a mensagem do teste; rodar `cargo test -p mustel-gui`. |
| PgBouncer em transaction pooling | baixa | baixo | O `BEGIN` e a consulta ficam na mesma conexão/transação; não há `SET` de sessão. |
| Construtores de `GuiPrefs`/`QueryRunRequest` esquecidos | média | baixo | O compilador aponta; T1/T3/T6 listam os pontos conhecidos. |
| Gate oficial é Windows; no Linux snapshots de help (`mustel.exe`) e replays de `pgpass` (CRLF) falham por plataforma | certa | médio | Rodar o QA Gate em `windows-latest`; no Linux, pular os 9 casos de plataforma e registrar. |
| Primeira execução já vem read-only e pode surpreender quem quer escrever | média | médio | Checkbox marcado, selo `Read-only` e alerta com instrução para desmarcar em `Settings › Query`; docs. |
| Manifesto sem teste correspondente | baixa | médio | T8 cita funções existentes e os marcadores `CONTRACT: IF-271` são adicionados em T2/T5. |

---

## 8. Questões em Aberto

- [x] **Q1** — Confirmar que, nesta fase, `COPY ... TO STDOUT` continua permitido (é leitura;
  `BEGIN READ ONLY` não o bloqueia e o `analyzer` o classifica `Safe`). Resolvido pelo escopo do
  plano: o bloqueio de `COPY` fica para a whitelist da fase seguinte.
- [x] **Q2** — Confirmar que o modo é **só da janela** (D1). Resolvido por D1: o CLI não é afetado;
  se precisar valer para o CLI, o campo migra para `defaults.readOnlyQueries` e o `query run`
  recusa com exit 40.

---

## 9. Log de Execução

Append-only. O agente registra cada avanço relevante (início, conclusão, bloqueio,
paralelização, falhas). Mantém o “Atualizado em” sincronizado.

- `[2026-11-11]` Plano criado como `rascunho` a partir da exploração do modo somente leitura.
- `[2026-11-11 23:40]` Execução iniciada pelo mustel-apply.
- `[2026-11-11 23:41]` T1 concluída — `read_only` em `QueryRunRequest`/`ExecutionRequest`; `QueryStream::open` abre `BEGIN TRANSACTION READ ONLY`; chamadas existentes atualizadas; `cargo check -p mustel-core --all-targets` passou.
- `[2026-11-11 23:42]` T3 concluída — `GuiPrefs.read_only_queries` com `skip_serializing_if = "is_false"`; testes de round-trip/default/arquivo antigo; `cargo test -p mustel-core --lib config::model::tests` passou (24 testes).
- `[2026-11-11 23:44]` T2 concluída — `a_read_only_stream_rejects_a_write_with_sqlstate_25006` e `a_read_only_stream_still_reads` contra Postgres 18.6 (podman); 12 testes de integração passaram.
- `[2026-11-11 23:44]` T4 concluída — `Category::Query`, `Icon::LockClosed` e checkbox; `cargo test -p mustel-gui --lib` passou (293 testes).
- `[2026-11-11 23:44]` T5 concluída — `ConfirmationKind`, alerta bloqueante, selo `Read-only` e flag em `Request::RunQuery`/`request_for_runs`; testes novos passaram.
- `[2026-11-11 23:44]` T6 concluída — `read_only: false` no CLI, `plumbing.rs` e `query_flow.rs`; `cargo check --workspace --all-targets` passou.
- `[2026-11-11 23:44]` T7 concluída — `settings-aba.md` (seis categorias + seção Query), `classificador-de-query.md` (modo somente leitura) e `mustel.example.jsonc`; `documentation_links_resolve` passou.
- `[2026-11-11 23:44]` T8 concluída — linha IF-271 e atualização da IF-248; `cargo test -p mustel --test contract_manifest` passou.
- `[2026-11-11 23:48]` T9 bloqueada no comando literal `cargo test --workspace` no Linux — 9 casos de plataforma (snapshots de `--help` com `mustel.exe` e replays de `pgpass` com CRLF). A suíte com esses casos pulados passou; fmt, clippy, build, núcleo (359), GUI (294), integração gated (12), contrato e links passaram. O QA Gate oficial é `windows-latest`.
- `[2026-11-11 23:48]` T10 parcial — teste headless `the_query_category_draws_the_read_only_checkbox` passou; smoke na janela do Windows pendente (skill `mustel-gui-smoke-test`).
- `[2026-11-12 00:06]` Ajuste de alinhamento do menu de Settings — `category_item` agora desenha o ícone num quadrado fixo (altura do texto) e o rótulo em x fixo, como as abas; novo teste `the_category_labels_share_the_same_left_edge`. `cargo fmt`, clippy e 295 testes da GUI passaram.
- `[2026-11-12 00:10]` Revisão a pedido do dono — `gui.readOnlyQueries` passa a ser `true` por padrão (primeira execução); a chave só é gravada no opt-out (`false`); `Restore defaults` não altera a escolha. Testes do modelo e da aba Settings atualizados; o teste de renderização confirma o checkbox marcado.
- `[2026-11-12 00:11]` Revisão do default ligado verificada — base do recurso no commit `9bcce97` (feito pelo dono); esta revisão está na árvore. Modelo 25, Settings 28, GUI 296, `contract_manifest` e `documentation_links_resolve` passaram; suíte completa com os 9 casos de plataforma pulados passou.
