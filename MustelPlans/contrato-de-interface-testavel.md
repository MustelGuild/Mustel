# Plano — Contrato de interface testável (só CLI) e remoção do legado

> **Status:** concluído
> **Criado em:** 2026-10-21
> **Atualizado em:** 2026-10-22
> **Complexidade:** média-alta (o mecanismo é pequeno; o volume está no inventário e na remoção)
> **Depende de (outros planos):** `migracao-rust.md` (concluído — 39/39 tarefas)

---

## 1. Visão Geral

O comportamento que o Mustel deve ter foi decidido em prosa espalhada por três lugares:
`MustelPlans/migracao-rust.md` (D1–D27), `oracle/known-divergences.md` (**17** divergências aceitas,
rotuladas de 1 a 18 — o item 10 não existe) e `openspec/specs/**` (contrato herdado). A garantia de que
o código cumpre isso hoje vive em dois lugares que **não são `cargo test`**:

1. `oracle-compare`, um binário que roda **à mão** e compara o Mustel com 47 capturas do oráculo;
2. conferência humana durante a execução das tarefas.

O resultado é que a frase "cada decisão está coberta" não é nem verificável nem falsificável: não há
nada que ligue uma decisão ao teste que a prova, e apagar um teste não quebra build nenhum.

**O que já é verdade** (medido nesta rodada): são 330 testes no workspace e **nenhum** caminho de
`cargo test` executa o .NET. Mais importante: `compare_case()` em `mustel-testkit` já é uma função
pura sobre *(captura congelada, execução do Mustel)* — as regras de comparação (exit code e artefatos
como contrato duro, texto como N4, flags longas de ajuda como superfície) **não precisam do binário
antigo**. O legado só é necessário para *re-gravar* o baseline (`oracle-capture`), nunca para *provar*
o comportamento.

Três decisões do dono do produto moldam este plano:

- **Só a superfície do CLI** é contrato: era a única interface que o FurLab tinha. GUI, licença,
  canais de distribuição e scripts de release **não** entram (CD13).
- **O código legado será removido**, mas só depois de os testes provarem que nada se perde (CD14).
- **`CLAUDE.md` deixa de existir**: `AGENTS.md` passa a ser o guia único (CD15).

O plano, então, faz duas coisas em sequência deliberada: primeiro **converte o comportamento em teste**;
 depois **apaga o original** — porque a partir desse momento o teste é a única memória executável do
contrato.

---

## 2. Escopo

**Inclui:**

- `AGENTS.md` como guia único de desenvolvimento (conteúdo do `CLAUDE.md` absorvido) e remoção do
  `CLAUDE.md`.
- `docs/contrato-de-interface.md` — o **manifesto**: uma linha por decisão de superfície de CLI, com ID
  estável, origem, classe, o que precisa ser verdade, onde se observa e qual teste prova.
- Replay das **47 capturas** de `oracle/baseline/` dentro de `cargo test` — **45** comparadas com o
  baseline e **2** (divergências deliberadas) como teste explícito do comportamento Mustel (CD8) —
  reutilizando `mustel-testkit::compare::compare_case`, **uma** única implementação das regras de
  comparação.
- Meta-teste que torna a cobertura **falsificável**: linha sem teste e teste sem linha quebram a suíte.
- Prova de **cobertura de superfície**: todo comando migrado tem caso no baseline e toda flag longa é
  exercitada por um caso ou está numa lista de exceções com motivo — esta é a condição de entrada da
  remoção.
- Testes para o que o CLI revela sem ser uma invocação trivial: exit codes, artefatos, cofre (ausência
  de senha na config) e os fluxos interativos (N3).
- **Remoção do `legacy/`** (130 arquivos rastreados) e dos artefatos que só existem por causa dele,
  com marcador git (`tag`) para que o código continue recuperável.
- Limpeza dos resíduos fora do `legacy/`: skills C# `furlab-command-manager` (em `.claude/` e
  `.opencode/`) e as configs de debug `.vscode/launch.json`/`tasks.json` (CD17).
- **Documentação nova em Diátaxis** (CD16): `docs/` recriado do zero para o Mustel, nos quatro
  quadrantes — tutorial, how-to, referência e explicação — em markdown.
- Aposentadoria das peças do harness que dependem do legado (`oracle-capture`, `oracle-perf`).

**Exclui explicitamente:**

- **GUI**: não existia no FurLab, logo não há paridade a garantir (Q1). Os testes da GUI continuam
  existindo no repositório — só não entram no manifesto nem nesta rodada.
- Decisões de **produto e distribuição**: licença, canais, scripts de release, workflow de
  empacotamento, `repository` do Cargo.toml. Não são superfície de CLI (Q1).
- Implementar a **segunda onda** (D21: `file`, `claude`, `opencode`, `winget`, `windowsfeatures`,
  `docker`). O código deles sai do diretório de trabalho mas fica no histórico do Git (CD14).
- Reescrever `openspec/specs/**`: as specs do produto .NET ficam como **histórico** (o contrato vivo
  passa a ser o manifesto + os testes).
- Traduzir a documentação nova para inglês nesta rodada: `docs/` sai em pt-BR (CD16).
- Publicação (crates.io/winget) — segue em `release-pendencias.md`.

> Se durante a execução surgir algo na lista "Exclui", **NÃO implementar**.
> Registrar na seção 8 (Questões em Aberto) e perguntar ao usuário.

---

## 3. Decisões

Decisões travadas na exploração. O agente **não reabre** sem justificativa e aval do usuário.

| #   | Decisão | Escolha | Razão |
|-----|---------|---------|-------|
| CD1 | Fonte de verdade da cobertura | `docs/contrato-de-interface.md` versionado, uma linha por decisão | É o único artefato que permite perguntar "esta decisão tem teste?" e obter resposta por comando, não por leitura |
| CD2 | Identidade das decisões | ID único `IF-NNN`; a origem (`D9`, `DIV5`, `T16`, `CLI query run`) vai em coluna própria | Renumerar D/DIV quebraria as referências já escritas no repositório; o namespace novo absorve todas as origens sem colidir |
| CD3 | Vínculo manifesto ↔ teste | Coluna `Teste` = `<arquivo>::<fn>` **e** marcador `// CONTRACT: IF-NNN` no teste | O caminho prova que o teste citado existe; o marcador prova que nenhum teste se declara dono de decisão ausente do manifesto |
| CD4 | Duas camadas de teste | (1) replay duro dos 47 casos; (2) snapshots do texto onde o texto é o contrato. Mais os testes de decisão que não são invocação simples | A captura separa contrato (exit code, artefatos) de redação (N4); testar texto no lugar certo evita aceitar deriva de mensagem como se fosse formatação |
| CD5 | Implementação das regras de comparação | Reutilizar `mustel-testkit::compare::compare_case`; **não** reimplementar normalização nos testes | Duas implementações de "o que é N4" divergiriam, e a divergente seria a que roda no gate |
| CD6 | Dados congelados | `oracle/baseline/*.json` continuam sendo a única fonte de expectativa medida | Já versionados, já reproduzíveis, já auditados; inventar constantes novas perderia a proveniência |
| CD7 | `Skipped` | É **falha** nos testes de replay | O motivo de existir `Skipped` era o stub de comando não migrado; não há mais stub, então pular é esconder regressão |
| CD8 | Casos de divergência deliberada | Não são pulados: viram teste explícito do comportamento Mustel | Pular é o oposto de garantir; o comportamento decidido precisa de teste próprio |
| CD9 | Gate de paridade | `cargo test --workspace`; `oracle-compare` vira diagnóstico opcional | O gate tem de ser o comando que todo mundo roda; depender de passo manual é depender de disciplina |
| CD10 | Alcance de "decisão de interface" | **Só superfície de CLI** (escada N1–N3). O que não se observa por invocação de CLI fica fora | Resposta do dono do produto (2026-10-21): era a única interface que existia antes |
| CD11 | Triagem obrigatória | Três classes: **A** observável por invocação, **B** observável por unidade/integração via CLI, **C** não observável por teste. Classe C só é aceita com verificação alternativa ou razão escrita | Sem triagem obrigatória, "cobrir cada decisão" degenera em testar o untestável e o manifesto vira ficção |
| CD12 | Prova de não-vacuidade | Falsificação em 4 decisões de comportamento (mutação real, revertida) + caso inline violador para as estáticas | Um marcador prova existência, não força de asserção; só a falsificação prova que o teste morde |
| CD13 | O que é contrato | **Somente CLI**: árvore de comandos, flags, defaults, exclusividade, validação, exit codes, mensagens, artefatos e fluxos interativos | Q1 (2026-10-21). GUI e distribuição ficam de fora — não havia equivalente no legado |
| CD14 | Remoção do legado | Aprovada pelo dono (Q2), **não** imediata: o `legacy/` só sai quando a cobertura de superfície estiver provada | "Nos assegurando de termos todos os casos de teste": a remoção é o *fim* do plano, não o começo |
| CD15 | Guia de desenvolvimento | `AGENTS.md` único; `CLAUDE.md` absorvido e removido | Q3 (2026-10-21): "hoje não existe mais razão para usar o CLAUDE.md" |
| CD16 | Forma da documentação | `docs/` recriado em **Diátaxis**: quatro pastas — `tutorials/`, `how-to/`, `reference/`, `explanation/` — em markdown e em pt-BR, escritas para o Mustel | Resposta do dono (2026-10-21): documentação nova no padrão Diátaxis, refeita a partir do programa Rust. Diátaxis separa o que o usuário *faz* (tutorial/how-to) do que ele *sabe* (referência/explicação) — misturar os quatro é a origem da maior parte dos docs ruins |
| CD17 | Resíduos do nome antigo | Skills `furlab-command-manager` e configs `.vscode` de debug do legado **removidas**; `assets/FurLab_icon.png` **mantido e renomeado** para `assets/Mustel_icon.png` | Resposta do dono (2026-10-21): "podemos limpar, deixe o ícone". O ícone é o do produto; o nome do arquivo e o `alt` dos READMEs ("FurLab Logo") não podem sobreviver ao rename |
| CD18 | Git nesta rodada | **Sem tag e sem commit do agente**: nenhuma tarefa cria tag, nenhuma commita. As remoções usam `rm`/`mv` do sistema de arquivos (não `git rm`/`git mv`, que **stageiam**) e ficam **não-staged**, para o dono revisar e confirmar | Q3 (2026-10-21): "não vamos criar tag no git, pois o trabalho será migrado para outro repositório depois" + "não auto-commit nada, deixe as mudanças no git para eu verificar". Staging silencioso também é uma forma de decidir pelo dono |

