# Plano — Autocomplete de SQL na GUI

> **Status:** concluído
> **Criado em:** 2026-10-23
> **Atualizado em:** 2026-10-23
> **Complexidade:** média (núcleo simples; o valor está no motor de sugestão)
> **Depende de (outros planos):** `contrato-de-interface-testavel.md` (concluído)

---

## 1. Visão Geral

Quem escreve SQL na janela hoje digita tudo de cabeça: nome de tabela, nome de coluna, alias. O CLI já
sabe quais bancos existem; a janela não sabe nada sobre o **esquema**.

Este plano liga as duas pontas: o núcleo passa a ler o catálogo do Postgres (tabelas, views, colunas e
funções) de **um banco de referência**, e o editor de SQL completa o que está sendo digitado — com
contexto (`FROM` sugere tabela, `SELECT` sugere coluna, `c.` sugere as colunas daquela tabela).

**O que mudou durante o desenho:** eu tinha afirmado que o `TextEdit` do egui comeria `Tab`/`Enter` e
que seria preciso consumir as teclas à mão (e que talvez isso justificasse uma tela de edição
separada). Fui à fonte e me corrigi: o **egui 0.37 já traz `CompletionPopup`** para `TextEdit`, com o
comentário literal *"Consume the popup keys before the `TextEdit` sees them"*. Ele resolve teclas,
foco, `Esc`, reset de seleção e inserção. O nosso trabalho é **o motor de sugestão** — que é, de todo
modo, a parte que dá valor.

Consequência: o autocomplete entra no editor **inline**, e a tela de edição separada fica fora (volta
como conveniência, se doer).

---

## 2. Escopo

**Inclui:**

- `usecases::schema::load` no `mustel-core`: catálogo → `Schema` (tabelas, views, colunas e funções).
- `completion::suggest` no `mustel-core`: função pura `(texto, cursor, &Schema) -> Vec<Candidate>` com
  contexto por palavra-chave, mapa de **aliases** e ranking por prefixo.
- Carregamento na GUI: `Request::LoadSchema`, cache em memória por `(servidor, banco)` e o botão
  `⟳` que **descobre os bancos e depois carrega** quando o servidor usa `fetchAllDatabases`.
- Linha "Referência do autocomplete" abaixo da lista de Servers: servidor, banco e `⟳`.
- Ligação do `CompletionPopup` do egui no editor inline: abertura automática com **2+ letras** e
  `Ctrl+Space`, `Tab`/`Enter` aceitam, `↑`/`↓` navegam, `Esc` fecha.
- Linhas no manifesto de contrato para as decisões de GUI que este plano fixa.

**Exclui explicitamente:**

- **DDL / versionar esquema** (decisão D2 do desenho): reavaliar depois de usar.
- **Tela de edição separada** (D14): o `CompletionPopup` tornou isso desnecessário; se voltar, é
  conveniência com a mesma peça.
- **Colunas de CTE/subquery** no autocomplete: exigiria análise sintática de verdade.
- **Vários bancos ao mesmo tempo**: a referência é **um** banco (D3).
- **Prompt de senha na janela**: sem senha no cofre o botão fica desabilitado (D10).
- Mudança de contrato do **CLI**: é GUI-only, nenhum `IF` de superfície de CLI muda.

> Se durante a execução surgir algo na lista "Exclui", **NÃO implementar**.
> Registrar na seção 8 e perguntar ao usuário.

---

## 3. Decisões

