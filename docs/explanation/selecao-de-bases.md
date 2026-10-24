# Por que a seleção de bases é assim

A lista de bases poderia ser só uma decisão por servidor: "roda em todos" ou "roda nestes". Na
prática ela é um cadastro à parte — o dono guarda os *tenants* e suas connection strings num banco,
e é **de lá** que sai a resposta de quais bases existem hoje. Esta página explica por que a solução
tem a forma que tem.

Para o que existe, campo a campo, veja a [referência](../reference/selecao-de-bases.md).

## A string é ponteiro, não cadastro

A tentação óbvia é deixar a query devolver a connection string inteira e conectar nela. O dono tem
um banco que guarda exatamente isso: `Host=…;Port=…;Database=…;Username=…;Password=…` por tenant.

O Mustel não faz isso, por duas razões que se somam:

1. **A senha já tem um lugar.** O cofre do sistema guarda uma senha por servidor cadastrado
   (`keyring`). Se a GUI passasse a aceitar senha vinda de uma tabela, existiriam dois cofres — e o
   mais frágil seria o que qualquer um lê com um `SELECT`.
2. **O cadastro é o que o resto do produto usa.** Timeout, `sslMode`, paralelismo e
   `excludePatterns` vivem no servidor cadastrado. Uma conexão avulsa não teria nada disso, e o
   resultado seria um servidor que funciona na janela e não no CLI.

A string continua sendo lida — é o formato mais comum, e o dono já tem os dados nele. Mas ela
identifica **qual servidor do cadastro** casa com aquela linha. Host que não existe ali é **erro**,
não um servidor novo: o produto não inventa cadastro a partir de uma query.

Consequência assumida: quem tem host fora do cadastro precisa cadastrá-lo antes. É por isso que a
mensagem de erro agregada diz quantos hosts e quantas linhas, e ensina o comando — a carga inteira
aborta em vez de marcar "quase tudo".

## Dois servidores no mesmo host e porta

Com dois servidores em `127.0.0.1:5432`, casar só por host e porta é ambíguo. Escolher um "mais
parecido" seria adivinhar sobre qual banco de produção vai rodar um `DELETE`. A decisão foi **pedir
a coluna `server`**: a ambiguidade vira erro com os nomes dos candidatos, e a saída é uma coluna que
o dono controla.

## Por que a exclusão aparece na tela

`excludePatterns` nasceu como regra de *descoberta*: ao listar os bancos do servidor, `template*` e
`postgres` ficam de fora para ninguém rodar `DROP` no catálogo. Uma seleção em massa pode trazer
exatamente esses nomes de volta — e, se a tela simplesmente os omitisse, o dono veria "312 marcadas
"
sem saber que duas ficaram para trás.

Então a base barrada **aparece**: desabilitada, com o padrão que a barrou no tooltip, e contada no
status. A regra do cadastro continua valendo (ela nunca é marcada nem chega ao plano), mas deixa de
ser invisível.

## Por que a seleção não persiste

A seleção é o *escopo da próxima execução*, não uma preferência. Guardada no arquivo, ela viraria
estado escondido: abrir a janela outro dia e apertar `Run query` rodaria em bases que ninguém
revisou hoje.

O que persiste é só a **query** (`gui.selections`), que é conhecimento reutilizável: ela descreve
como descobrir o conjunto, não o conjunto em si. Reaplicar é um clique e mostra o resultado na hora.

## Descoberta em melhor esforço, carga que aborta

As duas operações falham de formas diferentes de propósito:

- `⟳ Discover` só quer **enumerar**. Um servidor fora do ar não pode impedir que os outros mostrem
  suas bases, então cada servidor responde por si e o aviso é por servidor.
- `⟳ Apply` quer **decidir o que vai rodar**. Se metade das linhas não casa com o cadastro, marcar a
  outra metade produziria uma execução com escopo que o dono não escolheu. Aborta-se tudo, com o
  erro agregado.

## Bases explícitas no plano

A tentação de deixar o núcleo descobrir os bancos (`fetchAllDatabases`) é grande: ele já faz isso bem.
Mas descoberta é do servidor, não da seleção — e o que a tela marcou precisa ser o que roda. O
pedido leva `(servidor, bases)` com `fetch_all_databases: false` e `all: false`, e servidor sem base
marcada nem entra. Os `excludePatterns` vão junto porque o plano os aplica sempre: é a segunda
barreira, para o caso de uma base barrada escapar da tela.