---

## 4. Arquitetura / Design

### 4.1 As camadas (todas dentro do CLI)

```
+-----------------------------------------------------------------------------+
|  camada 1 — replay duro (N1/N2)                                             |
|  oracle/baseline/*.json  -->  compare_case()  -->  45 testes nomeados        |
|  exit code + artefatos = contrato; texto = N4 (nao falha aqui)               |
+-----------------------------------------------------------------------------+
|  camada 2 — texto (N4 exibido, nao aceito em silencio)                      |
|  insta snapshots  -->  erros de uso, mensagens de sucesso, orientacoes        |
+-----------------------------------------------------------------------------+
|  camada 3 — decisoes de CLI que nao sao uma invocacao simples                |
|  exit codes, cofre (config sem senha), sem-TTY, flags novas, prompts N3      |
+-----------------------------------------------------------------------------+
                              ^
                              |  manifesto + marcadores
+-----------------------------------------------------------------------------+
|  docs/contrato-de-interface.md  <-->  CONTRACT: IF-NNN em crates/**/*.rs     |
|  contract_manifest.rs falha quando um lado cita o que o outro nao tem        |
+-----------------------------------------------------------------------------+
```

### 4.2 Ordem de execução (o porquê de a remoção ser a última fase)

```
manifesto --> mecanismo --> testes --> COBERTURA PROVADA --> tag --> remocao
                                          ^
                                          |
                       sem esta prova, remover o legado
                       apaga a unica referencia do contrato
```

### 4.3 Esquema do manifesto

Tabela markdown, uma linha por decisão, **sete colunas fixas** (o parser recusa qualquer outra coisa):

| Coluna | Conteúdo | Exemplo |
|--------|----------|---------|
| `ID` | `IF-NNN`, único, estável, nunca reutilizado | `IF-042` |
| `Origem` | `D9`, `DIV18`, `T16`, `CLI query run` | `D9` |
| `Classe` | `A`, `B` ou `C` | `A` |
| `Decisão` | frase assertiva: o que precisa ser **verdade** | `mustel sem subcomando e sem TTY imprime a ajuda e sai 0` |
| `Onde se observa` | a invocação ou o artefato que o teste olha | `stdout do binário sem argumentos` |
| `Teste` | `<arquivo>::<fn>` separado por `;`, ou `—` | `crates/mustel/tests/cli_tree.rs::root_help_lists...` |
| `Status` | `coberta`, `pendente` ou `nao-coberta: <razão>` | `coberta` |

Regras de conteúdo:

- `Decisão` descreve **comportamento observável**, não intenção ("falha com exit 40", não "deve validar").
- `Onde se observa` é **sempre** uma invocação de CLI ou um artefato que ela produz. Linha que precise
  de janela, licença ou release não pertence a este manifesto (CD13).
- Uma linha nunca repete a narrativa de `known-divergences.md`: cita `DIV-n` e fixa **o que o teste
  prende**.
- Linha `pendente` só existe durante a execução; o plano só fecha com zero `pendente`.

### 4.4 Gramática do marcador

Em qualquer arquivo `.rs` sob `crates/`:

```rust
/// CONTRACT: IF-042
#[test]
fn root_without_a_tty_prints_the_help_and_exits_zero() { /* ... */ }
```

O meta-teste varre `crates/**/*.rs`, extrai `CONTRACT: (IF-\d{3})` e compara com os IDs do manifesto
nas duas direções. Marcador em teste não é decoração: é o que transforma "existe teste" em "existe
teste **e** ele se declara responsável por esta decisão".

### 4.5 Fluxo do meta-teste

```
contract_manifest.rs
  |
  +-- le docs/contrato-de-interface.md
  |     - toda linha de dados comeca com | IF-
  |     - exatamente 7 colunas
  |     - IDs unicos
  |
  +-- para cada linha: Teste != "—"
  |     - arquivo existe
  |     - "fn <nome>" aparece nesse arquivo   -> senao FALHA "linha promete teste inexistente"
  |
  +-- varre crates/**/*.rs por CONTRACT: IF-
        - todo marcador tem linha no manifesto -> senao FALHA "teste orfao"
        - toda linha coberta tem >= 1 marcador -> senao FALHA "decisao sem teste"
```

### 4.6 Prova de cobertura de superfície (o portão da remoção)

```
contract_surface.rs
  |
  +-- para cada comando de SCOPE_COMMANDS (11):
  |     - existe >= 1 caso em oracle/baseline/ com esse caminho de comando
  |
  +-- para cada flag longa de `--help` de cada comando:
        - aparece em >= 1 argv de caso do baseline
          OU esta em EXCEPTIONS com motivo escrito
```

Sem essa prova, "temos todos os casos de teste" é opinião. `EXCEPTIONS` é a lista honesta do que não
dá para exercitar no harness (ex.: `--pooling`/`--max-pool-size`, sem efeito até o T34; flags que
exigem um Postgres vivo).

### 4.7 Arquivos novos

- `docs/contrato-de-interface.md` — o manifesto (CD1) e as convenções de ID/marcador.
- `crates/mustel/tests/support/mod.rs` — sandbox por caso (tempdir, `MUSTEL_CONFIG_DIR`,
  `MUSTEL_PGPASS_FILE`), aplicação de `config_seed`/`pgpass_seed`, execução do binário e leitura do
  baseline. Compartilhado por `mod support;` nos arquivos de teste de contrato.
- `crates/mustel/tests/contract_replay.rs` — camada 1 (45 casos + 2 divergentes).
- `crates/mustel/tests/contract_text.rs` — camada 2 (snapshots `insta`).
- `crates/mustel/tests/contract_manifest.rs` — o meta-teste.
- `crates/mustel/tests/contract_surface.rs` — a prova de cobertura (§4.6).
- `crates/mustel-core/tests/contract_decisions.rs` — decisões de classe B (exit codes, cofre) e o
  cheque de links da documentação.
- `docs/tutorials/primeiros-passos.md`, `docs/how-to/**`, `docs/reference/**`, `docs/explanation/**` e
  `docs/README.md` — a documentação Diátaxis (CD16), escrita no T23 e indexada no T24.

### 4.8 Mudanças em arquivos existentes

- `AGENTS.md` — absorve o `CLAUDE.md` inteiro (adaptado a Rust) e vira o guia único.
- `crates/mustel/Cargo.toml` — `mustel-testkit` como **dev-dependency** (sem ciclo: `mustel` →
  `mustel-testkit` → `mustel-core`; ninguém depende de `mustel`).
- `crates/mustel/tests/cli_tree.rs`, `pty_prompts.rs`, `settings_*.rs` — recebem marcadores
  `CONTRACT:` onde já provam uma decisão; nenhum teste é reescrito de propósito.
- `oracle/README.md` — o gate deixa de ser `oracle-compare` e o legado deixa de ser necessário.
- `assets/FurLab_icon.png` → `assets/Mustel_icon.png` (`mv`, sem staging — CD18) e os dois READMEs
  apontando para o nome novo, com `alt="Mustel Logo"`.
- `docs/**` — os 11 arquivos da era .NET saem (depois de minerados no T22) e dão lugar aos quatro
  quadrantes.
