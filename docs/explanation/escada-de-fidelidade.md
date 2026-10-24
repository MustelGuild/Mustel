# A escada de fidelidade (N1–N4)

O Mustel nasceu como reescrita de um produto existente. "Ficar igual" não é um objetivo útil —
igual em quê? A escada abaixo é o que o projeto trata como contrato, e ela explica por que algumas
diferenças são bugs e outras não são.

| Nível | Exige | Bloqueia? |
|-------|-------|-----------|
| **N1** | mesma árvore de comandos, flags e defaults; inválidos rejeitados do mesmo jeito | **sim** |
| **N2** | mesmos artefatos byte a byte, mesmos exit codes | **sim** |
| **N3** | mesmos fluxos interativos (perguntas, ordem, atalhos, defaults) | **sim** |
| **N4** | formatação textual: `--help`, cores, tabelas, redação de erros | não |

## Por que N4 não bloqueia

N1–N3 são o que um script percebe: se o comando existe, se aceita as mesmas flags, se escreve o mesmo
arquivo e sai com o mesmo código. N4 é o que um humano vê: a frase do erro, a borda da tabela, a cor.
Perseguir byte a byte o desenho do terminal do .NET significaria reimplementar `Spectre.Console` —
custo alto, valor baixo, e o resultado seria frágil.

A regra prática: **se um pipe, um `grep` ou um `if` do shell depende disso, é N1–N3; se só o olho
depende, é N4.**

## Como cada nível é garantido hoje

| Nível | Mecanismo |
|---|---|
| N1 | `contract_replay` (casos de ajuda comparam o conjunto de flags) + `contract_surface` |
| N2 | `contract_replay` (exit code + artefatos) + os testes de `value.rs`/`csv_writer.rs` |
| N3 | testes de PTY (`pty_prompts`) + `ui_primitives` |
| N4 | snapshots em `contract_text` — a deriva **aparece**, mas é aceita com revisão |

O detalhe de cada decisão está em [`contrato-de-interface.md`](../contrato-de-interface.md): uma linha
por decisão, com o teste que a prova. As diferenças conscientes de N4 moram em
[Diferenças deliberadas](diferencas-deliberadas.md).

### Como cada nível é comparado

| Item | Nível | Regra |
|---|---|---|
| exit code | **contrato** | igual ao produto anterior, sempre |
| artefatos (config) | **contrato** | JSON equivalente, com `encryptedPassword`/`password` removidos |
| artefatos (pgpass) | **contrato** | conteúdo **byte a byte** |
| flags longas nos casos de ajuda | **contrato** | toda flag do produto anterior precisa existir no Mustel; extras são informativas |
| stdout/stderr | aviso **N4** | a redação difere por framework; pulado quando a saída do antigo não é UTF-8 |
| comando não migrado | **reprovava** | não se aplica mais: não há mais stub, e `Skipped` reprova |

## O que a escada **não** cobre

- Comportamento que dependia de algo que não existe mais (blob DPAPI, por exemplo) — vira decisão de
  produto, não paridade.
- Fluxos que exigem console dos dois lados (prompts): comparados por teste de PTY no Mustel e por
  smoke manual, não pelo baseline.
- Performance: não é nível de paridade, e não é medida por teste. O custo de partida da GUI e o build
  enxuto (`--no-default-features`, 7,8 ms × 63 ms) estão no `README.md`.