| # | Decisão | Escolha | Razão |
|---|---|---|---|
| D1 | Fonte dos nomes | **metadados** (catálogo), não DDL | autocomplete precisa de nomes e tipos, não de `CREATE` |
| D2 | DDL | **fora** desta rodada | o `pg_dump --schema-only` já faz isso bem; reavaliar a necessidade |
| D3 | Escopo do esquema | **um banco de referência** | quem roda em massa assume bancos iguais |
| D4 | Descoberta | o `⟳` **descobre os bancos antes** de carregar quando o servidor usa `fetchAllDatabases` | senão o seletor não teria o que listar |
| D5 | Disparo do popup | automático com **2+ letras** **e** `Ctrl+Space` | fluidez com previsibilidade |
| D6 | O que completa | tabelas, colunas, funções e **aliases da consulta** | é o que economiza digitação de verdade |
| D7 | Onde vive a lógica | núcleo (`usecases::schema`, `completion`), sem UI | testável sem janela; `mustel-core` não depende de egui |
| D8 | Peça de sugestão | função pura `suggest(texto, cursor, &schema)` | recebe texto e esquema, devolve candidatos: testável com esquema fictício |
| D9 | Cache | em memória por `(servidor, banco)`, `⟳` recarrega | evita ida ao banco a cada tecla |
| D10 | Sem senha no cofre | botão **desabilitado** com tooltip | a janela não pergunta senha (regra atual) |
| D11 | String/comentário | **não** sugere dentro de string ou comentário | reusa `sql_editor::tokens`; sugerir ali é ruído |
| D12 | Volume | teto de **50** sugestões, prefixo antes de substring | catálogo grande não pode travar a lista |
| D13 | Popup | **`CompletionPopup` do egui 0.37** | resolve teclas, foco, `Esc` e inserção — verificado na fonte |
| D14 | Tela de edição | **adiada** | deixou de ser necessária tecnicamente |
| D15 | Tipo do candidato | `Candidate { insert, detail }` no núcleo; a GUI converte em `egui::Suggestion` | mantém o egui fora do núcleo |
| D16 | Nome com schema | As duas formas convivem; **insere o nome simples quando é inequívoco** e **qualifica (`schema.tabela`) quando há mais de um schema com o mesmo nome** | Decisão do dono (2026-10-23): ambíguo sem prefixo vira SQL inválido, então o prefixo entra só quando é necessário |
| D17 | CTEs | Os **nomes** das CTEs da consulta são sugeridos em `FROM`/`JOIN`; as colunas delas **não** | Decisão do dono (2026-10-23): meio-termo barato. Colunas exigiriam análise sintática; o nome já evita digitar |
| D18 | Assinatura de função | Carregada **junto** com o esquema (`pg_get_function_arguments`), em memória; **um** candidato por nome, com as assinaturas na descrição | Decisão do dono (2026-10-23): carregar tudo de uma vez não deixa o autocomplete mais lento — o custo é no `⟳`, uma vez, e a cada tecla só se lê memória |
| D19 | Disparo do popup (corrige D5) | Abre no **começo de um token** (logo após `FROM `, por exemplo) e a partir da **primeira** letra digitada; `Ctrl+Space` abre em qualquer posição | Correção do dono (2026-10-23) sobre D5: com o mínimo de 2 letras, a lista não abria exatamente onde ela é mais útil — escolher a tabela do `FROM` sem digitar o nome inteiro |

---

## 4. Arquitetura / Design

### 4.1 Fluxo

```text
[janela]                                     [núcleo]

screens/query.rs
  └─ linha "Referência do autocomplete"
       [servidor ▾] [banco ▾] [⟳]
             │ clique
             ▼
     Request::LoadSchema { server, database }
             │ app.rs → runtime (tokio)
             ▼
     usecases::schema::load(&client)  ──────► pg_catalog (2 consultas)
             │                                  tables/views/columns + functions
             ▼
     JobResult::Schema(Ok(Schema))  ────► cache em memória
             │
             ▼
     editor (TextEdit + CompletionPopup)
       suggest(query) ──► completion::suggest(text, cursor, &schema)
                            └─ contexto + aliases + ranking → Vec<Candidate>
```

### 4.2 Tipos novos (`mustel-core`)

```rust
pub struct Schema {
    pub tables: Vec<Table>,
    pub functions: Vec<Function>,
}

pub struct Table {
    pub schema: String,      // nspname ("public")
    pub name: String,
    pub kind: RelationKind,  // Table | View | MaterializedView | Partitioned
    pub columns: Vec<Column>,
}

pub struct Column {
    pub name: String,
    pub type_name: String,   // format_type(...)
}

pub struct Function {
    pub schema: String,
    pub name: String,
    /// Argumentos como o tipo os descreve (`text, integer`); `None` quando sem argumentos.
    pub arguments: Option<String>,
}

pub struct Candidate {
    pub insert: String,      // o que entra no texto
    pub detail: Option<String>, // ex.: tabela de origem ou tipo da coluna
}
```

### 4.3 Consultas do catálogo

Relações e colunas, numa passada:

```sql
SELECT n.nspname, c.relname, c.relkind, a.attname,
       format_type(a.atttypid, a.atttypmod)
FROM pg_catalog.pg_class c
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_catalog.pg_attribute a
       ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
WHERE c.relkind IN ('r','v','m','p')
  AND n.nspname NOT IN ('pg_catalog','information_schema')
ORDER BY n.nspname, c.relname, a.attnum
```

Funções **com assinatura**, tudo numa consulta só:

```sql
SELECT n.nspname, p.proname, pg_get_function_arguments(p.oid)
FROM pg_catalog.pg_proc p
JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
WHERE n.nspname NOT IN ('pg_catalog','information_schema')
ORDER BY 1, 2
```

> O `pg_proc` devolve **uma linha por sobrecarga** (a mesma função com assinaturas diferentes). O núcleo
> agrupa por `(schema, nome)` na hora de montar o `Schema`, guardando as assinaturas distintas — assim o
> autocomplete vê **um** candidato por nome, com as assinaturas na descrição.

**Tudo é carregado uma vez, em memória.** O autocomplete roda a cada tecla e **não** fala com o banco:
recebe o `Schema` já em memória. O custo de trazer assinaturas é pago **no carregamento** (que é por
clique no `⟳`, não por tecla), e é pequeno: catálogos típicos têm dezenas a poucas centenas de funções, e
a lista de argumentos é curta.

### 4.4 Regras de sugestão (o miolo)