- `oracle/known-divergences.md` — lacuna do item 10 e reescrita da lista branca (sem `legacy/`).
- `MustelPlans/migracao-rust.md` §6 — **uma linha** apontando para este plano; o texto existente fica
  como está (é histórico, e já houve substituição acidental de linha nesse arquivo).
- `crates/mustel-testkit/src/{lib,paths,cases,config_sandbox}.rs` — perdem tudo que só existia para
  falar com o .NET.

---

## 5. Tarefas

Cada tarefa tem **ID único** para referência cruzada. O agente marca `- [ ]` -> `- [x]` ao concluir e
atualiza a seção 9 (Log).

- **Depende de:** IDs que devem estar concluídos antes
- **Paralelizável com:** IDs que podem rodar simultaneamente
- **Concluído quando:** critério verificável, não subjetivo
- **Arquivos:** write-scope

> **Git (D14 + CD18).** O dono aprovou a remoção do código legado, mas **não** autorizou commit nem tag:
> o agente deixa tudo na árvore de trabalho, **não-staged**, e o dono revisa e commita. Consequências
> práticas: (a) usar `rm`/`mv`, **nunca** `git rm`/`git mv`, que stageiam; (b) o commit da remoção deve
> ser **isolado** e feito pelo dono, porque é o commit *anterior* a ele que preserva o código do
> `legacy/` no histórico (é disso que a segunda onda depende, já que não haverá tag); (c) nenhum
> `git commit`, `git tag`, `git add` ou push.

### Fase 0 — Guia único e manifesto

- [x] **T1** — Absorver o `CLAUDE.md` no `AGENTS.md` e remover o `CLAUDE.md` (CD15). O `AGENTS.md`
  atual está truncado (6 linhas, termina no meio da palavra "pro") — o conteúdo dele será substituído
  pelo do `CLAUDE.md`, adaptado: seções do legado marcadas como "transitórias até o T18", convenções e
  exit codes preservados, e uma seção nova sobre o contrato testável apontando este plano.
  - Depende de: —
  - Paralelizável com: T2
  - Concluído quando: `AGENTS.md` contém as seções Idioma, Estado do repositório, Stack, Estrutura,
    Comandos, Convenções, Paridade, Exit codes e OpenSpec; `git ls-files CLAUDE.md` vazio;
    `grep -rn "CLAUDE\.md" --exclude-dir={.git,legacy,openspec,MustelPlans,target}` retorna vazio.
  - Arquivos: `AGENTS.md`, `CLAUDE.md` (remoção), `oracle/known-divergences.md` (lista branca)

- [x] **T2** — Criar `docs/contrato-de-interface.md` com as convenções (CD2, CD3, CD13) e uma linha por
  decisão de **superfície de CLI**: árvore dos 11 comandos, flags/defaults/exclusividade, validação,
  exit codes, mensagens, artefatos (config, `pgpass.conf`, CSV consolidado, `_log.csv`, `_erros.csv`) e
  prompts. Rows de D1–D27 e DIV só entram quando o observável for uma invocação de CLI.
  - Depende de: —
  - Paralelizável com: T1, T4
  - Concluído quando: o arquivo existe; `grep -c '^| IF-'` bate com o total declarado no rodapé;
    nenhum `ID` repetido (`grep -o 'IF-[0-9]*' | sort | uniq -d` vazio); toda linha tem `Onde se observa`
    preenchido com uma invocação de CLI ou artefato.
  - Arquivos: `docs/contrato-de-interface.md`

- [x] **T3** — Triagem obrigatória: classificar cada linha em A/B/C e preencher `Status` — `coberta`
  citando `<arquivo>::<fn>`, `pendente` quando falta, e `nao-coberta: <razão>` **somente** para classe C
  com verificação alternativa descrita.
  - Depende de: T2
  - Paralelizável com: —
  - Concluído quando: `grep '^| IF-' docs/contrato-de-interface.md | grep -c '| — |'` = 0; toda linha
    de classe C tem texto em `nao-coberta:`; a contagem por status no rodapé soma o total.
  - Arquivos: `docs/contrato-de-interface.md`

### Fase 1 — Mecanismo (o que torna a cobertura verificável)

- [x] **T4** — Ligar `mustel-testkit` ao `mustel` como dev-dependency e escrever
  `crates/mustel/tests/support/mod.rs`: sandbox em `tempfile` por caso, apontando `MUSTEL_CONFIG_DIR`
  e `MUSTEL_PGPASS_FILE` para dentro dele; aplicação de `ConfigSeed`/`PgPassSeed`; execução de
  `env!("CARGO_BIN_EXE_mustel")` devolvendo `RunOutput`; carga de `oracle/baseline/<slug>.json`.
  - Depende de: —
  - Paralelizável com: T1, T2, T9–T13, T14
  - Concluído quando: `cargo test -p mustel --test contract_replay -- --list` compila e o teste
    `support_isolates_the_environment` passa, provando que os dois caminhos de ambiente apontam para o
    tempdir e que nenhum arquivo do usuário é tocado; `cargo test --workspace` continua verde.
  - Arquivos: `crates/mustel/Cargo.toml`, `crates/mustel/tests/support/mod.rs`,
    `crates/mustel/tests/contract_replay.rs` (só o teste de sanidade; o replay entra no T5)

- [x] **T5** — Camada 1: `crates/mustel/tests/contract_replay.rs` com **um `#[test]` por captura**
  (45 casos) gerado por macro a partir de uma lista estática de slugs. Cada teste monta o sandbox,
  semeia o estado registrado, roda o `mustel` com `argv` da captura e chama `compare_case`; falha se
  `worst()` for `Fail` ou `Skipped`. A mensagem de falha imprime todos os `Check` (nome, nível, detalhe).
  - Depende de: T4
  - Paralelizável com: T6, T8
  - Concluído quando: `cargo test -p mustel --test contract_replay -- --list` lista 45 nomes
    `replay_*`; a suíte passa; nenhum teste escreve fora do tempdir.
  - Arquivos: `crates/mustel/tests/contract_replay.rs`

- [x] **T6** — Camada 1b: os dois casos pulados hoje (`error-database-restore-file-missing`,
  `error-db-servers-test-offline`) viram teste **explícito** do comportamento Mustel (CD8): exit code
  esperado e mensagem, citando no doc-comment a divergência que autoriza a diferença (itens 11 e 13 de
  `known-divergences.md`).
  - Depende de: T4
  - Paralelizável com: T5, T8
  - Concluído quando: os dois testes existem com nome próprio, passam, e cada um cita no doc-comment a
    divergência que justifica não comparar com o oráculo.
  - Arquivos: `crates/mustel/tests/contract_replay.rs`

- [x] **T7** — `crates/mustel/tests/contract_manifest.rs`: parser estrito do manifesto (7 colunas, IDs
  únicos) + as duas direções do §4.5 (linha → `fn` existe no arquivo citado; marcador → linha existe).
  Falha com mensagem que nomeia o ID e o problema.
  - Depende de: T2, T4
  - Paralelizável com: —
  - Concluído quando: o teste passa no estado final; **e** as três provas de mutação foram executadas
    e registradas no log: (a) apagar um marcador → falha "decisao sem teste"; (b) acrescentar linha com
    `Teste` inventado → falha "teste inexistente"; (c) duplicar um ID → falha "id duplicado".
  - Arquivos: `crates/mustel/tests/contract_manifest.rs`

### Fase 2 — Texto onde o texto é o contrato

- [x] **T8** — Camada 2: `crates/mustel/tests/contract_text.rs` com snapshots `insta` dos textos que
  **são** entregáveis: erro de uso em `stdout` com exit 1, `Server '<n>' added successfully.` +
  orientação de `set-password`, orientação de config vazia do `ls`, `Entry added: host:port:db:user`,
  mensagem de binário ausente do `backup`/`restore`, e os avisos do `--all`. Normalizar quebras de
  linha (`\r\n` → `\n`) antes do snapshot, senão o snapshot só passa no Windows.
  - Depende de: T4
  - Paralelizável com: T5, T6
  - Concluído quando: todos os snapshots estão versionados em `crates/mustel/tests/snapshots/`;
    `cargo test -p mustel --test contract_text` passa **sem** `INSTA_UPDATE` (o `insta` falha sozinho
    quando falta snapshot e grava `.snap.new` quando diverge — não é preciso o CLI `cargo insta`);
    cada teste tem marcador.
  - Arquivos: `crates/mustel/tests/contract_text.rs`, `crates/mustel/tests/snapshots/*`

### Fase 3 — Decisões de CLI que não são uma invocação simples

> Todas independentes entre si. Cada uma preenche linhas do manifesto e acrescenta o marcador.

