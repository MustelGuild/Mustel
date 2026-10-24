# Referência: seleção de bases por query

Na aba **Query**, quais **bases** de quais servidores vão rodar é escolhido por checkbox, e uma
**query salva** pode marcar tudo em massa. Os servidores são sempre os do **cadastro**
(`mustel settings db-servers`); o que a query decide é só *quais bases*.

O cadastro das queries fica na aba [Settings](settings-aba.md), na seção **Database selection
queries**, e é gravado em `gui.selections` do `mustel.jsonc`:

```jsonc
{
  "gui": {
    "selections": [
      { "name": "Produção", "server": "configurador", "database": "configurador",
        "sql": "select host, port, database from tenants where ambiente = 'prod'" }
    ]
  }
}
```

Enquanto a lista está vazia, a chave **não existe** no arquivo.

## A tela

Servidores e escolha de bases ficam numa **sidebar** à esquerda; a área principal é da execução
(SQL, saída, `Run query` e resultados). A referência do autocomplete continua **em cima do editor**.

A lista é **plana e alfabética**: cada linha é uma base, e o **servidor é um filtro** — não há
agrupamento nem colapso. O nome do servidor aparece em cada linha, porque o mesmo nome de base pode
existir em mais de um servidor.

```text
┌──────────────────────────────────────┬──────────────────────────────────────────┐
│ ◀  5 selected                        │ Referência do autocomplete: [Local] [conf…] ⟳ │
│                                      │ SQL                                      │
│ Databases [ Producao             ▼ ] │ ┌──────────────────────────────────────┐ │
│ Server    [ Local · 3 of 104     ▼ ] │ │ SELECT 1 AS valor                    │ │
│ [⟳ Apply to Local] [⟳ Discover Local]│ └──────────────────────────────────────┘ │
│ [Select all 103] [Clear]             │ Output directory [ ./results ] [📂] [🗂]  │
│ [ Search database…                ] ✕│ [ Run query ]                             │
│ 5 marked                             │                                          │
│ 3 of 104                             │ Results                                  │
│ ☑ configurador                 Local │ ┌──────────────────────────────────────┐ │
│ ☑ dev                          Local │ │ Server │ Database │ valor                │ │
│ ☐ postgres            Local (cinza)  │ └──────────────────────────────────────┘ │
│ ☐ demo_001                   Local 2 │                                          │
└──────────────────────────────────────┴──────────────────────────────────────────┘
```

A sidebar tem **300 px** fixos, com **12 px de respiro** entre ela e o conteúdo à direita. O botão
`◀` recolhe para uma barra `▶ N selected` na área principal; `▶` reabre. A aba **Servers** usa a
mesma sidebar — a lista do cadastro fica lá, em cards que ocupam a largura toda, e o formulário
ocupa a área principal.

### Escopo dos botões

O **filtro de servidor** define o escopo de tudo:

| Ação | Escopo |
|---|---|
| `⟳ Apply to …` | roda a query salva e **substitui** a seleção do escopo (um servidor, ou todos) |
| `⟳ Discover …` | lista as bases do escopo |
| `Select all N` | marca **o que está visível**: filtro × busca |
| `Clear` | desmarca o que está visível |
| busca | cruza o nome da base **e** o do servidor |

`Select all` e `Apply` têm papéis fixos: um marca tudo, o outro aplica a query — nenhum dos dois muda
de significado conforme o outro controle. Base barrada por `excludePatterns` e base com aviso de senha
**nunca** entram automaticamente, e o status conta o que ficou de fora:

> `206 marked · 2 skipped (excluded by excludePatterns) · 1 skipped (no password)`

### Paginação (opcional)

Desligada por padrão: a lista desenha todas as bases do filtro. Quem tem milhares liga em
**Settings → Database list**, que define também o tamanho da página (10–500). Ligada, aparece um
`Load N more (M left)` no fim da lista. A busca **ignora** a paginação — resultado de busca é sempre
completo.

```jsonc
"gui": {
  "selectionList": { "paginate": true, "pageSize": 50 }
}
```

A chave só aparece no arquivo quando foge do padrão (desligado, 50).

