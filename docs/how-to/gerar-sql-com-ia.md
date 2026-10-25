# Gerar SQL com a IA

Você sabe o que quer consultar, mas não lembra onde está a coluna. Este guia liga a IA uma vez e
mostra o ciclo: descrever, ler a resposta, pedir ajuste, mandar para o editor.

## 1. Configure um perfil (uma vez)

Abra **Settings › AI** e clique em `Add profile`:

| Campo | Exemplo |
|---|---|
| `Name` | `Rápido` |
| `Base URL` | `https://api.openai.com/v1` (ou o `/v1` do Ollama, OpenRouter, Groq…) |
| `Model` | `gpt-4o-mini` |
| `Temperature` | `0.10` (o padrão; suba só se o modelo errar muito) |
| `API key` | a chave do provedor |

`Save profile` grava o perfil e guarda a chave no **cofre do sistema** — ela não vai para o
`mustel.jsonc`. Use `Test` para confirmar que endpoint, modelo e chave conversam.

## 2. Abra o painel

Na aba **Query**, clique em **`AI Generate SQL`** (no alto da tela). O painel abre à direita; o
ícone `sparkles` no canto dele esconde, e o mesmo botão do alto traz de volta.

## 3. Escolha o banco e carregue o schema

Em `Database`, escolha servidor e banco. Se ainda não há schema em cache, clique em `Load schema`.
Quando terminar, aparece `DDL <tamanho> · <data>`.

> O schema é a **estrutura** (tabelas, tipos, chaves, índices e views), nunca os dados das suas
tabelas.

## 4. Descreva o que precisa

Escreva no campo de envio e aperte `Send`:

```
total de vendas por mês, só do último ano
```

A resposta vem com o SQL realçado.

## 5. Peça ajustes

Não gostou? Escreva no mesmo campo o que mudar:

```
agrupa por cliente em vez de mês
```

Cada pedido leva o histórico inteiro (e o schema), então a conversa mantém o contexto.

## 6. Mande para o editor

`Insert in editor` põe o SQL na aba Query. Se o editor **já tinha** texto, o botão se chama
`Replace editor text` e **substitui tudo** — o anterior não volta.

`Copy` copia o SQL para a área de transferência, se você quiser colar em outro lugar.

## 7. Revise e rode

Leia o SQL. A IA **não executou nada**: quem roda é o `Run query`, e é ali que você confere o escopo
(quais bases estão marcadas na sidebar) antes de apertar.

## Trocar de banco no meio da conversa

Se você mudar o servidor ou o banco com uma conversa aberta, aparece um aviso:

- `Start new session` — começa do zero com o banco novo (e carrega o schema dele);
- `Keep talking` — segue falando do banco antigo, que é o schema que o modelo conhece.

## Quando algo dá errado

| Mensagem | O que fazer |
|---|---|
| `The AI provider rejected the key (401)` | refaça a chave no perfil, em Settings › AI |
| `does not know this endpoint or model (404)` | confira `Base URL` (termina em `/v1`?) e `Model` |
| `rate limiting (429)` | espere e envie de novo |
| `the schema may be too large` | banco muito grande para o contexto do modelo; use um perfil com janela maior |
| `No key stored for …` | o perfil existe, mas a chave não foi gravada |

A conversa continua na tela depois de um erro: corrigir e enviar de novo não perde o que já foi
discutido.