- [x] **T9** — Exit codes: teste de tabela sobre o mapeamento completo (0, 1, 2, 10, 11, 20, 21, 22,
  30, 40, 41, 50, 130) e sobre a regra "falha com SQLSTATE → 10; sem SQLSTATE → 11". Cobrir também o
  quirk do legado: binário ausente é **3** no `backup` e **1** no `restore`.
  - Depende de: —
  - Paralelizável com: T10–T13, T14
  - Concluído quando: os testes existem, passam, e a tabela de exit codes do `AGENTS.md` tem
    correspondência 1:1 com os casos testados.
  - Arquivos: `crates/mustel-core/tests/contract_decisions.rs`, `crates/mustel-core/src/error.rs`

- [x] **T10** — Cofre (D6), pelo que o CLI deixa ver: `settings db-servers set-password` grava **só** no
  cofre (o `mustel.jsonc` resultante não tem campo de senha) e a senha recuperada é usada na conexão
  sem aparecer em mensagem ou arquivo. O teste contra o cofre real continua atrás de
  `MUSTEL_KEYRING_TESTS=1` e **limpa** o que criou.
  - Depende de: —
  - Paralelizável com: T9, T11–T13
  - Concluído quando: os testes passam com e sem `MUSTEL_KEYRING_TESTS` (sem a variável, o do cofre
    real se declara pulado); o cofre do Windows fica limpo depois da execução.
  - Arquivos: `crates/mustel-core/tests/contract_decisions.rs`, `crates/mustel/tests/settings_db_servers_set_password.rs`

- [x] **T11** — Binário único, lado CLI (D9 + divergência 18): `mustel` **sem subcomando e sem TTY**
  imprime a ajuda e sai **0** (paridade); com TTY abre a GUI — essa metade fica `nao-coberta` (abrir
  janela não é verificável em teste). O predicado de decisão vira função testável.
  - Depende de: —
  - Paralelizável com: T9, T10, T12, T13
  - Concluído quando: existe teste do predicado (com e sem TTY) e o caminho sem TTY confirma exit 0 +
    ajuda em `stdout`; a metade da GUI está no manifesto como `nao-coberta:` com razão.
  - Arquivos: `crates/mustel/src/commands/mod.rs`, `crates/mustel/tests/cli_tree.rs`

- [x] **T12** — `database restore --create-if-not-exists` (D25): superfície (a flag aparece na ajuda) e
  comportamento — sem a flag, banco inexistente falha (paridade); com a flag, o banco é criado pelo
  driver. O caso real vai para a suíte gated de Postgres.
  - Depende de: —
  - Paralelizável com: T9–T11, T13
  - Concluído quando: o teste de superfície roda em todo `cargo test` e o de comportamento passa com
    `MUSTEL_PG_TEST_URL` (sem a variável, declara-se pulado).
  - Arquivos: `crates/mustel-core/tests/postgres_integration.rs`, `crates/mustel/tests/cli_tree.rs`

- [x] **T13** — Divergências com observável de CLI: percorrer as **17** linhas (rotuladas de 1 a 18,
  sem o item 10) e, para cada uma, apontar no manifesto o teste que a prende (muitas já têm: `DIV3`
  JSONC com `//` em string, `DIV12` formatos en-US, `DIV14` mensagem do driver, `DIV1` config sem
  `encryptedPassword`, `DIV5` forma posicional do `pgpass add`). Criar teste onde não houver — pelo menos
  `DIV8` (modo `0600` do `pgpass.conf` no Unix, sob `#[cfg(unix)]`) e `DIV15` (linhas gravadas antes da
  falha).
  - Depende de: T2, T5
  - Paralelizável com: T9–T12
  - Concluído quando: nenhuma linha `DIV*` do manifesto está `pendente`; as que exigem console (`DIV16`,
    `DIV17`) e as de produto (`DIV6`) têm `nao-coberta:` com razão.
  - Arquivos: `crates/mustel-core/tests/contract_decisions.rs`, `crates/mustel/tests/contract_replay.rs`

### Fase 4 — Prompts (N3)

- [x] **T14** — `pty_prompts.rs` passa a cobrir o contrato dos fluxos interativos: texto do prompt de
  senha (o mesmo do legado), `--no-confirm` dispensa a confirmação destrutiva, a confirmação destrutiva
  tem **sim** como default, e a multi-seleção de servidores entra com todos pré-selecionados.
  - Depende de: —
  - Paralelizável com: T9–T13
  - Concluído quando: os quatro comportamentos têm teste com PTY, verde em
    `cargo test -p mustel --test pty_prompts`, cada um com marcador.
  - Arquivos: `crates/mustel/tests/pty_prompts.rs`

### Fase 5 — Prova de cobertura (o portão da remoção)

- [x] **T15** — `crates/mustel/tests/contract_surface.rs` (§4.6): para cada um dos 11 comandos do escopo,
  existe caso no baseline; para cada flag longa do `--help`, ela aparece em algum `argv` do baseline ou
  está em `EXCEPTIONS` com motivo escrito.
  - Depende de: T5, T12
  - Paralelizável com: T16
  - Concluído quando: o teste passa e a lista `EXCEPTIONS` é impressa no log da execução; remover um
    caso do baseline (numa cópia temporária) faz o teste falhar.
  - Arquivos: `crates/mustel/tests/contract_surface.rs`

- [x] **T16** — Registrar em `oracle/known-divergences.md` que **não existe item 10** na numeração (foi
  omitido, não removido) e reescrever a **lista branca de resíduos** para o mundo sem `legacy/`.
  **Não renumerar** (quebraria as referências já escritas).
  - Depende de: T1
  - Paralelizável com: T15
  - Concluído quando: a nota e a lista nova existem em linhas acrescentadas; as linhas vizinhas estão
    intactas (`sed -n` conferido logo depois da edição).
  - Arquivos: `oracle/known-divergences.md`

### Fase 6 — Remover o legado

- [x] **T17** — Checagem de ponto de não-retorno, **sem tag** (CD14 + CD18): confirmar que a árvore
  está limpa (só os arquivos deste plano como mudança), que a cobertura do T15 passou, e registrar no
  log que a remoção deve virar um **commit isolado do dono** — é o pai desse commit que guarda o
  `legacy/` no histórico, já que não haverá tag.
  - Depende de: T15
  - Paralelizável com: T18 — **não**: é o portão antes da remoção
  - Concluído quando: `git --no-optional-locks status --short` mostra apenas mudanças deste plano;
    `git tag -l legacy-final` **vazio** (nenhuma tag criada); o log registra a recomendação do commit
    isolado; `git log --oneline -1` igual ao do início da execução (nenhum commit do agente).
  - Arquivos: nenhum (verificação)

- [x] **T18** — Remover o diretório legado **sem stagiar** (CD18): `rm -rf legacy/` e apagar as saídas
  de build não rastreadas (`legacy/**/bin`, `legacy/**/obj`, ~275 MB em disco), mais `oracle/bin/`,
  `oracle/workdir/` e `oracle/reports/`. Atualizar `.gitignore`.
  - Depende de: T17
  - Paralelizável com: —
  - Concluído quando: `test ! -d legacy`; `git ls-files --deleted legacy | wc -l` = **130** (as
    remoções aparecem como não-staged); `git diff --cached --name-only | wc -l` = **0** (nada staged);
    `cargo build --workspace` e `cargo test --workspace` verdes; `grep -rn "legacy/"
    --exclude-dir={.git,target,MustelPlans,openspec}` não aponta para caminho inexistente.
  - Arquivos: `legacy/**` (remoção), `.gitignore`

- [x] **T19** — Aposentar o que só existia para falar com o .NET: `oracle-capture` e `oracle-perf`
  (e `cases.rs`, `config_sandbox.rs`, as funções de caminho do legado em `paths.rs`). `oracle-compare`
  fica como diagnóstico. **Consequência aceita e registrada**: o baseline não pode mais ser re-gravado;
  ele é o contrato congelado.
  - Depende de: T18
  - Paralelizável com: T20
  - Concluído quando: `cargo build --workspace` verde; `cargo run -p mustel-testkit --bin oracle-compare`
    roda contra o baseline; nenhuma referência a `dotnet` ou `FurLab` sobra no código executável
    (`grep -rn "dotnet\|FurLab.CLI.exe" crates --include=*.rs` só encontra doc-comment de rastreabilidade).
  - Arquivos: `crates/mustel-testkit/src/**`, `crates/mustel-testkit/Cargo.toml`

- [x] **T20** — Atualizar a documentação viva para o mundo sem legado: `AGENTS.md` (tirar as seções
  transitórias, registrar que o baseline é congelado), `README.md`/`README.pt-BR.md` (o aviso de
  "FurLab congelado" sai), `oracle/README.md` (o baseline vira contrato congelado; sem captura).
  - Depende de: T19
  - Paralelizável com: —
  - Concluído quando: os quatro arquivos não descrevem mais o `legacy/` como existente; o `README`
    explica como rodar a suíte de contrato.
  - Arquivos: `AGENTS.md`, `README.md`, `README.pt-BR.md`, `oracle/README.md`