1. **Palavra sob o cursor** — a `CompletionQuery` já entrega `word` e `word_range`; o `word_boundary`
   inclui `A-Z a-z 0-9 _ . "`, para que `c.` faça parte da palavra (sem o `.`, o alias se perde).

   > **Correção (2026-10-23):** este item estava invertido. O `word_boundary` do egui pergunta quais
   > caracteres **separam** palavras (ele fica com o trecho *depois* do último separador), então o que
   > se passa é o **complemento** do que está escrito acima. A regra virou uma só no núcleo
   > (`completion::is_word_boundary`, o complemento exato de `is_word_char`) e o GUI só a repassa.
   > Do jeito anterior a "palavra" era o espaço anterior ao identificador: o motor e o popup
   > discordavam de qual trecho seria substituído — nada abria ao digitar e aceitar grudava o texto
   > (`select` + `nome` = `selectnome`).
2. **Contexto** — com `sql_editor::tokens`, varrer para trás e achar a última palavra-chave:

   | Última palavra-chave | Sugere |
   |---|---|
   | `FROM`, `JOIN`, `INTO`, `UPDATE`, `TABLE` | tabelas, views e **CTEs da própria consulta** |
   | `SELECT`, `WHERE`, `ON`, `GROUP`, `HAVING`, `ORDER`, `SET` | colunas das tabelas citadas + funções |
   | nenhuma (início) | palavras-chave + tabelas |

   **CTEs (decisão do dono, meio-termo):** os **nomes** de CTE (`WITH nome AS (...)`, incluindo várias
   separadas por vírgula) entram como candidatos em `FROM`/`JOIN`. As **colunas** delas continuam fora
   (Q1): exigiriam interpretar o `SELECT` interno. Consequência assumida: `totais.` não sugere nada,
   mas `FROM to` já oferece `totais`.

3. **Aliases** — pareamento `FROM|JOIN <tabela> [AS] <alias>` a partir dos tokens; `c.` resolve para a
   tabela do alias `c`.

4. **Schema no nome (decisão do dono)** — as duas formas convivem:

   | Situação | O que é sugerido | O que é inserido |
   |---|---|---|
   | nome de tabela existe em **um** schema | `clientes` | `clientes` |
   | mesmo nome em **dois ou mais** schemas | `clientes` **com o schema no prefixo** | `vendas.clientes` |
   | usuário já digitou `schema.` | só as tabelas daquele schema | `vendas.clientes` |
   | prefixo digitado casa com um **alias** da consulta | colunas daquela tabela | `c.nome` (só o nome da coluna) |

   Regra curta: **nome simples quando é inequívoco; qualificado quando há ambiguidade.** Ambiguidade é
   contada **entre schemas** — duas tabelas homônimas em schemas diferentes exigem o prefixo, porque
   senão o SQL nem compila.

5. **Ranking** — prefixo exato primeiro, depois substring; removidos duplicados; corte em 50.
6. **Fora de string/comentário** — se o cursor cai dentro de um token `String` ou `Comment`, devolve
   vazio.

### 4.5 Ligação com o egui

```rust
let output = egui::CompletionPopup::new(id)
    .word_boundary(sql_completion::word_boundary)
    .accept_keys([egui::Key::Tab, egui::Key::Enter])
    .align(egui::RectAlign::BOTTOM_START)
    .show(ui, text_edit, |query| {
        sql_completion::candidates(&mut trigger, query, schema.as_deref(), force)
    });
```

> **Correção (2026-10-23):** o esboço original passava a **fronteira invertida** (os caracteres que
> *formam* a palavra). O código de produção não monta o predicado aqui: chama
> [`crate::sql_completion`], que repassa `completion::is_word_boundary`. O `force` vem de
> `sql_completion::force_requested`, que lê os modificadores do **próprio evento** de tecla — o
> estado do quadro depende de um `ModifiersChanged` ter chegado antes, e o atalho morria quando isso
> não acontecia.

O retorno traz `accepted: Option<Suggestion>` (para telemetria/teste) e `is_open: bool` (para desligar
atalhos nossos enquanto o popup está aberto).

### 4.6 Arquivos novos

- `crates/mustel-core/src/usecases/schema.rs` — o `load` e os tipos.
- `crates/mustel-core/src/completion.rs` — motor de sugestão (puro).
- `crates/mustel-gui/src/schema_cache.rs` — cache por `(servidor, banco)` e estado do carregamento.

### 4.7 Mudanças em arquivos existentes

- `crates/mustel-core/src/usecases/mod.rs` — registra `schema`.
- `crates/mustel-core/src/lib.rs` — registra `completion`.
- `crates/mustel-gui/src/request.rs` — `Request::LoadSchema`, `JobResult::Schema`.
- `crates/mustel-gui/src/app.rs` — despacho e guarda do resultado no cache.
- `crates/mustel-gui/src/screens/query.rs` — linha de referência, estados do botão e o popup no editor.
- `crates/mustel-gui/src/sql_editor.rs` — expor a consulta "cursor está em string/comentário?".
- `docs/contrato-de-interface.md` — linhas `IF-202`.. para as decisões de GUI.

