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
│ ◀  5 selected                        │ Referência do autocomplete: [Local] ⟳    │
│                                      │ SQL                                      │
│ Server                               │ ┌────────────────────────────────────┐   │
│ [ Local (106) ▼ ] [⟳]                │ │ SELECT 1 AS valor                  │   │
│ Query Selector                       │ └────────────────────────────────────┘   │
│ [ Produção ▼ ] [∅]                   │ Output directory [ ./results ] [📂]       │
│                                      │ [ Run query ]                            │
│ [ Search database… ]                 │                                          │
│ [Select all 103] [Reset]             │ Results                                  │
│ 5 marked · from 'Produção'           │ ┌────────────────────────────────────┐   │
│    ☐ Show only marked                │ │ Server │ Database │ valor           │  │
│ 3 of 104                             │ └────────────────────────────────────┘   │
│ ☑ configurador            Local      │                                          │
│ ☑ dev                     Local      │                                          │
│ ☐ postgres      Local (cinza)        │                                          │
│ ☐ template0     Local (cinza)        │                                          │
│ ☐ demo_001             Local 2       │                                          │
└──────────────────────────────────────┴──────────────────────────────────────────┘
```

A sidebar tem **400 px** fixos, com **12 px de respiro** entre ela e o conteúdo à direita. O botão
`◀` recolhe para uma barra `▶ N selected` na área principal; `▶` reabre. A aba **Servers** usa a
mesma sidebar — a lista do cadastro fica lá, em cards que ocupam a largura toda, e o formulário
ocupa a área principal.

A **janela mínima** é **880 × 560** na escala 1,0x e cresce junto com o `fontScale`
(Settings → Text size): a 100% e a 125% de escala do sistema cabe num 1366×768, mas a 125%
com `fontScale` 1,25 o mínimo passa de 1100 × 700 e pode não caber.

O combo de **Server** mostra o total de bases de cada servidor ao lado do nome (`Local 104`,
`Local 2 0`) — o `0` é o sinal de que aquele servidor ainda não foi descoberto. A contagem é só de
exibição: buscar "104" não encontra servidor nenhum, porque a busca é sobre o nome.

O total é o que o **catálogo do servidor** tem: a descoberta lista `pg_database` inteiro, inclusive
`template0` e `template1`. Os templates aparecem **apagados**, com o motivo no tooltip (`template
database`, ou `datallowconn = false`), e **contam** no total, mas nunca podem ser marcados. Aplicar
um seletor não muda a contagem. O [porquê](../explanation/selecao-de-bases.md#por-que-a-contagem-é-o-catálogo-do-servidor)
está na explicação.

O recorte de **execução** é outro: `query`, `backup --all` e o combo `Database` onde a seleção
roda continuam usando as listas filtradas (sem template e sem `datallowconn = false`). Só a
contagem e a lista da sidebar são do catálogo inteiro.

Base que **só um seletor** conhece (um tenant apontando para uma base que o servidor não tem) também
entra no universo, para não ser barrada em silêncio: aparece na lista e no total do escopo, sem
alterar o catálogo do servidor na próxima descoberta.

O mesmo combo mostra **qual seletor** marcou cada servidor e quantas bases ele tem
(`Producao · 104`), com o nome do seletor truncado em **20 caracteres**
(`Homologacao-Tenant… · 104`); servidor sem seletor aparece `· 104`, e `· 0` é o sinal de
que aquele servidor ainda não foi descoberto.

### Escopo: o filtro manda

O **filtro de servidor** define o escopo de tudo o que está na sidebar:

| Controle | O que faz | Escopo |
|---|---|---|
| `Query Selector` | escolher um seletor **aplica na hora** e substitui a seleção | o filtro: um servidor, ou todos |
| `⟳` ao lado do filtro | lista as bases do escopo | o filtro |
| `Select all N` | marca **o que está visível** | filtro × busca |
| `Clear` / `Reset` | **desfaz a seleção manual**: sem seletor desmarca o visível; com seletor devolve o visível ao que a query marcou | filtro × busca |
| busca | cruza o nome da base **e** o do servidor | — |

`Select all` e o combo têm papéis fixos: um marca tudo, o outro aplica a query. Base barrada por
`excludePatterns` ou recusada pelo servidor, e base com aviso de senha, **nunca** entram
automaticamente. O status diz quantas estão marcadas e de onde vieram:

> `103 marked · from 'Producao'`

O botão da direita muda de **nome** conforme o escopo (é a coluna `Clear` / `Reset` da tabela
acima): sem seletor aparece `Clear`, com seletor aparece `Reset`. Esquecer o seletor (`(none)` ou a
vassoura) devolve o botão ao estado `Clear`. O
[porquê](../explanation/selecao-de-bases.md#por-que-clear-vira-reset) está na explicação.

O toggle **Show only marked**, na mesma linha do status, esconde as bases não marcadas — é
**visão**, não ação: `Select all` e `Clear` continuam valendo para o filtro × busca inteiro, e o
cabeçalho da lista passa a contar só o que está à vista (`3 of 3`).

O recado da última ação vem embaixo, e só conta o que ficou de fora:

> `2 skipped (excluded by excludePatterns) · 1 skipped (no password)`

### Quem lembra o seletor é o servidor

Cada servidor guarda o nome do seletor que o marcou, e o combo mostra o que vale no escopo atual:

| Filtro | Seletores dos servidores | O combo mostra | Escolher aplica em |
|---|---|---|---|
| um servidor | o dele (ou nenhum) | o nome, ou vazio | **só naquele servidor** |
| todos | todos com o mesmo seletor | o nome comum | todos |
| todos | diferentes (ou algum **sem** seletor) | **vazio** | todos (e passa a valer para todos) |

Servidor **sem** seletor conta como um valor diferente: aplicar `todas` só no `Local`, com `Local
2` sem nada, deixa o escopo "todos" **sem** nome — a linha `All servers` do dropdown mostra só a
contagem (`· 209`), e `Local` mostra `todas · 104`. A linha `All servers` descreve o escopo **dela**,
nunca o do filtro atual.

A linha `(none)` **esquece o rótulo** do escopo, sem desmarcar nada: marcar uma base à mão não
apaga o nome do seletor, e o rótulo diz de onde a seleção veio.

Ao lado do combo, o botão `∅` é a **vassoura**: esquece o seletor de **todos** os servidores de uma
vez, sem desmarcar nada, e age mesmo com o filtro num servidor. Fica apagado quando não há seletor
para limpar. As duas linhas (Server e Query Selector) têm a mesma geometria — combos da mesma largura
e o botão lateral na mesma coluna —, e o indicador de progresso aparece **no lugar** da vassoura
enquanto a aplicação roda.

### Falha ao aplicar

Se a carga falhar (servidor fora do ar, host fora do cadastro), a seleção anterior **fica como
estava** e um alerta aparece logo abaixo do combo, com o nome da query, o erro do núcleo e
`Selection unchanged.`:

> ⚠ Could not apply 'Producao'.
> connection refused
> Selection unchanged.

O alerta é dispensável no `✕` e some sozinho quando a próxima aplicação dá certo.

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
  aviso é por servidor. Aplicar um seletor, ao contrário, **aborta** — uma seleção pela metade seria pior
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

A lista deste combo é a de **execução**: não oferece template nem base com `datallowconn = false`,
porque a query precisa conectar ali. Já `postgres` aparece (é uma base comum). A exclusão por
`excludePatterns` vale na aba **Query**, na hora de marcar, e não impede apontar onde a query roda.

Falha na descoberta aparece no próprio formulário, em amarelo, com a mensagem do núcleo (servidor
fora do ar, senha ausente no cofre). Antes disso o `⟳` ficava em silêncio e parecia não fazer nada.

## `Test` no cadastro

O botão **Test** do formulário roda o **mesmo** caminho da carga e mostra só números: servidores,
bases, quantas foram barradas por padrão, quantas ficaram sem senha utilizável e quantas linhas
foram ignoradas. Senha e connection string nunca aparecem.

## Limites

- a seleção **não persiste**: fechar a janela e abrir de novo volta sem nada marcado;
- não há agendamento nem histórico de execuções;
- a lista de bases só conhece o que veio do cadastro (`databases`), do `⟳` e da query — base criada
  depois disso só aparece após outra descoberta.