- [x] **T21** — Limpar os resíduos do nome antigo, **já autorizado** (CD17), **sem staging** (CD18):
  `rm -rf .claude/skills/furlab-command-manager .opencode/skills/furlab-command-manager`,
  `rm .vscode/launch.json .vscode/tasks.json` (eram debug do .NET; `.vscode/settings.json` fica) e
  `mv assets/FurLab_icon.png assets/Mustel_icon.png`; atualizar as duas referências nos READMEs,
  **inclusive o `alt="FurLab Logo"`**.
  - Depende de: T20
  - Paralelizável com: T22
  - Concluído quando: `git ls-files | grep -i furlab` só devolve os manifests do Cargo (`repository`,
    D18) e as linhas da lista branca; `git diff --cached --name-only` vazio (nada staged);
    `grep -rn "FurLab_icon\|FurLab Logo" --exclude-dir={.git,target,MustelPlans}` vazio; os dois
    READMEs renderizam o ícone novo (arquivo existe em `assets/Mustel_icon.png`); nenhum commit criado.
  - Arquivos: `.claude/skills/furlab-command-manager/**`, `.opencode/skills/furlab-command-manager/**`,
    `.vscode/launch.json`, `.vscode/tasks.json`, `assets/`, `README.md`, `README.pt-BR.md`

### Fase 7 — Documentação em Diátaxis

> Ordem dentro da fase: primeiro **salvar o conteúdo** dos docs antigos, depois escrever os novos,
> depois **apagar** os antigos. Apagar antes de ler é a única forma de perder uma decisão que o plano
> não registrou.

- [x] **T22** — Mineração e inventário: ler os **11** documentos rastreados em `docs/` (`MULTI_SERVER.md`,
  `QUERY_COMMAND.md`, `rust-postgres-spike.md`, `en/{ARCHITECTURE,FEATURE_SPECIFICATION,GUI_REFACTORING_PLAN}.md`,
  `pt-BR/{ARCHITECTURE,FEATURE_SPECIFICATION,GUI_REFACTORING_PLAN,SESSION_SUMMARY_2025-03-17,credenciais}.md`)
  e produzir uma **tabela de salvamento**: cada arquivo → o que é aproveitável → em qual quadrante e
  qual arquivo novo ele vira → ou `descartar: <razão>`. Registrar em `docs/README.md` (o índice).
  - Depende de: T21
  - Paralelizável com: T23
  - Concluído quando: os 11 arquivos aparecem na tabela; cada um tem destino ou `descartar:` com
    razão; nenhum quadrante fica sem pelo menos um arquivo de destino.
  - Arquivos: `docs/README.md`

- [x] **T23** — Escrever os quatro quadrantes em `docs/`, em pt-BR, com o material do T22 e a verdade
  **verdade** do código (não a do `openspec/`, que descreve o .NET):
  `tutorials/primeiros-passos.md` (lição em **duas metades**: a primeira roda sem servidor — instalar,
  `--help`, `settings db-servers add`, `ls`, `pgpass add`; a segunda sobe o container pinado
  `postgres:18.6` na 55432 e faz `query run`), `how-to/` (salvar senha no cofre, restaurar um dump,
  exportar 100k linhas, rodar contra vários bancos, usar `--no-confirm` em automação),
  `reference/` (árvore de comandos, flags, exit codes, formatos de artefato, variáveis de ambiente) e
  `explanation/` (por que keyring e não DPAPI, por que o CSV é byte a byte como o Npgsql, a escada de
  fidelidade N1–N4, a decisão do binário único).
  - Depende de: T22
  - Paralelizável com: —
  - Concluído quando: as quatro pastas existem com os arquivos acima; a referência dos comandos bate
    com `mustel <cmd> --help` (conferido comando a comando); **o tutorial inteiro foi executado do
    começo ao fim** contra o `mustel-tutorial` (18.6) e um `mustel` buildado; há um passo explícito de
    `pg_dump --version` antes de `backup`/`restore`, com o aviso de que a major precisa casar; nenhuma
    página menciona DPAPI, `furlab.jsonc` ou `encryptedPassword` como comportamento atual; cada página
    de `explanation/` cita o ID de decisão (`D`/`DIV`/`IF`) que a originou.
  - Arquivos: `docs/tutorials/**`, `docs/how-to/**`, `docs/reference/**`, `docs/explanation/**`

- [x] **T24** — Fechar o índice e apagar os antigos: `docs/README.md` como porta de entrada (a bússola
  do Diátaxis: o que o usuário faz × o que ele sabe), links a partir dos dois READMEs da raiz, e
  `rm` dos 11 arquivos antigos (sem staging — CD18).
  - Depende de: T23
  - Paralelizável com: T25
  - Concluído quando: `git ls-files docs` mostra só a estrutura nova; `cargo test -p mustel-core --test
    contract_decisions` inclui `documentation_links_resolve` (todo link relativo de `docs/**/*.md` e
    dos READMEs resolve para um arquivo existente) e passa; `grep -rn "legacy/" docs` vazio.
  - Arquivos: `docs/**`, `README.md`, `README.pt-BR.md`, `crates/mustel-core/tests/contract_decisions.rs`

### Fase 8 — Verificação e fechamento

- [x] **T25** — Prova de falsificação (CD12) em **4 decisões de comportamento** — exit code de uso (1
  em `stdout`), CSV com BOM+CRLF, modo `0600` do `pgpass.conf`, `--create-if-not-exists`: inverter o
  comportamento **temporariamente**, rodar o teste correspondente, confirmar que **falha**, reverter e
  confirmar verde.
  - Depende de: T5, T9, T12, T13
  - Paralelizável com: T24
  - Concluído quando: as 4 mutações estão registradas na seção 9 com o nome do teste que falhou; a
    árvore volta limpa (`git --no-optional-locks status --short` só mostra os arquivos do plano).
  - Arquivos: nenhum permanente (mutações revertidas)

- [x] **T26** — Fechamento: rodar a Estratégia de Verificação inteira, publicar as contagens (testes de
  contrato, linhas do manifesto por status, arquivos legados removidos, páginas de doc escritas) e
  marcar o plano como `concluído`. **Inclui a limpeza dos containers de teste** (Q7):
  `docker rm -f mustel-tutorial mustel-t19-pg`.
  - Depende de: todas
  - Paralelizável com: —
  - Concluído quando: todos os comandos da seção 6 passam; o manifesto tem **zero** `pendente`; a
    seção 9 registra o número de testes antes (330) e depois, e o total de arquivos removidos;
    `docker ps -a` não mostra `mustel-tutorial` nem `mustel-t19-pg`; os containers que já existiam antes
    do plano (`postgres-ptbr`) **continuam intactos**.
  - Arquivos: `MustelPlans/contrato-de-interface-testavel.md`

---

## 6. Estratégia de Verificação

- **Formatação:** `cargo fmt --all -- --check` — sem diffs
- **Lint:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero warnings
- **Testes (o gate):** `cargo test --workspace` — verde. O .NET **não** é necessário para a suíte (a
  prova é estrutural, no item abaixo)
- **Nada commitado nem staged (CD18):** `git log --oneline -1` igual ao do início da execução e
  `git diff --cached --name-only` vazio
- **Documentação:** `cargo test -p mustel-core --test contract_decisions documentation_links_resolve`
  — verde (todo link relativo de `docs/**` e dos READMEs resolve)
- **Quadrantes:** `ls docs` mostra `tutorials how-to reference explanation README.md` e nada mais
- **Cofre real:** `MUSTEL_KEYRING_TESTS=1 cargo test --workspace` — verde, e o cofre fica limpo
- **Postgres real:** `MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test
  -p mustel-core --test postgres_integration` — verde (os casos gated se declaram pulados sem a variável)
- **Cobertura do manifesto:** `cargo test -p mustel --test contract_manifest` — verde; e `coberta` +
  `nao-coberta` = total de linhas
- **Cobertura de superfície:** `cargo test -p mustel --test contract_surface` — verde, com a lista de
  exceções impressa
- **Replay completo:** `cargo test -p mustel --test contract_replay -- --list | grep -c '^replay_'` =
  **45** mais os 2 testes de divergência
- **Legado ausente:** `test ! -d legacy` e `git ls-files --deleted legacy | wc -l` = **130** (as
  remoções ficam não-staged até o dono commitar — CD18). `git tag -l legacy-final` vazio (nenhuma tag
  criada).
- **Sem referência ao .NET no código:** `grep -rn "dotnet\|FurLab.CLI.exe\|oracle/bin" crates
  --include=*.rs` só pode retornar doc-comments de rastreabilidade
- **Resíduo do nome:** `grep -ri "furlab" --exclude-dir={.git,target,MustelPlans,openspec,docs}` só
  retorna o que a lista branca de `oracle/known-divergences.md` justifica
