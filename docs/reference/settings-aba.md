# Referência: aba Settings (aparência)

A aba **Settings** da janela guarda as preferências de aparência. Tudo o que ela edita fica na seção
`gui` do `mustel.jsonc` — o mesmo arquivo do cadastro de servidores —, então dá para versionar,
inspecionar e editar à mão:

```jsonc
{
  "servers": [ /* ... */ ],
  "defaults": { /* ... */ },
  "gui": {
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
| `interfaceFont` | família do texto comum (rótulos, botões, tabela) | a embutida (`Ubuntu-Light`) |
| `editorFont` | família do **editor de SQL** (monoespaçada) | a embutida (`Fira Code`) |
| `fontScale` | multiplicador de **todos** os tamanhos de texto | `1.0` |

Os dois combos listam: a opção **embutida**, a **Fira Code** (mesmo que não esteja instalada) e as
famílias **instaladas no sistema**. A lista do sistema é montada uma vez, quando a aba abre.

## Faixa do tamanho

`fontScale` aceita **0,80x a 1,25x**. Fora disso, o valor lido é preso na borda — o arquivo é
editável à mão e um `5.0` salvo por engano deixaria a janela inutilizável antes de alguém conseguir
abrir a aba para corrigir.

O **tamanho mínimo da janela cresce junto**: a 1,25x o mínimo passa de 640×480 para 800×600. É o que
mantém a folga do layout em qualquer escala (a conta está na [explicação](../explanation/fontes.md)).

## O que a aba não faz

- não muda o **tema** (claro/escuro) nem o tamanho da janela — só fonte, por enquanto;
- não lista fontes que não estejam instaladas e não instala nada;
- não pede nada ao banco e não depende de servidor: a aparência vale antes de qualquer conexão.

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
gravação acontece ao soltá-lo — arrastar não escreve o arquivo a cada pixel.
