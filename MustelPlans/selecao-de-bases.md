# Plano — Seleção de bases a partir de uma query salva

> **Status:** concluído
> **Criado em:** 2026-10-23
> **Atualizado em:** 2026-10-23
> **Complexidade:** média-alta (núcleo de leitura/validação simples; o valor e o risco estão na tela)
> **Depende de (outros planos):** `autocomplete-sql.md` (concluído — reaproveita o padrão de `⟳` +
> cache em memória), `migracao-rust.md` (concluído — cadastro, cofre e `query run`)

---

## 1. Visão Geral

O dono tem um banco **configurador** onde ficam os tenants e suas connection strings. O que ele quer é
usar essa base para **decidir em quais bases a consulta roda** — por exemplo, "só as bases de produção,
nenhuma de test" — sem montar a seleção à mão toda vez.

Hoje a janela escolhe **servidores** (checkbox) e o destino das bases é decidido pelo cadastro
(`fetchAllDatabases` ou a lista `databases`). Não existe seleção de base, e a query de negócio não tem
onde entrar.

Este plano acrescenta três coisas:

1. **Seleção por base**, não só por servidor: a lista de servidores abre em `servidor → bases`, com
   checkbox em cada base.
2. **Queries de seleção salvas** na seção `gui` do `mustel.jsonc`: um SQL que roda contra um
   servidor+banco do cadastro e devolve as bases (connection string Npgsql **ou** colunas).
3. **Aplicação em massa** dessa seleção — em todos os servidores ou em um só — substituindo o que
   estava marcado.

**O que mudou durante o desenho (registro honesto):** a primeira versão tinha "dois modos" (Fixed ×
Custom). O dono cortou: **não há modos**; há **um combo de queries de seleção** ao lado da lista de
servidores, e o que muda é *o que fica marcado*. Também mudamos a regra de credencial duas vezes: eu
propus "string vence", o dono disse "ignore a senha da string, use o cofre", e na revisão final ficou
**"o que vier da string vale; o que faltar vem do cadastro"**. As três versões estão no §8 para quem
for revisar não achar que o plano nasceu torto.

---

## 2. Escopo

**Inclui:**

- Parser de connection string Npgsql **no núcleo** (hoje vive no binário do CLI), passando a **ler
  também a senha** — o CLI continua ignorando-a, por paridade com o legado.
- `usecases::custom_databases::load(client, sql)`: roda o SQL e devolve as linhas lidas.
- `usecases::custom_databases::validate(...)`: casa host → servidor do cadastro, aplica os
  `excludePatterns` do servidor e agrega os erros.
- `GuiPrefs::selections: Vec<SelectionQuery>` (lista desde já), preservada pelo CLI.
- Aba **Settings**: CRUD das queries de seleção + botão **Testar**.
- Aba **Query**: combo de queries, `⟳ Apply to all` / `⟳ Apply` (por servidor), `⟳ Discover all` /
  `⟳ Discover` (por servidor), lista `servidor → bases` com checkbox, itens excluídos desabilitados com
  tooltip, busca e paginação.
- `Request::RunQuery` passa a levar **bases explícitas** por servidor.

**Não inclui (declarado):**

- Qualquer mudança de comportamento do **CLI** (`--all`, `--exclude`, `fetchAllDatabases` ficam como
  estão).
- Parâmetros na query de seleção (bind variables); hoje é SQL literal salvo no arquivo.
- Persistir a **seleção** entre sessões: a seleção é do momento da execução, não configuração.
- Mapeamento arbitrário de colunas (nome de coluna configurável): as colunas aceitas são as do §4.3.
- Múltiplas queries *aplicadas em conjunto*: uma por vez no combo.

---

## 3. Decisões

| # | Decisão | Escolha | Razão |
|---|---|---|---|
| D1 | Modelo da tela | **Sem modos.** Lista de servidores do cadastro + **combo de queries de seleção** com `⟳` | Decisão do dono (2026-10-23): os servidores são os mesmos; o que muda é o que fica marcado |
| D2 | Escopo do `⟳ Apply` | **Todos** (botão do combo) ou **um servidor** (botão da linha) | Pedido do dono: aplicar a query só no servidor 4, por exemplo |
| D3 | Semântica do `⟳ Apply` | **Substitui** a seleção pelo resultado da query | A query define uma seleção em massa; desmarcar depois é livre |
| D4 | Universo da lista | **Todos** os servidores do cadastro; cada um com as bases que conhece | O que muda entre execuções é só o marcado |
| D5 | Origem das bases | **Descoberta** (`pg_database`, por servidor ou global) **+** resultado da query; as duas se somam | A query pode não conhecer tudo; descobrir preenche o resto |
| D6 | Formato da linha | **Connection string** (Npgsql) quando presente; senão colunas `host, port, database` (+ `username`, `password`, `server` opcionais) | Pedido do dono: "assuma a string como verdade" |
| D7 | Credenciais | O que vier da string/colunas vence; o que faltar vem do cadastro + **cofre** | Revisão final do dono (2026-10-23) |
| D8 | Usuário da string ≠ usuário do cadastro **sem** senha na string | Linha vira **aviso** e **não** é marcada | A senha do cofre é do usuário do cadastro; usar com outro usuário falharia sem explicar |
| D9 | Validação de host | **Obrigatória**: casa por `host`+`porta`, ou pelo **nome** quando a coluna `server` existe; host fora do cadastro **aborta a carga** com erro agregado | Pedido do dono: parar e pedir o cadastro do servidor |
| D10 | `excludePatterns` | **Valem sempre**, inclusive para o que a query trouxe; a tela **mostra** o que foi excluído e por qual padrão | Pedido do dono: o padrão tem de ser respeitado |
| D11 | Persistência | `gui.selections: [{ name, server, database, sql }]` no `mustel.jsonc` | Mesma seção das demais preferências da janela |
| D12 | Carga | **Sob demanda** (`⟳`), sem cache entre cliques: cada `⟳` reexecuta a query | Decisão do dono: sob demanda; sem estado velho escondido |
| D13 | Lista grande (1.000+) | **Paginação** + busca por servidor/base; a seleção sobrevive a ambas | Decisão do dono; 1.000+ registros não cabem numa rolagem útil |
| D14 | Falha/lixo na query | **Mantém a seleção anterior**; status com erro e linhas ignoradas | Menos destrutivo que limpar a tela |
| D15 | Execução | `ServerRun` com `databases` explícitas, `fetch_all_databases: false`, `all: false`, `exclude_patterns` **do cadastro**; servidor sem base marcada **não entra** | Reaproveita o plano do núcleo; evita execução vazia |

