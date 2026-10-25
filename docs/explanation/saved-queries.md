# Por que as queries salvas são assim

A aba **Query** guarda texto em dois lugares diferentes, e a diferença importa: o **Query Selector**
(na sidebar) guarda queries de **seleção**, que rodam num servidor e banco fixos para marcar bases; as
**queries salvas** guardam só SQL, para não digitar de novo o que se roda toda semana. Esta página
explica por que a segunda existe separada e por que ela se comporta como se comporta.

Para o que existe, botão a botão, veja a [referência](../reference/saved-queries.md). Para o outro
conceito, veja [por que a seleção de bases é assim](selecao-de-bases.md).

## Duas listas, dois propósitos

A tentação é ter uma lista só: afinal, as duas guardam SQL. O problema é que a query de seleção tem
**servidor e banco** — ela roda num lugar definido para descobrir o que marcar — e a query salva não
tem lugar nenhum: ela é texto que vai para o editor, e o editor é que decide onde roda.

Unificar exigiria `server`/`database` **opcionais**, e a tela passaria a ter linhas de dois tipos que
parecem iguais. Pior: o mesmo combo ofereceria coisas que fazem coisas diferentes — uma carrega texto,
a outra marca bases. Por isso são duas chaves no arquivo (`gui.selections` e `gui.savedQueries`) e
dois combos em lugares diferentes.

O CLI também não ganhou comando para isso. Uma query salva é atalho de **interface**: no terminal, o
caminho equivalente já existe (`query run --input arquivo.sql`), e criar um `saved-queries list`
viraria decisão de contrato com o oráculo por um ganho que ninguém pediu.

## Escolher é barato; salvar não volta

As duas ações parecem simétricas e não são:

- **escolher** troca o texto do editor por outro — e trocar de novo desfaz, ou o editor tem desfazer;
- **salvar por cima** joga fora o texto que estava gravado, e isso não volta.

Daí a assimetria: escolher **nunca** pergunta nada; salvar pergunta **só quando o texto do editor
difere** do que foi carregado. Pedir confirmação na escolha treinaria o dono a clicar em "sim" sem
ler — e a confirmação que importa ficaria gasta.

## O combo é a lista

Excluir mora na própria linha da lista aberta do combo, e não numa sub-aba de gerenciamento. O motivo
é o momento: quem quer excluir está com a lista aberta, olhando o nome. Uma tela à parte obrigaria a
sair do fluxo, achar a query de novo e voltar.

A confirmação aparece num **popup**, não numa linha do formulário. Uma linha a mais empurra o editor
para baixo no meio do clique — o formulário se mexe embaixo do cursor, e o alvo do segundo clique
muda de lugar.

Não há desfazer. Desfazer exigiria guardar a query removida em algum lugar e oferecer um caminho de
volta — estado novo, para proteger contra um clique que já pediu confirmação. A exclusão é definitiva,
e a confirmação é deliberada: dois cliques, o segundo num botão que diz `Delete`.

Excluir a query **carregada** não apaga o trabalho: o texto continua no editor e o vínculo se desfaz,
então `Save` volta a pedir um nome. O que se perde é o nome, não o SQL.

## O nome é a chave

O nome é único **sem diferenciar maiúsculas**, como o nome de servidor e o de query de seleção. Duas
entradas `Faturamento` e `faturamento` seriam indistinguíveis na lista e na busca, e a segunda
sobrescreveria a primeira sem ninguém entender por quê.

O diálogo de nome abre **vazio**, sem sugerir `Query 1`. Sugestão automática vira lixo acumulado: a
lista enche de `Query 1`, `Query 2`, `Query 3` e o nome deixa de dizer o que a query faz. Quem salva
sabe o nome que quer — e se não sabe, esse é o momento de decidir.

## Só o SQL

A query salva guarda `{ name, sql }` e mais nada: nem servidor, nem banco, nem diretório de saída.
Diretório de saída é **por execução** e muda conforme o que se está fazendo; servidor e banco são o
escopo da execução, e o escopo é escolhido na sidebar, na hora de rodar.

Guardar isso transformaria a query salva numa "execução preparada", que o produto não tem — e
envelheceria: renomear um servidor ou remover um banco deixaria a query salva apontando para o nada,
sem que a tela tivesse como saber.

## O widget cresceu sem quebrar quem já usava

A lixeira por linha exigiu que o combo com busca soubesse desenhar **uma ação por linha** e devolver
qual foi clicada. A assinatura que os outros três lugares usam (filtro de servidor, Query Selector,
referência do autocomplete) continuou devolvendo só o item escolhido: o caso com ação virou uma
função própria, e a antiga ficou como embrulho dela.

A alternativa — acrescentar um campo ao retorno de todo mundo — obrigaria três telas a mudar por causa
de uma feature de uma. Aqui, quem não usa ação não sabe que ela existe.

## Duas lições que só a janela deu

Os dois bugs reais desta feature não apareciam em teste sem janela:

1. **O diálogo de nome não fechava.** O texto era devolvido para o estado no fim do bloco de desenho;
   fechar o diálogo e reescrever o texto o ressuscitava no quadro seguinte. A regra que ficou: o texto
   do diálogo só volta para o estado **enquanto o diálogo está aberto** — quem fecha é dono do estado.
2. **A confirmação de exclusão nascia coberta.** O popup do combo cobre a área onde a confirmação é
   desenhada, e o texto saía cortado com os botões escondidos. A regra que ficou: clicar numa ação da
   linha **fecha a lista**, para a confirmação aparecer no lugar de cima da tela, e não embaixo dela.

Ambas foram encontradas com a janela de verdade e a captura de tela, e conferidas de novo depois da
correção — teste sem janela não pega diálogo que ressuscita nem popup que cobre o que vem depois.

## O que a feature não faz

Deliberadamente fora, e cada uma pela mesma razão — não muda o ciclo escrever → salvar → escolher:

- **renomear**: `Save as…` com o nome novo e excluir a antiga resolve, sem um terceiro estado (nome
  antigo × novo) para manter;
- **abas múltiplas ou histórico**: o editor é um só; várias abas criam o problema de qual delas é a
  "carregada";
- **atalho de teclado dedicado**: o combo e os botões estão a um clique, e tecla nova é contrato novo;
- **gerenciar em Connections**: a exclusão já vive na lista que se abre na hora de usar.
