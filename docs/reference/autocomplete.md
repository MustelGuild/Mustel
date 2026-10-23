# Referência: autocomplete do editor de SQL

Esta página descreve **o que a janela oferece** no editor de SQL: de onde vêm as sugestões, o que é
sugerido em cada contexto, quais teclas agem e quais são os limites. Para aprender a usar, veja o
[how-to do autocomplete](../how-to/autocomplete-no-editor.md); para entender as escolhas, a
[explicação](../explanation/autocomplete.md).

## Origem das sugestões

As sugestões vêm de um **retrato do catálogo de um banco**, carregado sob demanda. Não há parser de
SQL nem acesso ao banco a cada tecla: a cada tecla o motor só lê a cópia que está em memória.

O retrato contém:

| Objeto | O que é guardado | Detalhe mostrado no popup |
|---|---|---|
| Tabela, view, view materializada, tabela particionada | nome, schema e colunas | `N colunas` |
| Coluna | nome e tipo como o Postgres escreve (`text`, `numeric(10,2)`) | `tabela.coluna (tipo)`; no caso `alias.`, só o tipo |
| Função e procedure | nome e assinaturas de `pg_get_function_arguments` | a assinatura (`text, integer`) |

Ficam de fora `pg_catalog` e `information_schema`. O que o usuário da conexão não pode ver também não
aparece — o filtro é o do próprio Postgres.

## A linha "Referência do autocomplete"

Fica **antes do editor de SQL**, no topo da tela de consulta, e tem quatro partes:

| Parte | O que faz |
|---|---|
| Combo de servidor | escolhe o servidor do cadastro; trocar de servidor **limpa** o banco escolhido |
| Combo de banco | lista os bancos que o **servidor** tem, descobertos em `pg_database` ao escolher o servidor; sem descoberta, cai para os bancos do cadastro (`databases`) |
| `⟳` | carrega (ou recarrega) o retrato do banco escolhido |
| Rótulo de estado | `não carregado`, `carregado (banco)` ou `carregado outro banco` |

A descoberta de bancos roda sozinha quando o servidor é escolhido (uma vez por servidor, em
background) e é o que faz o combo listar **todos** os bancos — inclusive os que não estão no cadastro,
como nos servidores com `fetchAllDatabases`. A barra de status acompanha:
`Discovering databases on '<servidor>'...` e, ao terminar, `'<servidor>' has N database(s).`.

Enquanto a lista não chegou, o combo vazio diz `descobrindo bancos…`; se a descoberta falhar e o
cadastro também estiver vazio, diz `nenhum banco no cadastro`.

O `⟳` pede servidor **e** banco escolhidos e **senha no cofre**: sem senha guardada o botão fica
desabilitado, e o tooltip diz como guardar. A janela **nunca** pergunta senha.

A barra de status acompanha o carregamento: `Loading schema of '<banco>' on '<servidor>'...` e, ao
terminar, `Schema of '<banco>' loaded: N relation(s), M function(s).`

Cada par `(servidor, banco)` fica em cache enquanto a janela estiver aberta. Recarregar substitui a
entrada daquele par; trocar de servidor ou de banco **não** descarta as outras entradas.

Se o servidor escolhido sair do cadastro (removido na aba Servers), a linha **volta ao início**:
servidor e banco em `—`, rótulo `não carregado`, e o esquema guardado daquele servidor é descartado —
nada de sugestão vinda de um servidor que já não existe.

## O que é sugerido, por contexto

O contexto é a **última palavra-chave antes do cursor**. As tabelas citadas na consulta são
consideradas mesmo quando estão **depois** do cursor (`SELECT c. FROM clientes c`).

| Última palavra-chave | Sugere |
|---|---|
| `FROM`, `JOIN`, `INTO`, `UPDATE`, `TABLE` | relações e **nomes de CTE** da própria consulta |
| `SELECT`, `WHERE`, `ON`, `GROUP`, `HAVING`, `ORDER`, `SET` | colunas das tabelas citadas e funções |
| nenhuma (início da consulta) | palavras-chave, relações e funções |

Três formas qualificadas:

| O que você digita | O que aparece |
|---|---|
| `alias.` (`SELECT c. FROM clientes c`) | colunas da tabela daquele alias, com o tipo |
| `schema.` (`vendas.`) | tabelas daquele schema, já qualificadas |
| nome de tabela que existe em **um** schema | nome simples (`clientes`) |
| nome de tabela que existe em **dois ou mais** schemas | nome qualificado (`vendas.clientes`) |

Funções entram como `nome(` — o parêntese já vem na inserção.

## Teclas

| Tecla | Efeito |
|---|---|
| digitar a **1ª letra** de uma palavra | abre a lista, já filtrada |
| começo de um token (logo após `FROM `, por exemplo) | abre com a lista do contexto |
| `Ctrl+Space` | abre em qualquer posição, inclusive no começo do texto |
| `↑` / `↓` | move a seleção |
| `Tab` / `Enter` | aceita a seleção e substitui a palavra sob o cursor |
| `Esc` | fecha a lista |
| clique numa linha | aceita aquela sugestão |

A exceção do gatilho automático é o **começo do texto com a caixa vazia**: ali não há contexto, e a
lista só abre com `Ctrl+Space`.

A lista abre **abaixo** da caixa de SQL e, quando não há espaço, acima. A altura para em 220 px;
depois disso ela rola.

Depois de aceitar, a lista fecha e só volta quando você digitar de novo (ou pedir `Ctrl+Space`): o
nome recém-inserido não reaparece como sugestão única, já selecionado.

## Limites

| Limite | Valor |
|---|---|
| Candidatos por vez | **50** |
| Ordem | igual, depois prefixo, depois “contém”; duplicados saem |
| Dentro de string ou comentário | **nada** é sugerido |
| Um retrato por vez em uso | só o do banco escolhido |
| Atualização | só ao clicar `⟳`; DDL feito fora da janela não aparece sozinho |
| Esquema carregado | sem ele, não há sugestão nenhuma |

## Limitações conhecidas

Separadas por origem: o que é **decisão** (e o porquê está na [explicação](../explanation/autocomplete.md))
e o que é **defeito conhecido**.

| Limitação | Origem | Consequência |
|---|---|---|
| Colunas de CTE não são sugeridas | decisão (meio-termo) | `totais.` não sugere nada; `FROM to` já oferece `totais` |
| Metadados, não DDL | decisão | a janela sugere nomes; não gera `CREATE` |
| Um banco de referência | decisão | quem tem bancos diferentes digita sem sugestão do outro |
| `Esc` fecha pela palavra | **defeito do egui** | depois de `Esc` numa palavra vazia, a lista não reaparece em **nenhuma** palavra vazia até você digitar uma letra |
| Aceitar com o cursor no **meio** da palavra | **defeito** | o popup substitui só o que está à esquerda do cursor: `clien|tes` + aceitar `clientes` vira `clientestes` |
| Descoberta de bancos depende do servidor | consequência | se a conexão falhar, o combo mostra os bancos do cadastro; sem bancos cadastrados ele fica vazio (`nenhum banco no cadastro`) |

O contrato dessas decisões está em [`docs/contrato-de-interface.md`](../contrato-de-interface.md)
(linhas `IF-202` a `IF-212`), cada uma com o teste que a prova.