---

## 4. Arquitetura / Design

### 4.1 Fluxo

```text
Settings: query de seleção salva (servidor+banco+SQL)
      │
      ▼
Query tab ──[⟳ Apply all / ⟳ Apply (servidor)]──▶ usecases::custom_databases
      │                                             ├─ connect (cadastro + cofre)
      │                                             ├─ load(sql) → linhas
      │                                             └─ validate(linhas, cadastro)
      │                                                    │
      │◀────────────── seleção (servidor → bases) ─────────┘
      │
      ▼
Run query → QueryRunRequest { servers: [ServerRun { databases: marcadas }] }
```

### 4.2 Tipos novos (`mustel-core`)

```rust
/// Uma linha lida da query de seleção, já normalizada.
pub struct SelectionRow {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub user: Option<String>,
    /// Só em memória: nunca vai para arquivo, log ou mensagem.
    pub password: Option<String>,
    /// Nome do servidor cadastrado, quando a query traz a coluna `server`.
    pub server: Option<String>,
}

/// O que a seleção diz para um servidor.
pub struct ServerSelection {
    pub server: String,
    pub databases: Vec<DatabaseChoice>,
}

/// Uma base e por que (não) entra.
pub struct DatabaseChoice {
    pub name: String,
    /// `Some(padrão)` quando o `excludePatterns` do servidor barrou a base (D10).
    pub blocked_by: Option<String>,
    /// Aviso não bloqueante (ex.: usuário da string sem senha, D8).
    pub warning: Option<String>,
}
```

### 4.3 Leitura da linha (D6)

1. Se existir uma coluna de **connection string** (a primeira coluna de texto da linha, ou
   `connection_string`), ela é a verdade para `host`, `port`, `database`, `user` e `password`.
2. Senão, lê as colunas por nome: `host` (aceita `data source`), `port` (texto ou inteiro),
   `database` (`initial catalog`), `username` (`user`, `user id`), `password`, `server` (nome do
   servidor cadastrado, D9). **`server` não é alias de `host`**: a mesma coluna não pode ser as duas
   coisas, e a saída para a ambiguidade (D9) é mais útil — o alias `Server=` de host continua valendo
   dentro da *connection string*, onde o parser o resolve.
3. Linha sem `database` é **ignorada** e contada no status. Sem `host` **e** sem `server` também:
   a coluna `server` sozinha identifica a base, porque aponta para o cadastro (D9).

### 4.4 Validação (D9, D10, D7, D8)

- **Servidor**: coluna `server` (nome, sem diferenciar maiúsculas) tem precedência; senão casa
  `host`+`porta` (porta ausente = 5432). Zero candidatos → acumula em `unknown_hosts`; mais de um →
  acumula em `ambiguous`. Os dois **abortam** a carga, com mensagem agregada (hosts distintos, contagem
  de linhas e, na ambiguidade, os nomes candidatos).
- **Exclusão**: base que casa com algum `excludePatterns` do servidor entra com `blocked_by`.
- **Credencial**: `ServerRun.settings` sai do cadastro; `user`/`password` da linha sobrescrevem quando
  presentes. Sem `password` na linha: usa o cofre **se** o usuário for o do cadastro; senão `warning`
  (D8).

### 4.5 Ligação com a GUI

- **Descoberta é best-effort.** Descobrir "todos" roda por servidor e **não aborta** quando um falha:
  o que veio entra na universo, o que falhou vira aviso no status (servidor fora do ar não pode impedir
  os outros). É o oposto do `⟳ Apply`, que **aborta** em host não cadastrado (D9).

- `Request::ApplySelection { query: String, scope: SelectionScope }` (global ou servidor) e
  `JobResult::Selection { query, selection, ignored_rows }`; o job roda `load`+`validate`.
- `Request::DiscoverDatabases { server: Option<String> }` — o pedido **já existe** (linha de
  referência do autocomplete) e recebe `server: String`; passa a aceitar `None` para descobrir
  **todos** os servidores da lista. O `JobResult::Databases` já devolve a lista por servidor e
  continua servindo aos dois usos.