---

## 5. Tarefas

### Fase 0 — Núcleo (sem UI, testável)

- [x] **T1** — `usecases::schema::load(client)` com as duas consultas do §4.3 e os tipos do §4.2.
  - Depende de: —
  - Paralelizável com: T2
  - Concluído quando: teste gated (`MUSTEL_PG_TEST_URL`) cria schema/tabela/view/função temporários,
    chama `load` e confere que aparecem com as colunas e tipos certos; **função com dois parâmetros
    vem com a assinatura preenchida** e sobrecargas do mesmo nome viram **um** item; roda também contra
    o container de verificação e passa; `cargo test --workspace` verde sem a variável (teste se declara
    pulado).
  - Arquivos: `crates/mustel-core/src/usecases/schema.rs`, `crates/mustel-core/src/usecases/mod.rs`,
    `crates/mustel-core/tests/postgres_integration.rs`

- [x] **T2** — `completion::suggest(text, cursor_char, &schema)` com contexto, aliases e ranking (§4.4).
  - Depende de: —
  - Paralelizável com: T1
  - Concluído quando: os testes cobrem, com esquema fictício: `FROM ` sugere tabelas; `SELECT ` sugere
    colunas das tabelas citadas; `c.` sugere as colunas de `clientes` dado `FROM clientes c`; `SELECT`
    em `FROM x` não vaza coluna de `y`; string/comentário devolve vazio; ranking põe prefixo antes de
    substring; teto de 50 respeitado; **tabela homônima em dois schemas sai qualificada**
    (`vendas.clientes`), enquanto nome único sai simples (`clientes`); digitando `vendas.` só aparecem
    as tabelas daquele schema; **`WITH totais AS (...)` faz `FROM ` sugerir `totais`**, e `totais.`
    continua sem sugestão (limite declarado em Q1); função vem com a assinatura na descrição.
  - Arquivos: `crates/mustel-core/src/completion.rs`, `crates/mustel-core/src/lib.rs`

- [x] **T3** — `sql_editor::cursor_in_literal_or_comment(text, cursor_char) -> bool`, para o motor pular
  string e comentário sem reimplementar o tokenizer.
  - Depende de: —
  - Paralelizável com: T1, T2
  - Concluído quando: testes cobrem cursor em string, em comentário de linha, em comentário de bloco e
    fora de ambos.
  - Arquivos: `crates/mustel-gui/src/sql_editor.rs`

### Fase 1 — Carregamento na GUI

- [x] **T4** — `Request::LoadSchema { server, database }` + `JobResult::Schema`; o job resolve a conexão
  (cadastro + cofre) e chama `usecases::schema::load`; `schema_cache.rs` guarda por `(servidor, banco)`.
  - Depende de: T1
  - Paralelizável com: T5
  - Concluído quando: teste de unidade com cache verifica guardar/recuperar/invalidar; sem senha no
    cofre o pedido devolve erro claro (string) em vez de tentar prompt.
  - Arquivos: `crates/mustel-gui/src/request.rs`, `crates/mustel-gui/src/app.rs`,
    `crates/mustel-gui/src/schema_cache.rs`, `crates/mustel-gui/src/lib.rs`

- [x] **T5** — Linha "Referência do autocomplete" abaixo da lista de Servers: combo de servidor, combo de
  banco, botão `⟳`; estados (sem senha, carregando, erro, carregado) e **descobrir + carregar** (§D4).
  - Depende de: T4
  - Paralelizável com: —
  - Concluído quando: com a janela aberta, escolher servidor/banco e clicar em `⟳` popula o cache e o
    rótulo mostra o banco carregado; sem senha o botão está desabilitado com tooltip; erro aparece na
    linha de status.
  - Arquivos: `crates/mustel-gui/src/screens/query.rs`

### Fase 2 — O popup no editor

- [x] **T6** — Ligar o `CompletionPopup` no `TextEdit` do SQL (§4.5): `word_boundary` com `.` e `"`,
  `accept_keys` Tab/Enter, `suggest` convertendo `Candidate` → `egui::Suggestion`, abertura automática
  com 2+ letras e `Ctrl+Space` forçando.
  - Depende de: T2, T3, T5
  - Paralelizável com: —
  - Concluído quando: captura de tela mostra o popup aberto sobre o editor; `Ctrl+Space` abre com
    palavra de 1 letra; `Tab`/`Enter` inserem; `Esc` fecha e não reabre até a palavra mudar; digitar em
    string/comentário não abre.
  - Arquivos: `crates/mustel-gui/src/screens/query.rs`

- [x] **T7** — Registro no manifesto: linhas `IF-202`+ para "referência por banco", "cache e `⟳`",
  "popup com 2+ letras e Ctrl+Space", "não sugere em string/comentário", com os testes que as provam.
  - Depende de: T6
  - Paralelizável com: —
  - Concluído quando: `cargo test -p mustel --test contract_manifest` verde e o resumo do manifesto
    atualizado (total/coberta/pendente/nao-coberta).
  - Arquivos: `docs/contrato-de-interface.md`

