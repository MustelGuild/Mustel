# Decisões de engenharia

> **Por que esta página existe.** Até 2026-10-24 as decisões moravam nos planos de `MustelPlans/`,
> misturadas com as tarefas. As tarefas viraram código, os planos foram removidos e o **porquê**
> ficou — aqui. `MustelPlans/` passou a guardar apenas trabalho ainda não executado (hoje, só
> [`release-pendencias.md`](../../MustelPlans/release-pendencias.md)).

Uma decisão não vira código: ela explica **por que** o código é assim. Quem for mudar um
comportamento começa por aqui — se a decisão que justifica aquele comportamento não existir mais,
então a mudança precisa de uma decisão nova, não de um "ajuste".

## Como ler um ID de decisão

O repositório conviveu com **três numerações independentes**, uma por plano. O mesmo número quer
dizer coisas diferentes em cada uma: `D6` é "keyring" na migração e "formato da linha" na seleção
de bases. O número sozinho **não** resolve nada — use o texto ao lado dele e a tabela de resolução
abaixo.

### Onde cada número é citado

| ID | Numeração | Onde aparece |
|---|---|---|
| `D5` | Migração | divergência 6 em [`known-divergences`](../../fixtures/known-divergences.md) (GUI em egui, não Tauri) |
| `D6` | Migração | doc-comments de `crates/**`, manifesto, divergência 1 (keyring) |
| `D7` | Seleção de bases | manifesto `IF-226` (credencial da linha) |
| `D8` | Seleção de bases | manifesto `IF-226`, `custom_databases.rs` (aviso de credencial) |
| `D9` | Migração | `mustel-gui/**`, manifesto `IF-001`/`IF-180`/`IF-224` (binário único, núcleo em processo) |
| `D10` | Migração **ou** Seleção | nome/paths do produto (`IF-131`) × `excludePatterns` (`IF-225`, `gui/query.rs`, `gui/reference.rs`) |
| `D11` | Migração **ou** Seleção | crates principais × `gui.selections` (`IF-229`/`IF-234`, `config/model.rs`) |
| `D12` | Migração | `tokio-postgres`, em `crates/mustel-core/src/postgres/**` |
| `D15` | Migração **ou** Seleção | versão inicial `1.0.0` × bases explícitas (`IF-228`, `gui/query.rs`) |
| `D24` | Migração | `.github/workflows/release-build.yml` (CI só empacota) |
| `D25` | Migração | manifesto `IF-071`/`IF-072` (`--create-if-not-exists`) |
| `D26` | Migração | `publish.ps1` / `publish.sh` (release manual) |

Os casos com duas numerações (`D10`, `D11`, `D15`) são os que exigem atenção: o texto entre
parênteses ao lado do ID indica o tema, e o tema diz a tabela.

### `R9` e `R10`: citações sem tabela de origem

`crates/mustel-gui/src/selection.rs` e o manifesto (`IF-228`) citam `R9` e `R10`, mas **não existe
tabela `R*`** em nenhum lugar do repositório — nem antes desta consolidação. O comportamento que o
código implementa e que o número representa:

- **`R9`** — base barrada por `excludePatterns` do cadastro **nunca** pode ser marcada, e a exclusão
  nunca é silenciosa: a base entra na lista desabilitada, com o motivo visível.
- **`R10`** — o `Run query` leva **só** o servidor que tem base marcada.

## Migração para Rust (`D1`–`D27`)

