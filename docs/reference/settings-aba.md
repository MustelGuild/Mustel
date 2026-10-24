# Referência: aba Settings (aparência)

> A aba Settings tem duas seções: **Appearance** (esta página) e **Database list**, a paginação da
> lista de bases, descrita em [`gui.selectionList`](selecao-de-bases.md#paginação-opcional).
>
> O cadastro de **seletores** **não** fica aqui: ele vive na aba **Connections**, na sub-aba
> `Query Selector`, e está descrito na
> [referência da seleção de bases](selecao-de-bases.md#o-cadastro-do-seletor).

A aba **Settings** da janela guarda as preferências de aparência. Tudo o que ela edita fica na seção
`gui` do `mustel.jsonc` — o mesmo arquivo do cadastro de servidores —, então dá para versionar,
inspecionar e editar à mão:

```jsonc
{
  "servers": [ /* ... */ ],
  "defaults": { /* ... */ },
  "gui": {
    "theme": "dark",
    "interfaceFont": "Segoe UI",
    "editorFont": "Fira Code",
    "fontScale": 1.1
  }
}
```

Enquanto ninguém mexe na aba, a seção **não existe** no arquivo: a chave só aparece quando há algo
configurado, e `Restore defaults` a remove de novo.

## Campos

| Campo | O que é | Ausente significa |
|---|---|---|
| `theme` | tema da **janela**: `system`, `light` ou `dark` | `system` (segue o tema do SO) |
| `interfaceFont` | família do texto comum (rótulos, botões, tabela) | a embutida (`Ubuntu-Light`) |
| `editorFont` | família do **editor de SQL** (monoespaçada) | a embutida (`Fira Code`) |
| `fontScale` | multiplicador de **todos** os tamanhos de texto | `1.0` |

Os dois combos listam: a opção **embutida**, a **Fira Code** (mesmo que não esteja instalada) e as
famílias **instaladas no sistema**. A lista do sistema é montada uma vez, quando a aba abre — e os
dois combos têm **campo de busca** (o mesmo widget do campo `Database` da query de seleção), porque
uma máquina típica tem centenas de famílias e achar o nome rolando a lista é o que dói.

## Tema

O primeiro item de Appearance é o controle segmentado **`💻 System | ☀️ Light | 🌙 Dark`**:

| Escolha | O que faz |
|---|---|
| `system` | segue o tema do sistema operacional — **default**, e o comportamento de antes da opção existir |
| `light` | sempre claro |
| `dark` | sempre escuro |

`system` **não** fixa claro nem escuro: o tema acompanha o SO, inclusive se ele mudar com a janela
aberta. Só `light` e `dark` ficam gravados no arquivo — em `system` a chave `theme` não existe, então
quem nunca mexeu no controle não vê o `mustel.jsonc` mudar.

As cores de **erro**, **aviso** e **alerta** vêm do tema ativo, não de valores fixos: o âmbar do
alerta tem uma variante para cada fundo, com contraste medido.

## Faixa do tamanho

`fontScale` aceita **0,80x a 1,25x**. Fora disso, o valor lido é preso na borda — o arquivo é
editável à mão e um `5.0` salvo por engano deixaria a janela inutilizável antes de alguém conseguir
abrir a aba para corrigir.

O **tamanho mínimo da janela cresce junto**: a 1,25x o mínimo passa de 880×560 para 1100×700. É o
que mantém a folga do layout em qualquer escala (a conta está na
[explicação](../explanation/fontes.md)).

## O que a aba não faz

- não muda o **tamanho da janela** (só o mínimo acompanha a escala da fonte);
- não lista fontes que não estejam instaladas e não instala nada;
- não pede nada ao banco e não depende de servidor: a aparência vale antes de qualquer conexão;
- não tem atalho de teclado para trocar de tema: a troca é só pelo controle da aba.

## Se a fonte escolhida desaparecer

Desinstalar a fonte ou abrir o arquivo noutra máquina não quebra a janela: o nome que não resolve cai
na família embutida e a aba mostra um aviso amarelo. Basta escolher outra (ou `Restore defaults`).

## Emoji das abas e símbolos da interface

Os ícones das abas (`🔍`, `🖥`, `⚙`) e os símbolos dos botões (`⟳`, `▼`, `📂`, `🗂`) continuam
funcionando com qualquer fonte escolhida: as fontes embutidas, incluindo as de emoji, ficam **no fim**
da cadeia de fallback.

> **Ícones das abas são desenhados, não emoji.** Emoji vinha do *Segoe UI Emoji* (fallback do sistema)
> e a métrica dessa fonte varia por máquina, o que deixava o glifo acima do texto. Os ícones das abas
> são formas vetoriais, centradas no texto por construção — ver
> [Fontes](../explanation/fontes.md#por-que-os-icones-das-abas-sao-desenhados).

## Quando a mudança é gravada

Escolher uma fonte **grava e aplica na hora**: a janela muda no mesmo quadro e a seção `gui` é
atualizada. Arrastar o slider de tamanho só mostra a prévia enquanto o botão está pressionado, e a
gravação acontece ao soltá-lo — arrastar não escreve o arquivo a cada pixel. Clicar numa opção de
**tema** é escolha concluída, então também grava e aplica no mesmo quadro; `Restore defaults` volta o
tema para `system` e a chave sai do arquivo.