### Fase 3 — Verificação

- [x] **T8** — Rodar a estratégia da seção 6 inteira, registrar no log e fechar o plano.
  - Depende de: todas
  - Paralelizável com: —
  - Concluído quando: fmt, clippy, `cargo test --workspace`, suíte gated de Postgres, manifesto e a
    captura do popup estão registrados; plano marcado `concluído`.
  - Arquivos: `MustelPlans/autocomplete-sql.md`

---

## 6. Estratégia de Verificação

- **Formatação:** `cargo fmt --all -- --check`
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Gate:** `cargo test --workspace` — verde, **com** e **sem** a feature `gui`
- **Núcleo sem UI:** `cargo tree -p mustel-core` sem `egui`/`clap`/`inquire`/`indicatif`
- **Postgres real:** `MUSTEL_PG_TEST_URL=postgres://postgres:dev@127.0.0.1:55432/mustel_tutorial cargo test -p mustel-core --test postgres_integration`
  — verde, incluindo o teste de `schema::load`
- **Manifesto:** `cargo test -p mustel --test contract_manifest` — verde, com as linhas novas
- **Visual:** captura da janela com o popup aberto (feature `__screenshot` temporária, revertida), como
  foi feito no editor de SQL e no botão "ir ao fim"
- **Manual (o que só o humano vê):** digitar `SELECT c.` com `FROM clientes c`, aceitar com `Tab`, e
  confirmar que o texto final é o esperado

> O plano só pode ser marcado como `concluído` após **todas** as verificações acima passarem.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|---|---|---|---|
| Popup abrir sozinho o tempo todo e atrapalhar | média | médio | mínimo de 2 letras, `Esc` respeitado (a lib guarda `dismissed_word`) e só abrir com candidatos |
| `word_boundary` errado quebra o caso `c.` | média | alto | teste explícito do alias; `.` e `"` no boundary |
| Catálogo grande (10 mil tabelas) travar o quadro | média | médio | ranking + teto de 50 + sugestão calculada a cada quadro (texto curto) |
| Esquema desatualizado depois de DDL | alta | baixo | `⟳` recarrega; o rótulo mostra o que está carregado |
| Sem senha no cofre ⇒ feature parece quebrada | média | médio | botão desabilitado **com tooltip** explicando |
| Permissão esconde objetos e o usuário acha que falhou | baixa | baixo | comportamento correto: só oferece o que pode usar; tooltip do `⟳` diz "objetos visíveis ao usuário" |
| `mustel-core` ganhar dependência de UI por acidente | baixa | alto | `cargo tree` na verificação; `Candidate` neutro (D15) |

---

## 8. Questões em Aberto

### Resolvidas

- [x] **Q1 — Colunas de CTE e subquery: RESOLVIDA PARCIALMENTE (2026-10-23, dono).** O dono aceitou o
  **meio-termo barato**: os **nomes** das CTEs entram como candidatos em `FROM`/`JOIN`; as **colunas**
  delas continuam fora. Virou D17 e está nas regras (§4.4) e no critério do T2. O que continua de fora,
  e por quê: uma CTE (`WITH totais AS (SELECT ...)`) é relação temporária que não está no catálogo, e
  sugerir `totais.s` exigiria interpretar o `SELECT` interno (projeções, `*` expandido, `UNION`) —
  análise sintática de verdade. Um atalho heurístico acerta o caso simples e erra os casos reais, o que
  é pior que não sugerir. O mesmo vale para `FROM (SELECT ...) t`.

- [x] **Q2 — Assinatura de função: RESOLVIDA (2026-10-23, dono).** Carregamos **tudo de uma vez**: a
  consulta de funções já traz `pg_get_function_arguments(p.oid)`, o  `Schema` guarda as assinaturas e o
  autocomplete lê **só memória** a cada tecla. Virou D18. A pergunta do dono estava certa: consultar por
  sugestão seria lento; carregar no `⟳` não custa nada por tecla. O custo real é no carregamento (uma
  consulta só, não N+1) e no tamanho: catálogos típicos têm dezenas a poucas centenas de funções, com
  listas de argumentos curtas. `pg_proc` devolve **uma linha por sobrecarga**, então o núcleo agrupa por
  `(schema, nome)` e mostra **um** candidato por nome com as assinaturas na descrição.

- [x] **Q3 — Nomes qualificados por schema: RESOLVIDA (2026-10-23, dono).** As duas formas convivem.
  Insere-se o **nome simples** quando ele é inequívoco; se o **mesmo nome existir em mais de um schema**,
  a sugestão sai **com o prefixo do schema** (`vendas.clientes`). Digitando `schema.`, sugerem-se só as
  tabelas daquele schema. Virou D16 e está nas regras (§4.4) e no critério do T2.