| ID | Tema | Decisão | Por quê |
|---|---|---|---|
| `D1` | Motivação dominante | Aprender Rust **+** performance | Define metas de perf medidas e tolerância a retrabalho |
| `D2` | Branch da migração | `feat/rust-migration` a partir de `main` | Escolha do usuário |
| `D3` | Convivência .NET/Rust | Workspace Cargo na raiz; .NET em `legacy/` até o corte | Permite usar o .NET como oráculo em todos os comandos |
| `D4` | Branches pendentes (`api`, `gui`, `update`, `quality`) | Fora de escopo, não migradas | Evita alvo móvel |
| `D5` | Linguagem/UI da GUI | **egui** (immediate mode), sem Tauri/Angular | Foco em performance; Tauri traria webview + stack JS |
| `D6` | Credenciais | `keyring` nativo, **sem** migração de senhas DPAPI; config começa vazia | O usuário aceitou reentrada de senhas |
| `D7` | Alvos de distribuição | win-x64 e linux-x64 (gnu); **macOS fora desde 2026-10-21** | macOS exigiria runner Apple; manter Linux com GUI troca `musl` por `gnu` |
| `D8` | Escada de fidelidade | N1 superfície, N2 comportamento/artefatos, N3 interativo (obrigatórios); N4 formatação (não) | Paridade testável sem perseguir byte-match de `--help`/ANSI — detalhe em [Escada de fidelidade](escada-de-fidelidade.md) |
| `D9` | Integração CLI ↔ GUI | Núcleo compartilhado + eventos, ambos em processo; **binário único** `mustel` com `gui` (`eframe` atrás da feature `gui`) | Preferência do usuário; crates separados deixam dois binários como uma linha de `[[bin]]` |
| `D10` | Nome/paths do produto | Binário `mustel`; config `mustel.jsonc` no diretório do SO | Consistência com o rename |
| `D11` | Crates principais | `clap`, `tokio`, `inquire`, `indicatif`, `tabled`, `csv`, `chardetng`+`encoding_rs`, `globset`+`walkdir`, `serde`+`jsonc-parser`, `dirs`, `tracing`, `thiserror`/`anyhow`, `keyring`, `tokio-util`, `eframe`/`egui` | Equivalentes diretos das dependências anteriores |
| `D12` | Driver Postgres | **`tokio-postgres`** (`sqlx` descartado) | `Row`/`Column`/`Type` para mapear tipos arbitrários em CSV; `sqlx` empurra para tipos conhecidos em compilação, e `query run` executa SQL livre — detalhe em [Por que `tokio-postgres`](por-que-tokio-postgres.md) |
| `D13` | Convenções do repositório | CLI em **inglês**; plano/docs/conversa em **pt-BR**; commits em inglês | Regra vigente no `AGENTS.md` |
| `D14` | Mutações de Git | Exigem autorização explícita do dono | Guard rails do `AGENTS.md` |
| `D15` | Versão inicial | `1.0.0`; tags `v1.x.y` dirigem o release | Mustel é software novo |
| `D16` | Oráculo | Binário .NET em `legacy/` é a fonte de verdade; baseline versionado em `fixtures/baseline/` | Onde docs e código divergem, vale o **código observado** |
| `D17` | Canais de distribuição | crates.io (`mustel-core`, `mustel-gui`, `mustel`) + GitHub Releases + winget `Mustel.CLI` | Nomes verificados livres no crates.io |
| `D18` | Repositório GitHub | Permanecer em `liphvf/FurLab` durante a migração, renomear depois | Renomear no meio misturaria duas mudanças; **consumado em 2026-10-24** (`MustelGuild/Mustel`) |
| `D19` | Escopo da migração | Grupos `query`, `database` e `settings` **completos**; demais no backlog | Decisão do usuário |
| `D20` | Licença | `AGPL-3.0-only` | O que está no `LICENSE`; README/docs/scripts alinhados em 2026-10-24 |
| `D21` | Backlog | Grupos fora do escopo voltam numa **segunda onda**, depois do 1.0 | Não é descarte |
| `D22` | Pacote winget antigo | `FurLab.CLI` permanece como está | Não mexer no antigo |
| `D23` | Notarização macOS | Fora do escopo; documentar `xattr -d com.apple.quarantine` | macOS está fora do release |
| `D24` | Automação de CI | **Removida pelo dono** (commit `3c81ff1`): workflows e scaffolding de agentes não existem mais; gate roda à mão | O CI vermelho a cada push atrapalhava o desenvolvimento |
| `D25` | Criar banco no `restore` | **Falhar por padrão** (paridade) + `--create-if-not-exists`, criando pelo driver | O `CreateDatabaseIfNeeded` do legado, via `psql`, nunca criava nada — divergência 11 |
| `D26` | Release sem CI | Dois scripts locais (`publish.ps1`, `publish.sh`) geram o pacote + SHA256; **publicação manual** | O dono publica de `packaging/` onde quiser |
| `D27` | Workflow só para empacotar | Um workflow (`release-build.yml`), apenas `workflow_dispatch`; **nenhuma porta de qualidade** | Compatível com `D24`: o ruído era o gate vermelho, não o empacotamento |

## Autocomplete do editor (`D1`–`D20`)

