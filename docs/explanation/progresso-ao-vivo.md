# Por que uma tabela ao vivo, e não uma barra de progresso

Durante o `query run` o Mustel não mostra uma barra que enche. Mostra uma **tabela que cresce**: cada
banco que termina ganha uma linha, e no fim a tabela **fica na tela**, com o resumo e os caminhos dos
arquivos logo abaixo.

Isso não é preferência de desenho: é o comportamento que o produto anterior tinha, e há razões
práticas para mantê-lo.

## O que aparece

```text
╭────────┬──────────┬──────────┬───────┬──────────┬────────╮
│ Status │ Server   │ Database │ Rows  │ Duration │ Detail │
├────────┼──────────┼──────────┼───────┼──────────┼────────┤
│ ✓      │ dev      │ app_dev  │ 1204  │ 0.3s     │ —      │
│ ✗      │ dev      │ legado   │ 0     │ —        │ erro…  │
╰────────┴──────────┴──────────┴───────┴──────────┴────────╯
```

- **Status**: `✓` sucesso, `✗` falha.
- **Rows**: quantas linhas aquele banco devolveu.
- **Duration**: uma casa decimal no sucesso (`0.3s`); **traço** na falha.
- **Detail**: o erro, quando há.

Antes da tabela, a fase de descoberta usa um **spinner** (`Discovering databases on <servidor>...`),
cancelável com `Ctrl+C` como o resto.

## Por que não uma barra simples

Uma barra responde "quanto falta?". O `query run` tem uma pergunta mais útil: **qual banco falhou**.
Quando um servidor tem 40 bancos e um deles recusa a conexão, a barra chega a 100% e você descobre o
problema lendo o `_erros.csv` depois. A tabela mostra o `✗` na hora, com a mensagem na linha.

A barra também pressupõe um total fixo. No `--all` o total só é conhecido **depois** da descoberta, e
um banco descoberto pode ser excluído no meio — o número mudaria debaixo do usuário.

## O que é contrato, e o que é desenho

O **conteúdo** das linhas é contrato: quem terminou, quantas linhas, em quanto tempo, com que erro.
O **desenho** (bordas, cores, largura) é N4 — o produto anterior desenhava com o Spectre pelo codepage
do console; o Mustel usa `indicatif` + `tabled` e escreve UTF-8.

Por isso os testes fixam as **linhas** e não a moldura: `ui_primitives.rs` verifica o conteúdo, e o
replay do baseline compara o que é estável entre plataformas.

## Fora de terminal, nada é desenhado

`query run` em script, pipe ou CI **não** escreve tabela nem spinner. O produto anterior misturava
progresso e resultado no mesmo stream, o que suja qualquer redirecionamento; aqui o indicador é
**oculto** quando não há terminal, e o que sai é só o resumo e os caminhos.

Isso também é o que torna o `query run` testável: o teste de contrato lê stdout sem precisar filtrar
moldura.

## Os eventos por trás

A UI não conhece o executor: ela consome `CoreEvent` (`DatabaseFinished`, `FilesWritten`, `Summary`,
`DiscoveringDatabases`) pelo `EventSink`. É o mesmo canal que a GUI usa — a diferença é só quem
desenha.

Consequência prática: os eventos de arquivos e resumo são **enfileirados** e impressos **depois** de a
tabela ser encerrada. Imprimir durante o desenho apagaria o texto (medido durante a migração).