- **Diagnóstico (não é gate):** `cargo run -p mustel-testkit --bin oracle-compare -- --report` — os
  avisos N4 continuam visíveis caso a caso
- **Diagnostics do editor:** rodar rust-analyzer/clippy nos arquivos alterados

> O plano só pode ser marcado como `concluído` após **todas** as verificações acima passarem.
> Se algo falhar, registrar na seção 9 e corrigir antes de devolver ao usuário.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|-------|------|---------|-----------|
| Remover o legado antes de a cobertura estar provada apaga a referência do contrato | média | **alto** | CD14: T15 (prova de superfície) e T17 (tag) rodam **antes** do T18 (remoção); a ordem é critério de conclusão |
| A segunda onda (D21) perde o código dos 6 grupos | alta | alto | Sem tag (Q3): a recuperação depende do **histórico**. Exige que a remoção seja um commit isolado, feito pelo dono, cujo pai ainda tem tudo — está escrito no T17 |
| Baseline não pode mais ser re-gravado depois do T19 | alta | médio | Aceito e registrado: o baseline é o contrato congelado. Mudança deliberada nele passa a exigir atualizar o teste, com revisão explícita |
| O manifesto vira teatro: marcador existe, asserção é fraca | média | alto | T25 (mutação real em 4 decisões de comportamento) + caso inline violador nas estáticas, e a regra de que `Decisão` descreve comportamento observável |
| Duplicar as regras de comparação e criar uma segunda verdade | média | alto | CD5: os testes chamam `compare_case`; nenhuma normalização nova fora de `mustel-testkit` |
| Snapshots de texto quebram entre Windows e Linux (CRLF, largura de console) | alta | médio | Normalizar `\r\n` antes de snapshotar; o dependente de plataforma vira asserção explícita |
| `Warn` (N4) aceita deriva de mensagem em silêncio na camada 1 | média | médio | É por isso que existe a camada 2: onde o texto é entregável, ele está em snapshot |
| Replay acrescenta tempo ao `cargo test` (45 processos filhos) | alta | baixo | Medir no T26; se passar de ~10 s, agrupar por `#[test]` de tabela preservando o nome do caso |
| `git rm -r legacy/` apaga trabalho não commitado do usuário | baixa | **alto** | Antes do T18: `git --no-optional-locks status --short legacy/` tem de estar vazio; o log registra a saída. Com o CD18 a remoção é `rm -rf` (não-staged), então o dono ainda pode reverter pelo Git |
| Apagar os docs antigos antes de minerar perde decisões que não estão no plano | média | alto | T22 (tabela de salvamento) é dependência dura do T24 (remoção); a ordem está escrita na fase |
| Alterar `known-divergences.md` ou o plano mestre apaga linha existente | média | alto | Já aconteceu 3 vezes nesta migração: toda edição é âncora + append, conferida com `sed -n` na hora |
| O agente commitar ou stagiar sem permissão | média | **alto** | CD18: `rm`/`mv` em vez de `git rm`/`git mv`; T17 e T26 checam `git log --oneline -1` e `git diff --cached --name-only` |
| O tutorial ensinar `backup` com `pg_dump` de major diferente do servidor e falhar | alta | médio | Q6: versão pinada (`postgres:18.6`) + passo explícito de conferir `pg_dump --version` antes de `backup`/`restore` |
| Diátaxis degenerar em quatro pastas com o mesmo tipo de texto | média | médio | A bússola do CD16 é o critério de revisão: tutorial = ação+estudo, how-to = ação+trabalho, referência = cognição+trabalho, explicação = cognição+estudo |
| Renomear o ícone e esquecer o `alt`/docs antigos | alta | baixo | T21 tem grep específico para `FurLab_icon` **e** `FurLab Logo` |

---

## 8. Questões em Aberto

> **Nenhuma pendência de decisão.** Todas as questões foram respondidas pelo dono do produto em
> 2026-10-21 e estão marcadas abaixo com a resposta, para que o executor não precise reconstruir a
> conversa.

- [x] **Q1 — Idioma da documentação: RESOLVIDA (2026-10-21).** Decisão do dono: **pt-BR**. `docs/` sai
  em português, alinhado à convenção do repositório; `docs/en/` fica para uma rodada futura, com o
  mesmo recorte de quadrantes.
- [x] **Q2 — `openspec/specs/**` (91 arquivos): RESOLVIDA por escopo (2026-10-21).** Ficam como
  **histórico** e **não** alimentam a doc nova: o §2 (Exclui) já determina que as specs não são
  reescritas nesta rodada, e o T23 manda conferir a referência contra `mustel <cmd> --help`, não
  contra a spec. Arquivar `openspec/` de vez é rodada futura, fora deste plano.
- [x] **Q3 — Tag do Git: RESOLVIDA (2026-10-21).** Decisão do dono: **não criar tag**, porque o trabalho
  será migrado para outro repositório. Consequências registradas em CD18 e no T17: a recuperação da
  segunda onda (D21) passa a depender do **histórico do Git** — logo, a remoção do `legacy/` precisa ser
  um commit **isolado**, feito pelo dono, cujo pai ainda contém o código. Nada de `git tag`.
- [x] **Q4 — Granularidade: RESOLVIDA (2026-10-21).** Um **`#[test]` nomeado por decisão**, com tabela
  interna quando a decisão tem vários casos (exit codes, tipos CSV). É o que o pedido do dono implica
  ("um teste unitário para cada decisão"): o nome do teste é o que torna a falha legível em `--list`.
- [x] **Q5 — Os 2 casos hoje pulados: RESOLVIDA (2026-10-21).** Viram teste do comportamento Mustel
  (CD8), autorizados pelos itens 11 e 13 de `known-divergences.md`. O dono já avalizou as 18
  divergências no fechamento do T47 (*"pode fechar o T47 eu concordo"*), então o comportamento a
  congelar está aprovado; o T6 apenas o registra em teste.