| ID | Tema | Decisão | Por quê |
|---|---|---|---|
| `D1` | Fonte dos nomes | **Metadados** (catálogo), não DDL | Autocomplete precisa de nomes e tipos, não de `CREATE` |
| `D2` | DDL | **Fora** desta rodada | `pg_dump --schema-only` já faz bem |
| `D3` | Escopo do esquema | **Um banco de referência** | Quem roda em massa assume bancos iguais |
| `D4` | Descoberta | O `⟳` descobre os bancos **antes** de carregar, quando o servidor usa `fetchAllDatabases` | Sem isso o seletor não teria o que listar |
| `D5` | Disparo do popup | Automático com **2+ letras** e `Ctrl+Space` | Fluidez com previsibilidade |
| `D6` | O que completa | Tabelas, colunas, funções e **aliases da consulta** | É o que economiza digitação de verdade |
| `D7` | Onde vive a lógica | Núcleo (`usecases::schema`, `completion`), sem UI | Testável sem janela; `mustel-core` não depende de egui |
| `D8` | Peça de sugestão | Função pura `suggest(texto, cursor, &schema)` | Recebe texto e esquema, devolve candidatos |
| `D9` | Cache | Em memória por `(servidor, banco)`; `⟳` recarrega | Evita ida ao banco a cada tecla |
| `D10` | Sem senha no cofre | Botão **desabilitado**, com tooltip | A janela não pergunta senha |
| `D11` | String/comentário | **Não** sugere dentro de string ou comentário | Reusa `sql_editor::tokens`; sugerir ali é ruído |
| `D12` | Volume | Teto de **50** sugestões; prefixo antes de substring | Catálogo grande não pode travar a lista |
| `D13` | Popup | `CompletionPopup` do egui 0.37 | Resolve teclas, foco, `Esc` e inserção |
| `D14` | Tela de edição | **Adiada** | Deixou de ser necessária tecnicamente |
| `D15` | Tipo do candidato | `Candidate { insert, detail }` no núcleo; a GUI converte em `egui::Suggestion` | Mantém o egui fora do núcleo |
| `D16` | Nome com schema | Nome simples quando inequívoco; `schema.tabela` quando há homônimos | Ambíguo sem prefixo vira SQL inválido |
| `D17` | CTEs | Nomes das CTEs sugeridos em `FROM`/`JOIN`; colunas delas **não** | Colunas exigiriam análise sintática; o nome já evita digitar |
| `D18` | Assinatura de função | Carregada **junto** com o esquema (`pg_get_function_arguments`); um candidato por nome, assinaturas na descrição | O custo é no `⟳`, uma vez; a cada tecla só se lê memória |
| `D19` | Disparo do popup (corrige `D5`) | Abre no **começo de um token** e a partir da **primeira** letra; `Ctrl+Space` em qualquer posição | Com o mínimo de 2 letras a lista não abria onde é mais útil |
| `D20` | Posição da linha de referência | **Antes** do editor de SQL, no topo do formulário | O esquema alimenta as sugestões, então se escolhe o molde antes de escrever |

## Seleção de bases na janela (`D1`–`D15`)

| ID | Tema | Decisão | Por quê |
|---|---|---|---|
| `D1` | Modelo da tela | **Sem modos**: lista de servidores do cadastro + combo de queries de seleção com `⟳` | Os servidores são os mesmos; o que muda é o que fica marcado |
| `D2` | Escopo do `⟳ Apply` | **Todos** (botão do combo) ou **um servidor** (botão da linha) | Aplicar a query só num servidor |
| `D3` | Semântica do `⟳ Apply` | **Substitui** a seleção pelo resultado da query | A query define uma seleção em massa; desmarcar depois é livre |
| `D4` | Universo da lista | **Todos** os servidores do cadastro, cada um com as bases que conhece | Entre execuções muda só o marcado |
| `D5` | Origem das bases | **Descoberta** (`pg_database`) **+** resultado da query, somando | A query pode não conhecer tudo; descobrir preenche o resto |
| `D6` | Formato da linha | **Connection string** quando presente; senão `host, port, database` (+ `username`, `password`, `server` opcionais) | "Assuma a string como verdade" |
| `D7` | Credenciais | O que vier da string/colunas vence; o que faltar vem do cadastro + **cofre** | Revisão final do dono |
| `D8` | Usuário da string ≠ usuário do cadastro **sem** senha na string | Linha vira **aviso** e **não** é marcada | A senha do cofre é de outro usuário e falharia sem explicar |
| `D9` | Validação de host | **Obrigatória**: casa por `host`+`porta`, ou pelo **nome** quando existe a coluna `server`; host fora do cadastro **aborta a carga** com erro agregado | Parar e pedir o cadastro do servidor |
| `D10` | `excludePatterns` | **Valem sempre**, inclusive para o que a query trouxe; a tela mostra o excluído e o padrão | O padrão tem de ser respeitado |
| `D11` | Persistência | `gui.selections: [{ name, server, database, sql }]` no `mustel.jsonc` | Mesma seção das demais preferências da janela |
| `D12` | Carga | **Sob demanda** (`⟳`), sem cache entre cliques | Sem estado velho escondido |
| `D13` | Lista grande (1.000+) | **Paginação** + busca por servidor/base; a seleção sobrevive a ambas | 1.000+ registros não cabem numa rolagem útil |
| `D14` | Falha/lixo na query | **Mantém a seleção anterior**; status com erro e linhas ignoradas | Menos destrutivo que limpar a tela |
| `D15` | Execução | `ServerRun` com `databases` explícitas, `fetch_all_databases: false`, `all: false`, `exclude_patterns` do cadastro; servidor sem base marcada **não entra** | Reaproveita o plano do núcleo; evita execução vazia |

## O que não veio para cá

- **Tarefas** (`T1`–`T48` e afins): viraram código; o registro de execução vive no histórico do Git.
- **Questões abertas** (`Q1`–`Q12`): todas fechadas; a resposta de cada uma é a origem registrada na
  tabela da migração.
- **`DIV1`–`DIV18`**: continuam em [`fixtures/known-divergences.md`](../../fixtures/known-divergences.md),
  junto com a lista branca de resíduos do nome antigo e as regras de comparação.
- **Contrato do CLI**: não é decisão, é superfície verificada — vive em
  [`contrato-de-interface.md`](../contrato-de-interface.md), uma linha por `IF-NNN` com o teste que
  a prova.
