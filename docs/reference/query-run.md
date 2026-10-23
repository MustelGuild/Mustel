# Referência: `query run`

## Escolha do SQL

`-i/--input` e `-c/--command` são **mutuamente exclusivos**; sem nenhum dos dois o comando falha
antes de tocar o banco. Caminhos com traversal (`../`) são recusados.

## Escolha dos alvos

1. `--npgsql-connection-string` → conexão ad-hoc, sem cadastro.
2. Um servidor no cadastro → usa ele, sem perguntar.
3. Dois ou mais → seleção interativa, **todos pré-selecionados**.

Dentro de cada servidor:

- `--all` ou `fetchAllDatabases` no cadastro → descobre os bancos acessíveis;
- senão → usa a lista `databases` do cadastro.
- `--exclude` remove por **nome exato** (sem curinga), ignorando maiúsculas.
- `excludePatterns` do cadastro remove por curinga `*`.

## Precedência da senha

`-W` → cofre (chave = nome do servidor) → prompt. No máximo **um** prompt por servidor, não por banco.

## Confirmação

Consulta classificada como destrutiva pede confirmação **se**: `--no-confirm` não foi passado **e**
`requireConfirmation` está ligado no cadastro. Leitura nunca pede.

Reconhecidas como destrutivas: `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`, `CREATE`, `TRUNCATE`,
`MERGE`, `GRANT`, `REVOKE`, `SET ROLE` — inclusive dentro de CTE que escreve.

## Arquivos

Cada execução cria uma pasta `<saída>/<timestamp>/`:

| Arquivo | Conteúdo |
|---|---|
| `consolidated_<ts>.csv` | todos os bancos, com `Server` e `Database` como primeiras colunas |
| `<servidor>_<ts>.csv` | só aquele servidor (nome sanitizado) |
| `<ts>_log.csv` | `Server, Database, ExecutedAt, Status, RowCount, DurationMs, Error` |
| `<ts>_erros.csv` | só quando algum banco falha |

Formato do CSV consolidado:

- **BOM UTF-8** no início;
- quebra de linha **CRLF**;
- separador `,`;
- aspas só quando o campo contém `,`, aspas ou quebra de linha.

## Como cada tipo vira texto

| Tipo | Como sai |
|---|---|
| `NULL` | campo vazio |
| `bool` | `True` / `False` |
| `float4` | com precisão de f32 |
| `float8` | como o .NET (`2`, `1500.75`) |
| `numeric` | com as casas do tipo (`0.00`) |
| `bytea` | `System.Byte[]` |
| arrays | `System.String[]` (o nome do tipo .NET) |
| `jsonb` | compacto (`{"k": 1}`) |
| `timestamptz` | `10/18/2026 10:52:15 AM` |
| `date` | `10/18/2026` |
| `time` | `10:52 AM` |
| `uuid` | minúsculo, com hífens |

Datas e números usam a convenção **en-US fixa** (não a do sistema).

## Log

`ExecutedAt` no formato round-trip do .NET (`2026-10-22T04:06:10.9549810Z`); `DurationMs` como
double. Uma linha por banco.

## Falhas

- Falha de um banco → linha de erro + `Status` no log; os outros continuam.
- Nenhum banco alcançável → para **antes** de escrever arquivos e falha com a mensagem do produto
  anterior.