**Nenhuma questão aberta pendente.** As três foram respondidas em 2026-10-23 (Q1 parcialmente).

---

## 9. Log de Execução

Append-only.

- `[2026-10-23]` Plano criado em `rascunho`, a partir do desenho discutido. P1 (inline primeiro) e P2
  (linha abaixo de Servers) entraram **pela recomendação**, para revisão do dono. Fato que mudou o
  desenho: o egui 0.37 já traz `CompletionPopup` (`widgets/text_edit/completion.rs`), que consome as
  teclas antes do `TextEdit` — verificado na fonte, não presumido.
- `[2026-10-23]` **Revisão 1 — Q3 resolvida pelo dono e Q1/Q2 explicadas.** O dono decidiu que as duas
  formas de nome convivem, com o prefixo do schema entrando **só quando há ambiguidade** (D16), e pediu
  explicação melhor de Q1 e Q2. Q1 agora descreve o caso concreto (`WITH totais AS (...)` + `totais.`)
  e por que resolver exige análise sintática, com o meio-termo barato (só os nomes das CTEs). Q2 mostra
  o problema real (sobrecarga: `count` com vinte e poucas variantes), o custo da consulta extra e a
  regra para quando voltar. O critério do T2 ganhou o caso de ambiguidade entre schemas.
- `[2026-10-23]` **Revisão 2 — Q1 e Q2 resolvidas pelo dono.** (Q1) aprovado o **meio-termo**: nomes de
  CTE entram em `FROM`/`JOIN`, colunas não (D17). (Q2) o dono perguntou "não podemos carregar tudo em
  memória?" — sim, e é o desenho: a assinatura vem **na mesma consulta** do esquema
  (`pg_get_function_arguments`), paga uma vez no `⟳`; a cada tecla só se lê memória (D18). Isso
  **corrigiu uma afirmação minha**: eu tinha dito que "custa consulta extra por sugestão", o que
  confundia custo de carregamento com custo de digitação. Também removi um número que eu havia
  inventado ("vinte e poucas variantes de `count`"): a quantidade varia por versão do Postgres e não
  foi medida. Os tipos (`Function::arguments`), a consulta, as regras (§4.4), os critérios de T1/T2 e a
  seção 8 foram atualizados. Seção 8 agora tem **três resolvidas e nenhuma aberta**.
- `[2026-10-23]` Execução iniciada. **T1 concluída** — `usecases/schema.rs`: `Schema`/`Table`/`Column`/
  `Function` e `load()` com as duas consultas. Detalhe que a execução exigiu: `pg_class.relkind` é do
  tipo `"char"`, não `text` — sem `relkind::text` o `Row::get::<String>` falha ao deserializar (o teste
  gated pegou). `map_error` do `postgres::client` passou a ser público (é a classificação SQLSTATE→10/11,
  já testada; duplicar seria pior). Teste gated novo cria tabela, view e **duas sobrecargas** de função,
  lê o esquema e confere colunas/tipos/assinaturas agrupadas: `postgres_integration` **9/9**.
- `[2026-10-23]` **T2 concluída** — `completion.rs` com `suggest(texto, cursor, &schema)` e 12 testes.
  **Duas correções de desenho que os testes forçaram**: (a) eu só olhava o texto *antes* do cursor para
  achar `FROM`/alias, mas as tabelas costumam estar **depois** (`SELECT c. FROM clientes c`) — agora a
  referência vem da consulta inteira e só o **contexto** vem do prefixo; (b) palavra-chave só aparece
  quando não há contexto (`FROM ` não sugere `select`). Cobre: relações, homônimo qualificado, filtro
  por schema, colunas das tabelas citadas, alias, função com assinatura, CTE em `FROM`, nada em
  string/comentário, ranking e teto de 50.
- `[2026-10-23]` **T3 concluída, com correção de rumo** — o plano dizia "reusar o tokenizer do realce",
  mas isso criava **duas** implementações da regra de aspas/comentários (a do realce no GUI e a do
  motor no núcleo), e a do GUI errava o limite (tratava o espaço depois do fecha-aspas como "dentro").
  Unifiquei: `mustel_core::completion::cursor_in_literal_or_comment` é a **única** implementação, e o
  `sql_editor` do GUI delega. Ela trata string/comentário não terminados e a aspa escapada, e o teste
  de limite (IF-204) ficou no núcleo.
- `[2026-10-23]` **T4 e T5 concluídas** — `Request::LoadSchema` / `JobResult::Schema`, `SchemaCache`
  (chave `(servidor, banco)`), e a linha "Referência do autocomplete" abaixo da lista de Servers com
  combo de servidor, combo de banco e `⟳`. Sem senha no cofre a janela avisa e não carrega (nunca
  pergunta). Captura confirmou a linha renderizada com os dois combos, o `⟳` e o rótulo de estado.
