# Escolher quais bases vão rodar na janela

**Situação:** você abriu `mustel gui` e precisa que o `Run query` rode em algumas bases — não em
todas — e talvez por um critério que já está guardado numa query (`select … from tenants`).

A escolha mora na **sidebar** da aba **Query**. A área principal é da execução; nada de servidor ou
de base fica lá.

## Marcar à mão

1. No combo **Server**, escolha o servidor (ou deixe `All servers`).
2. Opcional: use a busca para achar a base — ela cruza o nome da base **e** o do servidor.
3. Marque as caixas que vão rodar. O contador no topo (`N selected`) e o status da sidebar
   (`5 marked`) acompanham.
4. `Run query`.

`Select all N` marca tudo o que está na tela (filtro × busca). Base **apagada** não é alvo: ela
casa com um `excludePatterns` do cadastro ou o próprio servidor a recusa (template), e o tooltip
diz qual dos dois.

## Usar um seletor salvo

O seletor é uma query cadastrada na aba **Connections**, na sub-aba **Query Selector** — o mesmo
lugar do cadastro de servidores, uma sub-aba ao lado (a troca é o controle segmentado no topo da
sidebar; o `•` avisa quando o outro lado tem rascunho aberto). Lá o formulário abre na área principal, com o
editor SQL largo e o `Test` embaixo. O campo `Reads from` diz de onde a query **lê**; o que ela marca
pode cair em qualquer servidor do cadastro.

1. No combo **Query Selector**, escolha o seletor. **Escolher já aplica** — não existe botão de
   aplicar.
2. Confira o status: `5 marked · from 'Producao'`. Ele diz quantas bases ficaram marcadas e de onde
   vieram.
3. Se quiser conferir o que veio, ligue **Show only marked**: a lista passa a mostrar só as
   marcadas (`3 of 3`), sem mudar o que os botões fazem.

O escopo da aplicação é o filtro de servidor: com um servidor filtrado, escolher aplica **só nele**;
com `All servers`, aplica em todos. Cada servidor guarda o seu — dá para deixar `Local` em
`Producao` e `Local 2` em `Homologacao` e trocar o filtro para ver o nome de cada um.

## Desfazer a bagunça

Depois de aplicar, marcar e desmarcar à mão desvia a seleção do que a query disse. Para voltar ao
que ela marcou, use **`Reset`** (o botão que fica ao lado de `Select all`): o que você marcou à mão
sai e o que você desmarcou à mão volta. Sem seletor aplicado, o mesmo botão aparece como `Clear` e
só desmarca.

Se o que falta é base **nova** (criada depois da última aplicação), escolha o seletor de novo — o
`Reset` devolve o resultado que foi carregado, não reexecuta a query.

## Ver o que o servidor tem

- `⟳`, ao lado do combo de servidor, lista as bases do escopo (um servidor, ou todos).
- O número ao lado do nome (`Local 106`) é quanto o **catálogo** daquele servidor tem, templates
  incluídos. Ele não muda quando você aplica um seletor.
- Base que o seletor conhece mas o servidor não tem também aparece, apagada, para não sumir em
  silêncio.

## Limpar o seletor de todos os servidores

O `∅` ao lado do combo **Query Selector** esquece o seletor de todos os servidores de uma vez, sem
desmarcar nada. A linha `(none)` do combo faz o mesmo só para o escopo atual.

## Se a aplicação falhar

A seleção anterior fica como estava e um alerta aparece abaixo do combo, com o nome da query, o erro
do núcleo (servidor fora do ar, host fora do cadastro) e `Selection unchanged.`:

> ⚠ Could not apply 'Producao'.
> error connecting to server
> Selection unchanged.

Dispense no `✕` ou corrija a causa e escolha o seletor de novo — o alerta some sozinho quando a
próxima aplicação der certo.

## Onde ler mais

- o que cada controle faz, campo a campo: [referência da seleção de bases](../reference/selecao-de-bases.md)
- por que o combo aplica, por que o `Clear` vira `Reset` e por que a contagem é o catálogo:
  [explicação](../explanation/selecao-de-bases.md)
- o mesmo assunto pelo terminal: [rodar em vários bancos](varios-bancos.md)
