# Referência: queries salvas

> Ficam na aba **Query**, **acima do editor**. São um atalho de digitação: escolher uma copia o SQL
> para o editor, onde ele continua editável. Não confundir com o **Query Selector** da sidebar, que
> guarda queries de **seleção** (rodam num servidor e banco fixos para marcar bases).

## O combo

`Saved query:` lista `(none)` e as queries salvas, com busca. Escolher uma **carrega o SQL no
editor na hora**, sem confirmação — o texto anterior é substituído. `(none)` não toca no texto:
ela só desfaz o vínculo com a query carregada.

Cada linha salva tem uma **lixeira `🗑`** à direita. Clicar nela **não** seleciona a query: abre um
**popup** de confirmação — `Delete '<nome>'? This cannot be undone.`, com `Delete` e `Cancel` — em vez
de empurrar o formulário com uma linha extra. A exclusão é **definitiva**: não há desfazer nem lixeira
de onde recuperar.

## Os botões

Ficam **na mesma linha do combo**, à direita dele — gravar é o outro lado de escolher.

| Botão | O que faz |
|---|---|
| `💾 Save` | grava o texto do editor sobre a query **carregada**; sem nada carregado, pede um nome |
| `Save as…` | sempre pede um nome, começando **vazio** |

Quando o editor tem texto **diferente** do que foi carregado, `Save` **avisa antes**, num **popup**:
`Replace '<nome>' with the text in the editor?` — só substitui no `Replace`. Escolher uma query no
combo nunca pede confirmação; o aviso existe só onde o conteúdo se perde.

O nome é **único sem diferenciar maiúsculas** (`Faturamento` e `faturamento` são a mesma query) e
vazio é recusado na própria janela. Gravar em cima de um nome existente mantém a **posição** dela na
lista.

## No arquivo

Ficam em `gui.savedQueries`, cada uma com **nome e SQL** (sem servidor, banco ou diretório de saída):

```jsonc
{
  "gui": {
    "savedQueries": [
      { "name": "Faturamento por mês", "sql": "SELECT ..." }
    ],
    "selections": [ /* queries de seleção, outro conceito */ ]
  }
}
```

A chave **não existe** enquanto a lista está vazia, como o resto de `gui`. As duas listas convivem no
mesmo arquivo e gravar uma **não** apaga a outra.

## Quando não há

Se a query carregada sumir da lista (excluída por fora, arquivo editado à mão), a tela solta o
vínculo e mantém o texto do editor: `Salvar` volta a pedir um nome.