- `[2026-10-23]` **T6 concluída** — `CompletionPopup` do egui ligado ao `TextEdit`: `word_boundary` com
  `.` e `"`, `Tab`/`Enter` aceitam, `Ctrl+Space` força, e o mínimo de 2 letras vale sem o atalho. O
  motor do núcleo é consultado a cada quadro; o popup fica **desligado** quando não há esquema
  carregado (devolve lista vazia). Teste headless novo (`tests/popup_probe.rs`) prova que o popup
  chama o motor e entrega o texto da caixa — num contexto sem foco o `word` sai vazio, e isso está
  documentado no teste em vez de escondido.
- `[2026-10-23]` **T7 concluída** — manifesto ganhou **IF-202..IF-208** (cache por par, recarga,
  literais/comentários, relações+CTEs+qualificação, alias, funções com assinatura, teto/ranking), cada
  uma com o teste que a prova. O meta-teste barrou duas vezes durante a execução (marcador sem linha e
  linha sem marcador) — exatamente o que ele existe para fazer.
- `[2026-10-23]` **T8 concluída — plano concluído.** `cargo fmt --check` limpo; `clippy --all-targets
  --all-features -D warnings` limpo; **`cargo test --workspace` = 447 testes, 0 falhas** (eram 426);
  `-p mustel --no-default-features` = 144/0; `cargo tree -p mustel-core` sem `egui`/`eframe`/`clap`/
  `inquire`/`indicatif`; `postgres_integration` **9/9** incluindo o teste do catálogo; manifesto verde
  com 135 linhas (132 cobertas, 0 pendentes, 3 não-cobertas). Container de teste removido; só o
  `postgres-ptbr` do dono ficou.
- `[2026-10-23]` **Correção pós-conclusão, a partir do uso real (relato do dono).** Três sintomas:
  (1) `select name from test` não abria a lista de tabelas; (2) só abria ao digitar `*`, e o texto
  aceito saía grudado (`selecttexto from bench_bkp`); (3) `Ctrl+Space` não fazia nada, enquanto três
  espaços abriam a lista.

  **Causa raiz (1 e 2):** o `word_boundary` do egui recebe os caracteres que **separam** palavras, e o
  código passava os que **formam** a palavra. A "palavra" virava o espaço anterior ao identificador.
  Com isso o mínimo de 2 letras media **espaços**, não letras: `from te` estava sempre bloqueado, e a
  lista só abria quando acumulavam 2 caracteres separadores — daí o `*` (palavra `" *"`) e o terceiro
  espaço. E o aceite apagava o espaço que ele acreditava ser a palavra, inserindo no lugar dele.

  **Causa raiz (3) — duas, e a segunda só apareceu com a janela de verdade.** O `force` lia
  `input.modifiers` (estado do quadro), que só reflete o Ctrl se um `ModifiersChanged` já tiver sido
  processado; os modificadores do **próprio evento** de tecla são a fonte confiável (é neles que o
  egui-winit escreve `command` = Ctrl fora do Mac). Corrigir isso **não bastou**: instrumentando a
  janela, o log mostrou `force=true` chegado corretamente e `is_open=true` no quadro da tecla,
  seguido de `is_open=false` no quadro seguinte — o popup abria e fechava em ~16 ms, invisível. O
  atalho vale num quadro só e o popup é reavaliado a cada quadro; faltava memória entre eles
  (`Trigger`). Lição registrada: o teste headless que eu tinha conferia **um** quadro, então passava
  com o popup piscando. Os testes novos conferem os quadros seguintes.

  **O que mudou:**
  - `mustel_core::completion::is_word_boundary` (complemento de `is_word_char`) é a **única** regra de
    fronteira; `crate::sql_completion` a repassa ao popup.
  - `sql_completion::force_requested` lê os modificadores do evento; `wants_suggestions` abre no
    começo de um token e a partir da **primeira** letra (D19 corrige D5), e mantém o mínimo só para o
    editor vazio **no começo do texto**, onde não há contexto — ali só `Ctrl+Space` abre.
  - `sql_completion::Trigger` guarda entre quadros o pedido do `Ctrl+Space` (vale enquanto o cursor
    não sai de onde foi pressionado) e a pausa depois de um aceite (a lista não reabre com o nome
    recém-inserido, já selecionado, na frente de quem quer digitar o resto).
  - O popup passou a abrir **abaixo** da caixa (`.align(BOTTOM_START)`): o padrão do egui desenhava a
    lista **acima** do editor, tapando o cabeçalho da janela.
  - A ligação saiu de `screens/query.rs` para `crates/mustel-gui/src/sql_completion.rs`, onde as três
    decisões (fronteira, gatilho, pedido ao motor) são testáveis sem janela.
  - Manifesto ganhou **IF-209** (palavra e trecho substituído) e **IF-210** (gatilho), cada um com o
    teste que prova. O resumo do manifesto estava desatualizado desde IF-202..IF-208 (dizia 128/125
    para 135 linhas); agora diz 137/134, conferido por contagem.

  **Testes:** `tests/popup_probe.rs` deixou de só provar que o popup chama o motor — agora digita como
  o winit entrega (um `Event::Text` por caractere) e confere a palavra, a abertura depois do `FROM `,
  o `Tab` sem comer o espaço, o `Ctrl+Space` **e os quadros seguintes**, o fechamento depois do
  aceite e o silêncio dentro de string/comentário. A primeira versão do teste afirmava que "`word`
  sai vazio num contexto headless"; era falso — saía vazio por causa da fronteira invertida, e a
  afirmação escondia o bug.

  **Verificação na janela de verdade** (teclado simulado via `SendInput`, captura por `CopyFromScreen`):
  `select * from ` abriu a lista com `bench_bkp` e `test` do banco `dev` real; digitar `ben` filtrou
  para `bench_bkp`; `Tab` produziu `select * from bench_bkp` (com o espaço); e `Ctrl+Space` abriu a
  lista sem nada digitado. **Limitação conhecida, que não é nossa:** o egui guarda o `Esc` pela
  *palavra* (`dismissed_word`), então quem fecha com `Esc` numa palavra vazia (logo depois de um
  espaço) não vê a lista reaparecer em **nenhuma** palavra vazia até digitar uma letra. Fica
  registrado; corrigir exigiria mexer no estado interno do `CompletionPopup`.