- Estado da tela: `SelectionState { by_server: BTreeMap<String, BTreeSet<String>>, query: Option<String>, page: usize, search: String }`.
- `Request::RunQuery` passa de `Vec<ServerConfigEntry>` para `Vec<(String, Vec<String>)>` (servidor +
  bases marcadas), mantendo a ordem da seleção.

### 4.6 Tela da aba Settings — cadastro das queries de seleção

A aba Settings ganha uma **segunda seção**, abaixo de **Appearance**. Mesmo padrão da aba Servers:
**uma lista** e **um formulário por vez**, com validação inline.

```text
Settings
┌─ Appearance ─────────────────────────────────────────────────┐
│ Interface font [Segoe UI ▾]  Editor font [Fira Code ▾]        │
│ Text size [──●────] 1.00x                                     │
└───────────────────────────────────────────────────────────────┘
┌─ Database selection queries ──────────────────────────────────┐
│                                                               │
│  Name    [Produção__________________________]                  │
│  Server  [configurador ▾]                                     │
│  Database[configurador_____] [⟳]   ← lista as bases do servidor│
│                                                               │
│  SQL                                                          │
│  ┌─────────────────────────────────────────────────────┐      │
│  │ select host, port, database                          │      │
│  │ from tenants                                         │      │
│  │ where ambiente = 'prod'                              │      │
│  └─────────────────────────────────────────────────────┘      │
│                                                               │
│  [Save]  [Test]  [Cancel]      ← Test mostra o resultado abaixo│
│  ✓ 312 rows · 4 servers · 7 excluded by patterns · 2 invalid   │
├───────────────────────────────────────────────────────────────┤
│  [＋ Add query]                                                │
│                                                               │
│  ▸ Produção        configurador / configurador    [Edit] [Remove]│
│  ▸ Staging         configurador / configurador    [Edit] [Remove]│
└───────────────────────────────────────────────────────────────┘
```

**Campos e regras**

| Campo | Como | Validação |
|---|---|---|
| `Name` | texto | obrigatório, único (sem diferenciar maiúsculas); é o rótulo do combo na aba Query |
| `Server` | combo com os servidores do **cadastro** | obrigatório; é ele que define a senha (cofre) |
| `Database` | texto livre + `⟳` que lista as bases do servidor escolhido | obrigatório; texto livre porque o cadastro costuma ser `fetchAllDatabases` (lista vazia) |
| `SQL` | caixa multilinha **com realce**, reusando `sql_editor::layout_job` | obrigatório, não vazio depois de `trim` |

**Comportamentos**

- **`Test`** roda `load` + `validate` e mostra **contagem, servidores e erros agregados** — nunca a
  senha, nunca as connection strings. É o mesmo caminho da carga real, então testar já pega host não
  cadastrado e ambiguidade.
- **Remover** é em dois toques (como o `Remove` da aba Servers): o botão troca para confirmar. Remover
  a query **em uso** no combo da aba Query deixa o combo em `—` (nada quebra).
- **Editar** abre o formulário preenchido, com o nome original guardado para permitir renomear (o
  mesmo cuidado de `SaveServer`, que compara com o nome original em vez do novo).
- **Nome duplicado** é recusado na hora de salvar, com a mensagem no formulário — não sobrescreve em
  silêncio.
- Gravação só ao **Save**: digitar no formulário não toca no arquivo.

### 4.7 Arquivos novos

- `crates/mustel-core/src/postgres/connection_string.rs` — parser Npgsql (movido do binário).
- `crates/mustel-core/src/usecases/custom_databases.rs` — `load`, `validate` e os tipos do §4.2.
- `crates/mustel-gui/src/screens/query/selection.rs` — estado e widget da lista paginada (se a tela de
  consulta ficar grande demais; senão, vive em `screens/query.rs`).
- `docs/reference/selecao-de-bases.md` e `docs/explanation/selecao-de-bases.md`.

### 4.8 Mudanças em arquivos existentes

| Arquivo | Mudança |
|---|---|
| `crates/mustel/src/commands/query/settings.rs` | passa a usar o parser do núcleo; **continua** ignorando a senha (paridade) |
| `crates/mustel-core/src/config/model.rs` | `GuiPrefs::selections` + `SelectionQuery` |
| `crates/mustel-core/src/usecases/mod.rs` | módulo novo |
| `crates/mustel-gui/src/request.rs` | `ApplySelection` novo; `DiscoverDatabases` ganha escopo (`Option<String>`); `RunQuery` passa a levar bases |
| `crates/mustel-gui/src/app.rs` | despacho dos jobs, guarda da seleção e das universos |
| `crates/mustel-gui/src/screens/query.rs` | lista `servidor → bases`, combo, botões, busca, paginação |
| `crates/mustel-gui/src/screens/settings.rs` | CRUD das queries de seleção + Testar |
| `docs/contrato-de-interface.md` | linhas novas (uma por decisão observável) |

---

## 5. Tarefas

### Fase 0 — Núcleo (sem UI, testável)