## Colunas aceitas pela query

A query de seleção pode devolver a **connection string** inteira ou colunas soltas.

| Forma | Colunas |
|---|---|
| connection string | `connection_string` (ou `connectionstring`, `conn_str`, `connstr`); se nenhuma tiver esse nome, a **primeira** coluna que contém `=` é lida como string |
| colunas soltas | `host` (ou `data source`), `port` (texto **ou** inteiro), `database` (ou `initial catalog`), `username` (ou `user id`, `userid`, `user`), `password` (ou `pwd`), `server` (ou `server_name`, `servername`) |

O que a string traz vence em host, porta, base, usuário e senha. Linha sem **base**, ou sem **host
e sem `server`**, é ignorada e contada no status.

`server` é o **nome do servidor cadastrado**, não um alias de host: é ele que desempata quando dois
servidores compartilham o mesmo host e porta (por exemplo `Local` e `Local 2` em `127.0.0.1:5432`).
O alias `Server=` só vale **dentro** da connection string.

## Regras

- **Host fora do cadastro aborta a carga inteira**, com o erro agregado: `3 host(s) not registered
  (12 row(s)): …`. Cadastrar o servidor é o que resolve; a seleção anterior fica como estava.
- **Host e porta que casam com dois servidores** abortam listando os candidatos. A saída é a coluna
  `server`.
- **`excludePatterns` do cadastro sempre valem**, inclusive sobre o que a query trouxe: a base
  aparece **desabilitada**, com o padrão no tooltip, e o status conta quantas foram barradas.
- **Descoberta é melhor esforço:** um servidor fora do ar não impede os outros de aparecer, e o
  aviso é por servidor. O `⟳ Apply`, ao contrário, **aborta** — uma seleção pela metade seria pior
  que nenhuma.
- **Recarga que perde base marcada avisa:** quem estava marcado e sumiu do servidor aparece no
  status.
- **`Run query` roda as bases marcadas**, explícitas: servidor sem base marcada fica de fora, e base
  barrada por padrão nem chega ao plano.

## O formulário do cadastro

Na aba **Settings**, o campo `Database` é onde a query de seleção **roda** — um banco do servidor
escolhido, lido com a senha do cofre. Ele funciona assim:

| Elemento | O que faz |
|---|---|
| **Server** | escolher o servidor **já carrega** as bases dele; não precisa apertar nada |
| **spinner + "Loading databases…"** | aparece enquanto a descoberta está em andamento |
| **Database** | um dropdown no estilo dos combos de fonte: fica **vazio** sem escolha e mostra o nome depois |
| **busca do dropdown** | filtra a lista sem diferenciar maiúsculas enquanto você digita |
| **⟳** | refaz a descoberta (útil quando alguém criou uma base depois do último carregamento) |

Não há campo de digitação solto no formulário: o nome entra pelo dropdown. A lista vem da
descoberta (`⟳`) e do `databases` do cadastro — se a base não aparecer, rode o `⟳` de novo: o
cadastro do dono costuma usar `fetchAllDatabases`, e a lista só fica completa depois de descobrir.

Escolher `postgres` ou `template0` é permitido — a exclusão por `excludePatterns` vale na aba
**Query**, na hora de marcar, e não impede apontar onde a query roda.

Falha na descoberta aparece no próprio formulário, em amarelo, com a mensagem do núcleo (servidor
fora do ar, senha ausente no cofre). Antes disso o `⟳` ficava em silêncio e parecia não fazer nada.

## `Test` no cadastro

O botão **Test** do formulário roda o **mesmo** caminho da carga e mostra só números: servidores,
bases, quantas foram barradas por padrão, quantas ficaram sem senha utilizável e quantas linhas
foram ignoradas. Senha e connection string nunca aparecem.

## Limites

- a seleção **não persiste**: fechar a janela e abrir de novo volta sem nada marcado;
- não há agendamento nem histórico de execuções;
- a lista de bases só conhece o que veio do cadastro (`databases`), do `⟳ Discover` e da query — base
  criada depois disso só aparece após outra descoberta.