- `[2026-10-23]` **Consolidação na documentação (Diátaxis) e auditoria do que foi dado como concluído.**
  Três páginas novas: `docs/reference/autocomplete.md` (o que existe, tecla por tecla, limites e
  limitações), `docs/how-to/autocomplete-no-editor.md` (carregar e usar) e
  `docs/explanation/autocomplete.md` (por que assim). Nenhuma página de tutorial: não há lição a
  executar — o autocomplete é ferramenta de uso, não habilidade a adquirir (e exigiria banco e cofre
  para uma aula que o how-to já cobre).

  Ao escrever a referência **contra o código** (e não contra este plano), dois itens marcados `[x]`
  não batiam com o que a janela faz. Ficam registrados como pendência, não como concluídos:
  - **D4/T5 — descoberta de bancos não existe.** O combo lista só `databases` do cadastro; não há
    chamada de descoberta no caminho do `⟳`. Consequência: servidor com `fetchAllDatabases` fica com
    o combo vazio (`nenhum banco no cadastro`) e **não consegue** usar o autocomplete. T5 dizia
    "descobrir + carregar (§D4)" e está `[x]`.
  - **D10 — o botão não é desabilitado por falta de senha.** `can_load` só olha servidor e banco; sem
    senha no cofre o clique acontece e a barra de status mostra `No password stored for '<servidor>'`.
    A parte "a janela nunca pede senha" está de pé; a parte "botão desabilitado com aviso" não.

  Nenhum dos dois quebra o contrato já publicado (IF-202..IF-210 não descrevem esses estados), mas
  ambos eram afirmações do plano que a documentação não podia repetir. As páginas agora descrevem o
  comportamento real e listam as duas como limitação/lacuna.
- `[2026-10-23]` **As duas lacunas foram fechadas (decisão do dono: o combo deve mostrar os bancos do
  servidor).** O código agora bate com o que o plano prometia:
  - **Descoberta de bancos (D4/T5).** Escolher um servidor dispara `Request::DiscoverDatabases`, que roda
    em **background** e chama `usecases::db_servers::test_connection` (a mesma consulta `pg_database` do
    `settings db-servers test`). O resultado vira `JobResult::Databases` e alimenta o combo. Bancos
    descobertos ganham do cadastro; sem descoberta (ou com descoberta vazia), vale o cadastro — a janela
    nunca fica pior do que era. Servidor com `fetchAllDatabases` passou a funcionar.
  - **Botão por senha (D10).** O mesmo job responde se o cofre tem senha (`has_password`), então o `⟳`
    fica desabilitado quando **sabemos** que não há, com tooltip dizendo como guardar. Enquanto não se
    sabe, deixa tentar: o erro do cofre é claro e é melhor do que um botão que parece quebrado.

  A descoberta **não** passa pelo `spawn` do app (fila única, `Already running`): é rápida e não pode
  esperar uma consulta longa, nem desligar o `Running` dela. Daí o resultado dela não zerar `running` no
  `drain`.

  Achado no caminho, registrado como lição: a primeira verificação na janela parecia mostrar o estado
  velho depois de trocar de servidor, e eu quase "consertei" o que estava certo — o status desenhado era
  `'local2' has 3 database(s).` e eu lia `'local'` na captura reduzida. Instrumentar o estado no ponto
  do desenho resolveu a dúvida em uma linha; antes disso, gastei tempo suspeitando do `drain`, do
  repaint e do canal.

  Manifesto: **IF-211** (combo lista os bancos do servidor, com fallback para o cadastro) e **IF-212**
  (`⟳` desabilitado sem senha no cofre). Documentação atualizada: o combo e o botão na referência, os
  passos e o "se nada aparecer" no how-to, e uma seção nova na explicação sobre de onde vem a lista.
  `cargo test --workspace` = **465 testes, 0 falhas**.