- [x] **T1** — Mover o parser Npgsql para `mustel-core::postgres::connection_string`, lendo `password`.
  - Depende de: —
  - Paralelizável com: T2
  - Concluído quando: o parser do núcleo devolve `host`, `port`, `database`, `user`, `ssl_mode`,
    `timeout` e `password`; o **CLI usa o do núcleo e continua ignorando a senha** (teste próprio
    garantindo a paridade do `--npgsql-connection-string`); os testes antigos de parsing continuam
    passando sem alteração de expectativa.
  - Arquivos: `crates/mustel-core/src/postgres/connection_string.rs`, `crates/mustel-core/src/postgres/mod.rs`,
    `crates/mustel/src/commands/query/settings.rs`

- [x] **T2** — `custom_databases::load(client, sql) -> Vec<SelectionRow>`.
  - Depende de: T1
  - Paralelizável com: —
  - Concluído quando: teste **gated** (`MUSTEL_PG_TEST_URL`) cria uma tabela com tenants (uma linha
    com connection string Npgsql, uma com colunas soltas, uma com `server` explícito) e confere a
    leitura; coluna de porta aceita **texto e inteiro**; linha sem host ou sem database é ignorada;
    sem a variável o teste se declara pulado.
  - Arquivos: `crates/mustel-core/src/usecases/custom_databases.rs`,
    `crates/mustel-core/tests/postgres_integration.rs`

- [x] **T3** — `custom_databases::validate(rows, &[ServerConfigEntry]) -> Result<Selection, SelectionError>`.
  - Depende de: T2
  - Paralelizável com: T4
  - Concluído quando: casa por `server` (nome) e por `host`+`porta`; **host fora do cadastro aborta**
    com `unknown_hosts` agregado (hosts distintos + contagem de linhas); **ambiguidade aborta** com os
    nomes candidatos — teste com **dois servidores no mesmo host e porta** (o caso real do dono:
    `Local` e `Local 2` em `127.0.0.1:5432`); base que casa com `excludePatterns` sai com
    `blocked_by` preenchido; base válida sai com `blocked_by: None`.
  - Arquivos: `crates/mustel-core/src/usecases/custom_databases.rs`

- [x] **T4** — Precedência de credencial (D7/D8).
  - Depende de: T3
  - Paralelizável com: —
  - Concluído quando: teste cobre a matriz — string com usuário **e** senha (ambos da string); string
    com usuário e **sem** senha (senha do cofre **se** usuário = do cadastro); string sem usuário nem
    senha (cadastro + cofre); usuário da string ≠ cadastro **sem** senha → `warning` e **não** marcada. E
    um teste garante que a senha **não** aparece em `Debug`/mensagem de erro nem em arquivo.
  - Arquivos: `crates/mustel-core/src/usecases/custom_databases.rs`

### Fase 1 — Configuração (D11)

- [x] **T5** — `GuiPrefs::selections: Vec<SelectionQuery>` com `skip_serializing_if` vazio.
  - Depende de: —
  - Paralelizável com: Fase 0
  - Concluído quando: arquivo sem seleções continua **byte a byte igual**; ida-e-volta preserva a
    lista; `settings db-servers add` **preserva** a seção (teste no core, como o das preferências de
    fonte); `SelectionQuery` tem `name`, `server`, `database`, `sql`.
  - Arquivos: `crates/mustel-core/src/config/model.rs`, `crates/mustel-core/src/usecases/gui_prefs.rs`

### Fase 2 — Aba Settings

- [x] **T6** — Seção **Database selection queries** na aba Settings (§4.6): lista + formulário.
  - Depende de: T5
  - Paralelizável com: —
  - Concluído quando: adicionar/editar/remover grava em `gui.selections`; **nome vazio ou duplicado é
    recusado** no formulário (sem sobrescrever em silêncio); **editar preserva o nome original** e
    permite renomear; **remover pede confirmação em dois toques**; remover a query em uso deixa o
    combo da aba Query em `—`; o campo `Database` é livre e o `⟳` lista as bases do servidor escolhido;
    a caixa de SQL tem realce (`sql_editor`); `Test` mostra **contagem + servidores + erros agregados**
    e **nunca** a senha nem a connection string; formulário só grava no `Save`.
  - Arquivos: `crates/mustel-gui/src/screens/settings.rs`, `crates/mustel-gui/src/request.rs`

### Fase 3 — Aba Query (a tela)

- [x] **T7** — Estado da seleção: `server → {bases}` + universo por servidor + paginação/busca.
  - Depende de: —
  - Paralelizável com: T5, T6
  - Concluído quando: testes de unidade cobrem — marcar/desmarcar preserva o resto; `⟳ Apply`
    **substitui** só o escopo pedido (global ou um servidor); a seleção **sobrevive** a mudar de página
    e a mudar a busca; serviço removido do cadastro leva a seleção dele junto; "recarga que perde base
    marcada" devolve a lista de sumidas para o aviso.
  - Arquivos: `crates/mustel-gui/src/screens/query/selection.rs` (ou `screens/query.rs`)

