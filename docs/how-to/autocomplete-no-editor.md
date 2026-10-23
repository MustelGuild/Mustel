# Ter sugestões ao escrever SQL na janela

**Situação:** você abre a janela do Mustel para escrever uma consulta e quer que os nomes de
tabelas, colunas e funções apareçam enquanto digita — sem decorar o esquema e sem consultar
`information_schema` no meio.

## Antes de começar

O autocomplete lê o catálogo com a **senha que está no cofre**. Se o servidor ainda não tem senha
guardada, resolva isso primeiro: [Guardar e trocar a senha de um servidor](senhas-no-cofre.md).

```bash
mustel settings db-servers set-password dev
```

## Carregar o esquema

1. Abra a janela: `mustel gui` (ou `mustel`, em terminal interativo).
2. No topo da tela de consulta, ache a linha **Referência do autocomplete**, acima do editor de
   SQL.
3. Escolha o **servidor** que serve de molde. A janela descobre os bancos dele em background — espere
   a barra de status dizer `'<servidor>' has N database(s).`.
4. Escolha o **banco**.
5. Clique em **`⟳`**.
6. Espere o rótulo virar `carregado (<banco>)` e a barra de status dizer
   `Schema of '<banco>' loaded: N relation(s), M function(s).`

Agora o editor de SQL tem sugestões. Elas valem para o **banco escolhido**, não para os outros do
mesmo servidor.

## Usar

Digite normalmente: a lista abre na **primeira letra** de uma palavra e também no **começo de um
token** (logo depois de `FROM `).

| Para… | Faça |
|---|---|
| escolher uma sugestão | `↑`/`↓` e `Tab` (ou `Enter`), ou clique na linha |
| pedir a lista onde ela não abriu | `Ctrl+Space` |
| fechar a lista | `Esc` |
| ver o tipo de uma coluna ou a assinatura de uma função | olhe o texto fraco à direita da linha |

## Recarregar depois de mudar o banco

O retrato é **do momento em que você clicou `⟳`**. Se alguém criou, renomeou ou removeu tabelas
nesse banco (por fora da janela, ou por outro cliente), clique em `⟳` de novo.

## Se nada aparecer

Na ordem em que vale conferir:

1. **O esquema está carregado?** O rótulo precisa dizer `carregado (<banco>)`. Se disser
   `carregado outro banco`, o par servidor/banco atual não é o que foi carregado — clique `⟳`.
2. **O `⟳` está desabilitado?** O tooltip diz o motivo: falta servidor ou banco escolhido, ou **senha
   no cofre** — resolva em [Guardar e trocar a senha](senhas-no-cofre.md). A janela não pede senha.
3. **O combo de banco está vazio?** Significa que a descoberta não deu e o cadastro também não tem
   bancos (`nenhum banco no cadastro`). Confira a conexão com
   `mustel settings db-servers test -n <servidor>`; com a conexão boa, a lista aparece sozinha.
4. **O cursor está dentro de uma string ou de um comentário?** Ali não se sugere nada, de propósito.
5. **A palavra é vazia e você apertou `Esc` antes?** O `Esc` fecha a lista pela *palavra*; digite uma
   letra que ela volta.
6. **Trocou de servidor?** A troca limpa o banco escolhido; escolha o banco de novo e clique `⟳`.

## O que não sai daqui

A janela sugere **nomes**; ela não gera `CREATE TABLE` nem altera o banco. Para exportar a estrutura,
use `pg_dump --schema-only`.

Detalhe de cada item desta página: [referência do autocomplete](../reference/autocomplete.md).
