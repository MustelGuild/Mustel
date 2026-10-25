# Referência: gerar SQL com IA

> O painel **AI** da aba **Query** conversa com um provedor OpenAI-compatible para escrever o SQL do
> banco escolhido. Ele **nunca executa**: entrega texto no editor e quem roda é o `Run query`. O
> porquê está em [Por que a IA escreve e o humano roda](../explanation/ia.md); o passo a passo em
> [Gerar SQL com a IA](../how-to/gerar-sql-com-ia.md).

## Como abrir

O painel **`AI Generate SQL`** (ícone `sparkles` no título) fica à direita da janela, com 360 px
fixos, e só existe na aba Query. O ícone `sparkles` no canto do painel **esconde**; fechado, o botão
`AI Generate SQL` aparece no alto da aba para trazê-lo de volta.

## O cabeçalho

| Campo | O que é |
|---|---|
| `Database` | servidor e banco da conversa (combos com busca, como os outros campos de escolha) |
| `Profile` | qual provedor/configuração usar (lista do `gui.ai`) |
| `DDL <tamanho> · <data>` | o schema em mãos e **quando** ele foi baixado; `Refresh` baixa de novo |

O DDL é **lido do catálogo do próprio servidor** (tabelas, colunas, tipos, `NOT NULL`, defaults,
chaves primária/estrangeira/única, índices e views) e fica em cache em `%APPDATA%/mustel/ai/ddl/`. Sem
cache, o painel pede `Load schema`.

> Não usa `pg_dump`: o binário depende da máquina e **recusa servidor mais novo que ele**
> (`server version 18.3; pg_dump version 16.6`). Pelo catálogo, funciona em qualquer versão de servidor
> que o app já consiga consultar.

O schema vai **inteiro** em cada pedido (sem truncar). Quando passa de **200.000 caracteres**, o
cabeçalho mostra `Large schema (<tamanho> KB): the whole schema is sent with every message…`: é um
aviso, não um bloqueio — o provedor pode recusar o pedido por tamanho de contexto ou cobrar mais.
O texto continua indo completo; para reduzir, use um banco com menos relações.

## A conversa

O campo de envio aceita texto livre ("vendas por mês"). Cada resposta vem com o SQL realçado e
duas ações:

| Ação | O que faz |
|---|---|
| `Insert in editor` | joga o SQL no editor da aba Query, **substituindo** todo o conteúdo |
| `Replace editor text` | o mesmo botão, com o nome trocado quando o editor **já tem** texto |
| `Copy` | copia o SQL para a área de transferência |

Depois de responder, o campo vira `Ask for a change…`: escrever ali e enviar **continua** a mesma
conversa, com o histórico inteiro indo junto em cada pedido.

## Contexto pinado

O par servidor/banco é **pinado** quando a conversa começa. Trocar os combos **não** muda o schema
dos próximos pedidos — aparece um aviso com duas saídas:

- `Start new session` — apaga a conversa e começa de novo com o banco escolhido;
- `Keep talking` — só dispensa o aviso; os pedidos seguem com o schema da sessão.

## Onde fica gravado

A conversa **persiste entre reinícios** em `%APPDATA%/mustel/ai/session.json`:

```jsonc
{
  "server": "Local",
  "database": "configurador",
  "profileId": "rapido",
  "panelOpen": true,
  "messages": [
    { "role": "user", "content": "vendas por mês" },
    { "role": "assistant", "content": "SELECT …" }
  ]
}
```

O arquivo guarda **só** a conversa e o contexto: o DDL **não** é copiado para lá (ele já está no
cache) e nenhuma chave aparece nele. Apagar o arquivo é o mesmo que começar do zero.

## O que a IA não faz

- **não executa** nada — nem "rodar depois de gerar";
- não recebe dados das suas tabelas, só a **estrutura**;
- não guarda histórico de várias conversas (existe uma sessão por vez);
- não fala com o Azure OpenAI (que usa `api-key` e deployment no path).