- [x] **T8** — Interface: combo + `⟳ Apply to all` + `⟳ Discover all`; por servidor `⟳ Apply` e
  `⟳ Discover`; lista com checkbox, busca e paginação; **excluídos desabilitados com tooltip do padrão**.
  - Depende de: T7, T6
  - Paralelizável com: —
  - Concluído quando: captura de tela mostra a lista com dois níveis, o paginador e um item excluído
    desabilitado; item excluído **não** fica marcável; o status após o `⟳ Apply` diz quantas bases
    entraram e quantas foram ignoradas (por padrão e por linha inválida); descobrir "todos" com um
    servidor fora do ar **não** impede os outros de aparecerem (aviso por servidor); verificado na
    janela pelo roteiro de `SKILL mustel-gui-smoke-test`.
  - Arquivos: `crates/mustel-gui/src/screens/query.rs`, `crates/mustel-gui/src/app.rs`

- [x] **T9** — `RunQuery` com bases explícitas (D15).
  - Depende de: T7
  - Paralelizável com: T8
  - Concluído quando: o pedido leva `(servidor, bases marcadas)` na ordem da lista; `ServerRun` sai com
    `fetch_all_databases: false`, `all: false` e `exclude_patterns` **do cadastro**; servidor sem base
    marcada **não entra**; teste garante que uma base marcada que casa com `excludePatterns` **não**
    chega ao plano (filtrada antes, com aviso na tela) e que o resto chega intacto.
  - Arquivos: `crates/mustel-gui/src/query.rs`, `crates/mustel-gui/src/request.rs`,
    `crates/mustel-gui/src/app.rs`

### Fase 4 — Contrato e documentação

- [x] **T10** — Linhas no manifesto e páginas Diátaxis.
  - Depende de: T8, T9
  - Paralelizável com: —
  - Concluído quando: cada decisão observável do §3 tem linha `IF-NNN` com o teste que a prova
    (`contract_manifest` verde); `docs/reference/selecao-de-bases.md` (o que existe: combo, botões,
    colunas aceitas, exclusão, paginação) e `docs/explanation/selecao-de-bases.md` (por que string é
    ponteiro para servidor cadastrado, por que exclusão aparece na tela, por que a seleção não persiste)
    estão linkadas em `docs/README.md`; `documentation_links_resolve` verde.
  - Arquivos: `docs/contrato-de-interface.md`, `docs/reference/selecao-de-bases.md`,
    `docs/explanation/selecao-de-bases.md`, `docs/README.md`

### Fase 5 — Verificação

- [x] **T11** — Gate completo + verificação na janela.
  - Depende de: T10
  - Paralelizável com: —
  - Concluído quando: `cargo test --workspace` verde (com e sem `gui`), `clippy -D warnings`, `fmt
    --check`, `cargo tree -p mustel-core` sem UI, `postgres_integration` verde com o teste novo, e o
    roteiro da skill `mustel-gui-smoke-test` executado (carregar uma seleção real, conferir marcação,
    desmarcar uma base, rodar e ver a base certa no CSV).
  - Arquivos: —

---

## 6. Estratégia de Verificação

- **Formatação:** `cargo fmt --all -- --check`
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Gate:** `cargo test --workspace` — verde **com** e **sem** a feature `gui`
- **Núcleo sem UI:** `cargo tree -p mustel-core` sem `egui`/`clap`/`inquire`/`indicatif`
- **Postgres real:** `MUSTEL_PG_TEST_URL=… cargo test -p mustel-core --test postgres_integration`
  (teste do `load` com connection string e com colunas)
- **Manifesto:** `cargo test -p mustel --test contract_manifest`
- **Paridade do CLI:** `cargo run -q -p mustel-testkit --bin oracle-compare` — 47 casos, 0 falhas
  (o parser continua no caminho do `--npgsql-connection-string`)
- **Janela:** roteiro da skill `mustel-gui-smoke-test` — carregar seleção, conferir marcação, desmarcar,
  rodar e ver o resultado; captura em `scratch://`

> O plano só pode ser marcado como `concluído` após **todas** as verificações acima passarem.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|---|---|---|---|
| Ambiguidade de servidor (`Local` e `Local 2` no mesmo host/porta) | **alta** | médio | coluna `server` tem precedência; sem ela, **erro** com candidatos em vez de adivinhar (T3) |
| `excludePatterns` barrar base que o usuário marcou e ele achar que é bug | alta | médio | item desabilitado com tooltip + contagem no status (T8); teste no T9 |
| Senha vazando em log/mensagem (agora que a string é lida) | baixa | **alto** | só em memória; teste garantindo que não aparece em `Debug`/erro/arquivo (T4); redação nas mensagens |
| Lista de 1.000+ travando a tela | média | alto | paginação + busca (D13); a seleção não depende do que está na página (T7) |
| Usuário marca, roda e a base não estava no plano | média | **alto** | o que está marcado é exatamente o que roda: exclusão aplicada **antes** com aviso (T9) |
| Parser do núcleo mudar o comportamento do CLI | baixa | alto | o CLI continua ignorando a senha; `oracle-compare` na verificação (T1) |
| Query do configurador lenta | média | baixo | sob demanda (D12), uma por clique, sem cache escondido |
| `mustel-core` ganhar dependência de UI | baixa | alto | `cargo tree` na verificação; tipos neutros (§4.2) |

---

## 8. Questões em Aberto

### Resolvidas

- [x] **Q1 — Modo Fixed × Custom: RESOLVIDA (2026-10-23, dono).** Não existem modos. Os servidores são
  os do cadastro; o que muda é **o que está marcado**. A query de seleção virou um **combo** com
  `⟳`, e no futuro pode haver várias (por isso `gui.selections` nasce como lista).