- [x] **Q6 — Alvo do tutorial: RESOLVIDA (2026-10-21).** Decisão: **(a)+(b) combinados** — a lição tem
  uma primeira metade que roda **sem servidor** (instalar, `--help`, `settings db-servers add`
  gravando o JSONC, `ls`, `pgpass add`) e uma segunda que sobe um container **próprio**, para não
  depender nem sujar o ambiente do leitor. Versão verificada no Docker Hub e pinada: **`postgres:18.6`**
  (o `latest` flutua; pinnar é o que mantém o texto verdadeiro). Comando do tutorial:
  `docker run -d --name mustel-tutorial -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=mustel_tutorial -p 55432:5432 postgres:18.6`.
  **Achado que vira aviso obrigatório no tutorial:** o `pg_dump`/`pg_restore` precisa ser da **mesma
  major** do servidor — medido nesta máquina: o `pg_dump` 16.6 do PATH aborta com
  `aborting because of server version mismatch`, enquanto o 18.6 de
  `C:\Program Files\PostgreSQL\18\bin` faz o dump. E o locator do Mustel
  ([locator.rs:96](worktree://8fa5b556-adb1-46cb-a046-9cfb7235f917/crates/mustel-core/src/postgres/locator.rs#L96))
  procura **no PATH primeiro** e só depois nas pastas de versão — com os dois instalados o Mustel pega
  o 16.6 e `database backup` falha. O tutorial precisa de um passo explícito "confira
  `pg_dump --version`" antes de qualquer `backup`/`restore`.
- [x] **Q7 — Container do tutorial vs. containers de teste: RESOLVIDA (2026-10-21).** Decisão do dono:
  **remover depois de concluir tudo**. O `mustel-tutorial` (18.6, 55432) fica de pé durante todo o
  trabalho — é ele que valida o tutorial do T23 — e sai no fechamento (T26), junto do `mustel-t19-pg`
  (`Exited (255)` há dias, lixo do T19). Nenhum teste automatizado depende de qualquer um dos dois:
  `MUSTEL_PG_TEST_URL` continua sendo escolha de quem executa.

---

## 9. Log de Execução

Append-only. O agente registra cada avanço relevante (início, conclusão, bloqueio, paralelização,
falhas). Mantém o "Atualizado em" sincronizado.

- `[2026-10-21]` Plano criado em `rascunho`. Levantamento que o fundamenta: **330 testes** em
  `cargo test --workspace`; **47** capturas em `oracle/baseline/`; nenhum caminho de teste executa o
  .NET; `compare_case` já é função pura sobre captura + execução; numeração das divergências salta o
  item **10**.
- `[2026-10-21]` **Revisão 2 — respostas do dono do produto.** (Q1) contrato restrito à **superfície de
  CLI**: fases de GUI e de distribuição saíram; `AGENTS.md` passa a guiar o manifesto. (Q2) o
  **`legacy/` será removido**, condicionado a os testes existirem → fase 5 (prova de cobertura) e tag
  `legacy-final` antes da remoção. Medido: **130 arquivos rastreados** em `legacy/`, **275 MB** em
  disco (build não rastreado), **6 grupos** da segunda onda só existem ali (`claude`, `docker`, `files`,
  `opencode`, `windowsfeatures`, `winget`). (Q3) `CLAUDE.md` absorvido no `AGENTS.md` e removido.
  Tarefas: 22 → **23**.
- `[2026-10-21]` **Revisão 3 — limpeza liberada e documentação Diátaxis.** (T21) o dono autorizou
  limpar os resíduos, **preservando o ícone**, que passa a se chamar `Mustel_icon.png` (CD17) — os
  dois READMEs o referenciam com `alt="FurLab Logo"`, então o rename inclui o texto alternativo.
  (Q2 da revisão 2) `docs/` **não** fica como histórico: será recriado em **Diátaxis** (CD16) a partir
  do produto Rust, com os 11 arquivos antigos minerados antes de sair. Medido: `docs/` tem **11**
  arquivos rastreados, todos da era .NET; `openspec/` tem **91**. Fase 7 nova com T22–T24; verificação
  renumerada para T25–T26. Tarefas: 23 → **26**.
- `[2026-10-21]` **Revisão 4 — tutorial resolvido com ambiente verificado.** Q1 fechada: documentação
  em **pt-BR** (CD16). Q6 fechada em (a)+(b): metade sem servidor + container próprio com
  **`postgres:18.6`** (existência do tag conferida no Docker Hub, não presumida). Criado e verificado
  nesta rodada o container `mustel-tutorial` (18.6, porta 55432, banco `mustel_tutorial`). **Achado
  medido:** `pg_dump` 16.6 (do PATH) aborta contra o servidor 18.6 com *server version mismatch*,
  enquanto o 18.6 de `C:\Program Files\PostgreSQL\18\bin` dumpa normalmente (695 bytes); e o locator
  do Mustel prova o **PATH primeiro** — logo, com as duas versões instaladas, `database backup` pega
  o cliente errado. Isso virou requisito do T23 e risco registrado. Podman ausente na máquina;
  container feito no Docker. Q6 encerrada, Q7 aberta (container do tutorial vs. suíte).
- `[2026-10-21]` **Revisão 5 — Q7 fechada.** Decisão do dono: os containers de teste saem **depois de
  concluir tudo**. `mustel-tutorial` (18.6/55432) fica de pé até o T26 — é o ambiente que valida o
  tutorial — e então é removido junto do `mustel-t19-pg`; o `postgres-ptbr`, que já existia antes do
  plano, não é tocado. Registrado no critério do T26, para a limpeza não virar promessa oral.
- `[2026-10-21]` **Revisão 6 — marcação das questões corrigida.** O dono apontou que só Q7 estava
  marcada. Revisando uma a uma: **Q1** (idioma → pt-BR), **Q2** (openspec fica histórico, já coberto
  pelo §2 Exclui), **Q4** (teste nomeado por decisão), **Q5** (divergências 11 e 13 já avalizadas no
  T47) e **Q6** (tutorial a+b com `postgres:18.6`) estavam **respondidas mas sem checkbox** — todas
  marcadas agora, com a resposta escrita ao lado. **Q3** (push da tag `legacy-final`) segue aberta: é a
  única que depende de autorização do dono. Lição para as próximas edições neste arquivo: decidir não é
  documentar — a marcação é parte da decisão.
- `[2026-10-21]` **Revisão 7 — sem tag, sem commit (Q3 fechada).** Decisão do dono: **nenhuma tag no
  Git** (o trabalho migra para outro repositório) e **nenhum auto-commit** — as mudanças ficam na árvore
  para revisão. Isso virou **CD18** e mudou o mecanismo, não só a letra: `git rm`/`git mv` **stageiam**,
  então T18/T21/T24 passaram a usar `rm`/`mv`, com critério "`git diff --cached --name-only` vazio".
  O T17 deixou de criar tag e virou **portão de não-retorno**: confere árvore limpa, nenhuma tag, nenhum
  commit do agente, e registra que a remoção precisa ser um **commit isolado do dono** — é o pai dele
  que guarda o `legacy/` no histórico, único caminho de recuperação agora que não há tag. Adicionado
  critério global na seção 6 (`git log` inalterado + nada staged) e risco novo ("commitar ou stagiar
  sem permissão"). Durante a edição, a linha de risco "apagar os docs antes de minerar" foi
  sobrescrita por acidente e **restaurada** na mesma rodada — conferida com `sed`.
- `[2026-10-22 00:50]` Execução iniciada pelo mustel-apply.
- `[2026-10-22 01:05]` **T1 concluída** — `AGENTS.md` reescrito absorvendo todo o `CLAUDE.md` (Idioma,
  Estado, Stack, Estrutura, Comandos, Convenções, Paridade/contrato, Exit codes, Legado, Guard Rails,
  OpenSpec) e atualizado para o mundo novo (gate = `cargo test`, `oracle-compare` é diagnóstico,
  guard rail de "sem commit/staging/tag"). `CLAUDE.md` removido com `rm` (não-staged, 0 staged).
  Lista branca de `known-divergences.md` passou a citar só `AGENTS.md`. Verificado: nenhum arquivo
  rastreado fora de `legacy/openspec/MustelPlans` menciona o nome antigo.
- `[2026-10-22 01:20]` **T2 concluída** — `docs/contrato-de-interface.md` criado com **117 linhas**
  `IF-NNN` cobrindo a superfície real dos 11 comandos (lida do `--help` de cada um, não de memória),
  exit codes e as divergências observáveis. `Onde se observa` é sempre invocação de CLI ou artefato.
- `[2026-10-22 01:22]` **T3 concluída** — triagem A/B/C e status: **105 coberta, 9 pendente,
  3 nao-coberta** (pool até T34, prompt por servidor, superfície nova). Sem IDs duplicados e sem
  coluna malformada. Conferência automática: todos os 105 caminhos de teste citados resolvem para um
  `fn` existente no arquivo citado (3 mapeamentos errados foram corrigidos nesta verificação).
- `[2026-10-22 01:40]` **T4 concluída** — `mustel-testkit` virou dev-dependency de `mustel` (sem ciclo)
  e `crates/mustel/tests/support/mod.rs` centraliza sandbox (`tempfile`), seeds de config/pgpass,
  execução do binário e carga do baseline. Teste `support_isolates_the_environment` verde.
- `[2026-10-22 01:45]` **T5 concluída** — `contract_replay.rs` com **45** testes `replay_*` gerados por
  macro, mais `every_baseline_capture_has_a_replay_test` (falha se surgir caso no baseline sem teste).
  **45/45 verdes** rodando só `cargo test`, sem .NET.
- `[2026-10-22 01:50]` **T6 concluída** — os 2 casos de divergência deliberada viraram asserção
  explícita. **Duas expectativas minhas estavam inventadas** e foram corrigidas contra o binário real:
  (a) `restore` com arquivo ausente **e senha presente** sai 2 com `File not found` + caminho absoluto
  (sem senha ele para no prompt — que é a divergência do item 11); (b) `db-servers test` sem senha no
  cofre sai 40 com a explicação de que falta terminal, **sem travar** — o teste prende exatamente o
  "não trava", então reintroduzir um prompt bloqueante faz o processo do teste nunca retornar.
  Regra operacional adotada: comando que pode pedir senha roda com `-W` ou stdin fechado; num PTY
  interativo ele bloqueia (aconteceu nesta execução).
- `[2026-10-22 02:10]` **T7 concluída** — `contract_manifest.rs` com o parser estrito (7 colunas, IDs
  únicos) e as duas direções. Para o vínculo reverso, **126 marcadores `CONTRACT: IF-NNN` inseridos em
  39 arquivos** (só comentário acima do `fn`, nenhuma mudança de lógica). **As três provas de mutação
  foram executadas e reverteram**: (a) remover um marcador → `IF-002: linha coberta sem marcador`;
  (b) inventar um `Teste` → `error.rs não tem fn nao_existe`; (c) duplicar um ID → `IF-002: id
  duplicado` + `IF-003: marcador sem linha`. Ajuste de desenho registrado: linha `pendente` pode
  apontar para teste ainda não escrito (é o estado intermediário legítimo); a exigência dura vale
  para `coberta`, e o plano só fecha com zero `pendente`.
- `[2026-10-22 02:35]` **T8 concluída** — `contract_text.rs` com **9 snapshots** `insta` dos textos que
  são entregáveis (erro de uso em stdout, sucesso do `add` + orientação de `set-password`, orientação
  de config vazia, `Entry added`, binário ausente, `restore --all` vazio, `backup` sem banco). Antes de
  aceitar, cada snapshot foi **conferido contra a captura manual** do binário — nenhum foi aceito no
  automático. Normalização obrigatória aplicada (`CRLF`→`LF`, `mustel.exe`→`mustel`), senão o snapshot
  só passaria no Windows. Manifesto: +4 linhas de texto (IF-170..IF-173) → **121 linhas, 112 coberta,
  6 pendente, 3 nao-coberta**.
- `[2026-10-22 03:20]` **T9/T10/T13 concluídas** — `crates/mustel-core/tests/contract_decisions.rs` com
  a tabela de exit codes (12 constantes conferidas contra o `AGENTS.md`, + 21/22 de caminho), a regra
  **SQLSTATE ⇒ 10 / sem ⇒ 11**, o quirk **3 no backup × 1 no restore**, a ausência estrutural de campo
  de senha no cadastro (10 chaves conferidas, nenhuma "pass"), porta fora de faixa como erro de parse
  (DIV4) e modo `0600` do `pgpass.conf` sob `#[cfg(unix)]` (DIV8).
- `[2026-10-22 03:30]` **T11 concluída** — o predicado do binário único saiu de dentro do `dispatch`
  para `commands::root_action(bool)`, com teste das duas pontas (`OpenGui`/`PrintUsage`). Só "abrir a
  janela" segue `nao-coberta`.
- `[2026-10-22 03:35]` **T12 concluída** — `--create-if-not-exists` validado na suíte gated contra
  **PostgreSQL 18.6** (container `mustel-tutorial`, 55432): `postgres_integration` **8/8**, incluindo
  `create_database_if_missing_creates_it_once` (cria uma vez; sem a flag, banco ausente falha).
- `[2026-10-22 03:40]` **T14 concluída** — a decisão de confirmação virou predicado puro
  `needs_confirmation(sql, no_confirm, require_confirmation)` com tabela dos três casos (destrutiva
  pede; `--no-confirm` dispensa; cadastro com `requireConfirmation=false` dispensa; leitura nunca pede),
  e o default "sim" do prompt fica no teste de PTY (`confirm_accepts_default_on_enter`).
- `[2026-10-22 03:50]` **T15 concluída** — `contract_surface.rs`: **11/11 comandos** têm caso no
  baseline e **toda flag longa** está classificada. O teste achou **17 flags** exercitadas por testes
  de unidade/CLI que **não** aparecem no baseline; em vez de declará-las exceção (mentira), criei a
  tabela `UNIT_COVERED`, que liga cada flag a um teste **verificado** (entrada inválida falha). 19
  flags ficam em `EXCEPTIONS` com motivo escrito (pool até o T34, flags que exigem servidor/dump vivo,
  `--interactive` no PTY).
- `[2026-10-22 03:55]` **Estado do contrato:** `cargo test --workspace` = **399 testes** (eram 330), tudo
  verde; suíte gated de Postgres **8/8** contra 18.6; cofrinho real sem falhas. Manifesto: **125
  linhas, 122 coberta, 0 pendente, 3 nao-coberta**. fmt/clippy limpos.
- `[2026-10-22 04:10]` **T17 + T18 concluídas — `legacy/` removido.** Checkpoint antes: HEAD `decc678`
  (o commit seu, não mexi), nenhuma tag, nada staged, e todas as mudanças eram deste plano. Depois:
  `rm -rf legacy/` → **130 arquivos rastreados deletados como não-staged**, **275 MB** liberados,
  `git diff --cached` vazio. `oracle/` ficou com `README.md`, `baseline/`, `known-divergences.md`,
  `perf/`. Verificação de risco antes de apagar: `oracle/workdir/legacy-pgpass.backup` era **idêntico**
  ao `pgpass.conf` real do usuário (31 bytes, mesma entrada), então o arquivo real está intacto.
- `[2026-10-22 04:25]` **T19 concluída — harness aposentado.** Removidos `oracle-capture`, `oracle-perf`,
  `cases.rs`, `perf.rs`, o `LegacyConfigSandbox`, o `LegacyPgPassSandbox` e os caminhos do legado em
  `paths.rs`. `oracle-compare` **fica** como diagnóstico e continua verde: **47 casos, 0 falhas, 25
  avisos N4, 2 pulados** — idêntico ao número da migração. O baseline agora é contrato congelado.
- `[2026-10-22 04:35]` **T16 concluída** — `known-divergences.md` ganhou a nota de que **o item 10 nunca
  existiu** (lacuna preservada, sem renumerar) e a lista branca foi reescrita para o mundo sem
  `legacy/` (saíram `.vscode`, as skills C# e o `winget-publish.ps1`; entrou `publish.sh`).
- `[2026-10-22 04:45]` **T20 concluída** — docs vivos atualizados: os dois READMEs viraram documentos do
  Mustel (146 linhas cada; a metade .NET de 366 linhas saiu), `oracle/README.md` explica o baseline
  congelado e o gate, e o `AGENTS.md` perdeu o "em remoção".
- `[2026-10-22 04:50]` **T21 concluída** — `rm -rf` das skills `furlab-command-manager` (10 arquivos),
  `rm` de `.vscode/launch.json` e `tasks.json`, e `mv assets/FurLab_icon.png → assets/Mustel_icon.png`
  com o `alt` dos dois READMEs corrigido. **Nada staged, nenhum commit.** Verificado: nenhuma referência
  a `FurLab_icon`/`FurLab Logo` sobrou fora de `MustelPlans/`.
- `[2026-10-22 05:00]` **T22 concluída — mineração dos docs antigos.** Os 11 documentos foram lidos e
  classificados antes de qualquer remoção: **4 salvos** (`QUERY_COMMAND`, `MULTI_SERVER`,
  `pt-BR/credenciais`, `rust-postgres-spike`) e **7 descartados com razão escrita** (arquitetura .NET,
  specs de grupos não migrados, plano de GUI em Electron, diário de sessão). A tabela de destino está em
  `docs/README.md`, para que a decisão de descartar seja auditável.
- `[2026-10-22 05:20]` **T23 concluída — os quatro quadrantes.** Escritos 11 arquivos em pt-BR:
  `tutorials/primeiros-passos.md` (lição em duas metades), 4 how-tos (vários bancos, automação, backup e
  restore, senhas no cofre), 3 referências (árvore de comandos, `query run`, exit codes e ambiente) e
  4 explicações (credenciais, `tokio-postgres`, CSV como o Npgsql, escada N1–N4). **O tutorial foi
  executado de ponta a ponta** contra o container `postgres:18.6`: e isso **corrigiu o texto duas
  vezes** — o layout real é `results/<timestamp>/consolidated_<ts>.csv` (não arquivos soltos) e o
  `_erros.csv` só existe quando há falha. Duas expectativas minhas estavam inventadas; agora são
  medidas.
- `[2026-10-22 05:30]` **T24 concluída — índice e remoção.** `docs/README.md` é a porta de entrada com a
  bússola do Diátaxis e a tabela de mineração; os **11 arquivos antigos** saíram com `rm` (não-staged, 0
  staged) e `docs/en`/`docs/pt-BR` deixaram de existir. Novo teste `documentation_links_resolve`
  (IF-192) confere todo link relativo de `docs/**` e dos READMEs — verde.
- `[2026-10-22 05:45]` **T25 concluída — as 4 falsificações mordem.** Cada mutação foi aplicada,
  testada e revertida: (1) exit code de uso `1`→`11` → `contract_text` falha; (2) `UTF8_BOM` vazio →
  `a_csv_starts_with_the_bom_and_uses_crlf` falha; (3) `--no-confirm` removido do predicado →
  `only_a_destructive_query_without_no_confirm_asks_for_confirmation` falha; (4) checagem de nome
  duplicado removida → `a_repeated_name_is_refused_with_exit_40` falha. Todos voltaram verdes na
  reversão. **Prova de que os testes não são decorativos.**
- `[2026-10-22 05:55]` **T26 concluída — plano concluído.** Estratégia de verificação inteira:
  `cargo fmt --check` limpo; `clippy --workspace --all-targets --all-features -D warnings` limpo;
  **`cargo test --workspace` = 392 testes, 0 falhas** (eram 330); cofre real (`MUSTEL_KEYRING_TESTS=1`)
  392/0; `postgres_integration` **8/8** contra PostgreSQL 18.6; `contract_replay` **45** casos +
  2 divergentes; `contract_text` 9 snapshots; `contract_manifest` 2; `contract_surface` 2.
  Manifesto final: **126 linhas, 123 `coberta`, 0 `pendente`, 3 `nao-coberta`** com razão escrita.
  `oracle-compare` (diagnóstico) segue **47 casos, 0 falhas, 25 N4, 2 pulados**.
  **Nada commitado e nada staged**: HEAD continua `decc678` (o commit do dono), 0 arquivos no índice,
  nenhuma tag `legacy-final`. Deleções e edições ficaram na árvore para revisão: **130 arquivos do
  `legacy/`**, 11 docs antigos, skills C# e `.vscode` do legado.
  Containers de teste removidos (`mustel-tutorial`, `mustel-t19-pg`); `postgres-ptbr` do dono intacto.