- [x] **Q2 — Carregar substitui ou soma: RESOLVIDA (dono).** **Substitui** — a query define uma seleção
  em massa; desmarcar depois é livre.
- [x] **Q3 — Escopo do `⟳`: RESOLVIDA (dono).** Os dois: global (all) e por servidor, para aplicar só
  no servidor 4.
- [x] **Q4 — Quais servidores aparecem: RESOLVIDA (dono).** Todos os do cadastro; muda só o marcado.
- [x] **Q5 — Origem do universo de bases: RESOLVIDA (dono).** Descoberta **e** query se somam; descobrir
  preenche o que a query não trouxe.
- [x] **Q6 — Credencial: RESOLVIDA com duas correções de rumo.** Eu propus "string vence"; o dono pediu
  "ignore a senha da string, use o cofre"; na revisão final decidiu **"o que vier da string vale, o que
  faltar vem do cadastro"** (D7), com a regra do usuário divergente sem senha (D8).
- [x] **Q7 — `excludePatterns` × seleção explícita: RESOLVIDA (dono).** Os padrões **valem sempre**,
  inclusive para o que a query trouxe (D10). O ponto que eu levantei (a seleção poder ser descartada em
  silêncio) virou requisito de tela: o excluído aparece desabilitado com o padrão no tooltip.
- [x] **Q8 — Paginação × virtualização: RESOLVIDA (dono).** **Paginação**, com busca; a seleção
  sobrevive a ambas (D13).
- [x] **Q9 — Falha na query: RESOLVIDA (dono).** Mantém a seleção anterior, status com erro e linhas
  ignoradas (D14).

**Nenhuma questão aberta.**

---

## 9. Log de Execução

Append-only.

- `[2026-10-23]` Plano criado em `rascunho`, a partir do desenho discutido com o dono. O desenho passou
  por três correções durante a conversa e todas estão no §8 (modos removidos, credencial revisada duas
  vezes, exclusão virou requisito de tela). Nada foi implementado ainda.
- `[2026-10-23]` **A tela do cadastro estava sem desenho.** O dono perguntou onde a query de seleção é
  cadastrada; o plano só tinha o requisito no T6. Acrescentado o **§4.6** com o esqueleto da seção
  "Database selection queries" (lista + formulário por vez, como a aba Servers), os quatro campos com
  as validações, e os comportamentos que o texto anterior não fixava: `Test` no mesmo caminho da carga,
  remover em dois toques, renomear preservando o nome original, e o campo de banco livre com `⟳`
  porque o cadastro costuma ser `fetchAllDatabases` (lista vazia). O T6 passou a exigir cada um desses
  pontos. Seções 4.7/4.8 renumeradas.
- `[2026-10-23]` **Fase 0 e T5 concluídos.** `connection_string` no core (T1), `custom_databases`
  com `load`/`validate`/`resolve_credential` (T2–T4) e `GuiPrefs::selections` (T5). Testes: 219 no
  core (todos unitários) e 10 de integração Postgres com `MUSTEL_PG_TEST_URL`, incluindo o `load`
  novo contra cenário real (connection string, colunas soltas, `server` explícito, porta como texto
  e como inteiro, linha sem host/banco contada). Duas correções de desenho aplicadas no caminho:
  (a) **4.3** dizia que `host` aceitava a coluna `server` e que `server` era o nome do servidor
  cadastrado — a mesma coluna não pode ser as duas; ficou `server` = nome do cadastro, e o alias
  `Server=` continua só na connection string. (b) `load` descartava linha sem `host`, o que anulava
  a coluna `server`; agora uma linha vale com `database` + (`host` **ou** `server`). A senha não
  aparece em `Debug` de `SelectionRow` nem de `ResolvedCredential` (teste `IF-226`), e
  `Chosen::to_prefs` virou `apply_to(&mut GuiPrefs)` — gravar a aparência não pode apagar as
  queries salvas.
- `[2026-10-23]` **Fases 2–5 concluídas; plano fechado.** Aba Settings com a seção "Database
  selection queries" (lista + formulário, `Test`, `⟳` do banco, realce no SQL), estado da seleção
  em `crates/mustel-gui/src/selection.rs` (T7), tela da aba Query com combo + quatro botões + lista
  em dois níveis + busca/paginação + excluídos desabilitados com tooltip (T8), `RunQuery` com bases
  explícitas (T9), linhas **IF-223..IF-231** no manifesto e as duas páginas Diátaxis (T10), e o gate
  completo (T11). Decisão de desenho no caminho: `Chosen::to_prefs` virou `apply_to(&mut GuiPrefs)`,
  porque a seção `gui` é gravada inteira e montar só a aparência apagaria as queries salvas.
  **Verificação:** `cargo test --workspace` verde (com e sem `--no-default-features`), 219 testes no
  core + 107 na GUI, `clippy -D warnings`, `fmt --check`, `postgres_integration` com 10 testes contra
  Postgres real, `cargo tree -p mustel-core` sem UI, `oracle-compare` 47 casos / **0 falhas**.
  Na janela (Windows, entrada sintética, capturas em `scratch://`): combo listou `Smoke`, `⟳ Apply to
  all` marcou `dev` (status "1 server(s), 1 database(s) selected"), `postgres` apareceu **cinza** com
  tooltip "Excluded by the server's excludePatterns: postgres", marcar/desmarcar `test2` mudou o
  contador (2→1 de 3), o CSV saiu **só** com `Local,dev`, a seção nova e o formulário renderizaram
  (com realce e a lista do `⟳`), `Test` mostrou "'Smoke' — 1 server(s), 1 database(s) selected", nome
  vazio foi recusado com "The query needs a name." sem gravar, e o `Save` válido gravou
  `gui.selections`. Não verificado por clique: o reset do combo ao remover a query em uso (os botões
  se deslocam quando o formulário abre; coberto pelo teste `removing_the_query_in_use_resets_the_combo`)
  — o "Confirm remove" dos dois toques foi confirmado visualmente.
- `[2026-10-23]` **Follow-ups observados (não bloqueiam o plano).** (a) Salvar qualquer coisa na aba
  Settings reescreve a seção `gui` inteira e passa a gravar `"interfaceFont": null` e
  `"editorFont": null` — comportamento herdado do `GuiPrefs`, não desta mudança, mas agora mais
  frequente porque o CRUD mora na mesma aba; `skip_serializing_if` nos dois fecharia isso. (b) A
  descoberta de bases (`⟳ Discover all`) dispara uma tarefa por servidor no runtime; com dezenas de
  servidores vale medir. (c) `⟳ Apply` com query apontando para servidor renomeado falha com "not
  registered" em vez de sugerir o nome novo.
- `[2026-10-23]` **Correção de um buraco achado na revisão final:** servidor do cadastro **sem base
  conhecida** não virava linha na lista, então o `⟳ Discover` daquela linha era inalcançável (contra
  R2). Agora ele aparece como bloco, com "0 of 0" e a dica "no database listed yet — use ⟳ Discover";
  teste `a_server_without_known_databases_still_shows_up` e conferido na janela com um cadastro de
  dois servidores (o segundo inexistente, para forçar a linha). Contagem final do gate: **521 testes,
  0 falhas** (com e sem `--no-default-features`).
- `[2026-10-23]` **Ambiente de teste manual** (`scripts/selection-fixture.ps1`). O dono pediu um
  Postgres com cadastro de tenants para exercitar a aba Query à mão. O script é idempotente, lê o
  `mustel.jsonc` do próprio dono e gera as linhas a partir dos servidores cadastrados (hoje `Local` e
  `Local 2`, ambos em `127.0.0.1:5432` — o caso de ambiguidade), criando a base `configurar` com a
  tabela `tenants` e seis vistas: `v_selecao` (caminho feliz, mistura connection string e colunas),
  `v_selecao_ambigua` (sem `server`), `v_selecao_host_desconhecido`, `v_selecao_string`,
  `v_selecao_string_sem_server` e `v_selecao_porta_texto`. Senhas nas strings são fictícias: a
  execução usa o cofre. Verificado na janela: `⟳ Apply to all` marcou **5 bases em 2 servidores**,
  com status "2 server(s), 5 database(s) selected, 2 blocked by excludePatterns, 1 without a usable
  password, 1 row(s) ignored" — as quatro contagens do §4.6 ao mesmo tempo.
  **Dois defeitos encontrados ao montar o fixture** e corrigidos: (a) linha com connection string
  **e** coluna `server` era tratada como ambígua, porque a leitura pela string descartava a coluna —
  agora `server` sobrevive e desempata (D9), com teste no `postgres_integration`; (b) base com aviso
  de credencial era marcada pelo `⟳ Apply`, contra R4b — agora fica desmarcada, com o motivo no
  tooltip, e o teste `a_warned_database_is_not_marked_by_apply` cobre.
- `[2026-10-23]` **Ajuste do ambiente a pedido do dono:** ele procurava a base pelo nome
  `configurador` (o do §4.2/§4.6) e não a achava, porque a primeira versão do script criava
  `configurar`. O padrão passou a ser `configurador`, e a tabela `tenants` agora tem as **duas
  formas preenchidas em cada linha** — a connection string Npgsql completa (`Host=…;Port=…;Database=…
;Username=…;Password=…`) **e** as colunas separadas (`host`, `port`, `database`, `username`,
  `password`) — mais `nome` como chave primária, `ambiente` e a coluna `server` do desempate. As
  vistas separam os caminhos: `v_selecao` lê pelas colunas, `v_selecao_string` pela string. A linha
  do usuário `ana` fica **sem** `Password=` de propósito (é o aviso de R4b) e `linha-sem-base` não
  tem string (é a linha ignorada). Verificado na janela com as duas vistas: ambas marcam 5 bases em 2
  servidores, com "2 blocked by excludePatterns, 1 without a usable password"; a de colunas ainda
  conta "1 row(s) ignored". A base `configurar` da primeira execução ficou para trás (o dono decide
  se remove).
- `[2026-10-23]` **Bug do `⟳` do campo Database (aba Settings) — diagnosticado e corrigido.** O dono
  relatou que o botão "não funcionava". A causa era dupla: (a) o único efeito era uma linha fraca
  **embaixo do SQL**, longe do campo, então parecia que nada acontecia; (b) quando a descoberta
  falhava (servidor fora do ar, senha ausente no cofre) o app **engolia** o erro — o `apply` só
  tratava `Ok`. A pedido do dono, a mudança ficou maior que a correção: escolher o **servidor** já
  dispara a descoberta, o formulário mostra **spinner + "Loading databases…"** enquanto carrega,
  erro aparece em amarelo no próprio formulário, e o campo ganhou um **dropdown com busca** (`▾`)
  sobre as bases descobertas, com a opção `Use "…"` para o nome digitado — o campo de texto continua
  livre para quem não tem descoberta.
  **Detalhe de `egui` que custou tempo:** popup aberto por **memória** (`Popup::is_id_open` +
  `toggle_id`) desaparece no quadro seguinte, porque o `begin_pass` limpa popups que ninguém mantém
  abertos; consultar a memória *antes* de reabrir fazia o popup nunca aparecer. Além disso, o clique
  que abre cai "fora" do popup (ele ainda está em *sizing pass*) e fechava tudo na hora. A solução foi
  guardar o aberto/fechado **no estado do formulário** (`SelectionForm::db_open`) e fechar por `Esc`,
  clique fora ou escolha. Coberto por teste headless em dois quadros
  (`the_database_picker_shows_a_search_box_and_the_options`), mais um teste de sanidade que prova que
  o harness pinta popup.
  **Verificado na janela:** trocar o servidor para `Local 2` carregou as bases sozinho ("4
  database(s) on 'Local 2'"); o `▾` abriu o popup com busca; digitar `te` filtrou para `test2` e
  ofereceu `Use 'te'`; escolher `test2` preencheu o campo e fechou o popup. O spinner **não** foi
  capturado em imagem (a descoberta termina em <400 ms) — está coberto pelo teste.
  **Bônus de diagnóstico:** descobri que a área cliente da janela começa em (0,23) e não em (-8,-8),
  o que explicava os cliques que "erravam" a posição nas verificações anteriores.
- `[2026-10-23]` **Campo `Database` virou dropdown puro, a pedido do dono.** Ele apontou que o campo
  de digitação manual ao lado do dropdown perdeu o sentido e pediu o formato dos combos de fonte
  ("Interface font"): um botão só, `-` quando nada está escolhido, e o nome depois de escolher. O
  `TextEdit` saiu do formulário; o dropdown mostra `-  ▼`, abre com campo de **busca** e lista as
  bases descobertas, e a entrada `Use "…"` continua aceitando um nome que a descoberta não viu — o
  §4.6 continua valendo (nome livre possível) sem devolver o campo de texto à tela.
  Teste novo (`the_database_combo_shows_a_dash_until_something_is_picked`) garante o `-` e a
  ausência do campo de digitação. **Verificado na janela:** o formulário mostra `-  ▼`; abrir traz
  busca + `configurador/dev/postgres/test2`; escolher `test2` deixa o botão com `test2 ▼`; e o
  `Save` gravou `"database": "test2"` no arquivo.
- `[2026-10-23]` **Combo com busca virou um widget único, e o vazio deixou de mostrar `-`.** O dono
  pediu três coisas de uma vez: sem escolha o campo fica **vazio** (mais consistente que `-`), o
  mesmo dropdown com busca aparece **nas fontes** e **na escolha da query de seleção** da aba Query,
  e sai o comportamento de transformar o texto digitado em opção (`Use 'aasdas'`). Feito em
  `crates/mustel-gui/src/search_combo.rs`, um widget que recebe `id`, texto do botão e `&[Item]` e
  devolve o índice escolhido — os três chamadores (fontes, base da query, query salva) só montam a
  lista. O estado do popup (aberto/fechado e busca) vive na memória do `egui` por `id`, então dois
  combos na mesma tela não compartilham a busca e `SelectionForm` perdeu os campos `db_filter` e
  `db_open`. O `ComboBox` do `egui` saiu dos dois lugares onde era usado.
  **Verificado na janela:** `Database` aparece **em branco** com `▼`; a fonte abre com `Search…` e a
  lista (Fira Code embutida, Agency FB, Algerian, …); o combo de queries da aba Query abre com
  `Search…`, entrada vazia + `Producao`/`Homologacao`, e digitar `hom` filtrou para `Homologacao`.
  Em nenhum dos três aparece `Use '…'`. Testes novos no widget: botão vazio/nome escolhido, busca
  filtrando sem maiúsculas, dica de busca com filtro vazio, e busca não compartilhada entre combos.
- `[2026-10-23]` **"Quadrado azul" no combo de queries (aba Query).** O dono viu um retângulo azul sem
  texto como primeira linha do dropdown. Era a entrada "nenhuma query" com texto vazio: escolhida,
  o egui pinta o fundo da seleção e, sem rótulo, sobra um quadrado. Duas correções: a primeira linha
  passou a ser **`(none)`** (o **botão** continua vazio quando nada está escolhido, que é o desenho
  que o dono pediu), e o widget agora **não desenha item sem texto** — assim nenhum outro chamador
  repete o defeito. O mapeamento "voltar para nenhuma" é por **posição** (índice 0), não por texto,
  então uma query batizada de `(none)` não confunde. Verificado na janela: a lista mostra `(none)`
  destacado, `Producao` e `Homologacao`, e nenhum quadrado sem rótulo.
  Teste novo: `an_item_without_text_is_not_drawn`.
