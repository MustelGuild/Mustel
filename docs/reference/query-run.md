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

`--password-stdin` → `-W` → `MUSTEL_PASSWORD` → cofre (chave = nome do servidor) → prompt. No
máximo **um** prompt por servidor, não por banco. `--password-stdin` e `-W` são exclusivos; `-W`
num terminal avisa que a senha fica visível no `ps` e no histórico.

## TLS

`--ssl-mode` aceita `Disable`, `Prefer` (default) e `Require`. Quando há TLS, o certificado é
validado por cadeia **e** hostname. `Prefer` só cai para texto puro quando o servidor não oferece
TLS; `Require` nunca cai. `Allow`, `VerifyCA` e `VerifyFull` foram removidos e são recusados com
mensagem de migração. A página [TLS do PostgreSQL](../explanation/tls-postgres.md) explica a
matriz e o caso de certificado self-signed.

## Confirmação

Consulta classificada como destrutiva pede confirmação **se**: `--no-confirm` não foi passado **e**
`requireConfirmation` está ligado no cadastro. Leitura nunca pede.

Reconhecidas como destrutivas: `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`, `CREATE`, `TRUNCATE`,
`MERGE`, `GRANT`, `REVOKE`, `SET ROLE` — inclusive dentro de **qualquer** CTE e em
`EXPLAIN ANALYZE` de DML. `EXPLAIN` sem `ANALYZE` continua seguro. O analisador usa `sqlparser`;
se o parse falhar, procura as palavras destrutivas fora de strings/comentários.

A página [Por que o classificador é assim](../explanation/classificador-de-query.md) explica os
bypasses, o fallback e a mudança de descrição.

## Arquivos

Cada execução cria uma pasta `<saída>/<timestamp>/`:

| Arquivo | Conteúdo |
|---|---|
| `consolidated_<ts>.csv` | todos os bancos, com `Server` e `Database` como primeiras colunas |
| `<servidor>_<ts>.csv` | só aquele servidor (nome sanitizado) |
| `<ts>_log.csv` | `Server, Database, ExecutedAt, Status, RowCount, DurationMs, Error` |
| `<ts>_erros.csv` | só quando algum banco falha |

Por padrão o CSV reproduz os bytes do legado. Com `--csv-escape-formulas`, todo campo que começa
com `=`, `+`, `-`, `@`, TAB ou CR ganha um `'` na frente (no cabeçalho e nas linhas), para o
Excel/LibreOffice não avaliar o conteúdo como fórmula. Default: desligado.

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

## Nova tentativa (retry)

Uma consulta que falha é tentada de novo até **3 vezes**, com espera exponencial de
**1 s, 2 s e 4 s** entre as tentativas (`500 ms · 2^n`, com `n` = 1, 2, 3).

Só retenta quando **as duas** condições valem:

1. o erro é de Postgres (`CoreError::Postgres`) ou **timeout** — erro de configuração, validação ou I/O
   não é retentado;
2. **nenhuma linha** daquela consulta já foi gravada. Uma consulta que começou a escrever e falhou no
   meio **não** é retentada, porque retentar duplicaria dados no arquivo daquele banco.

Esgotadas as tentativas, o banco entra no `_erros.csv` e o log recebe `Status=Error`, `RowCount=0` e
`DurationMs=0` — o valor que o produto anterior gravava no caminho de erro.

## Cancelamento (`Ctrl+C`)

`Ctrl+C` cancela **todas** as consultas em andamento — em todos os servidores e bancos — e o processo
termina com **130**.

- Nenhuma consulta nova começa depois do sinal.
- Um banco cancelado no meio **não** entra no `_erros.csv`: o produto anterior também não registrava
  o que foi interrompido. Medido: com `Ctrl+C` durante uma consulta longa, saem 130 e
  `Error: operation cancelled`, e **nenhum arquivo é criado** — a tabela ao vivo também não sobrevive
  ao cancelamento.
- Pressionar `Ctrl+C` mais de uma vez tem o mesmo efeito da primeira; não há execução duplicada de
  cancelamento nem travamento.

## Escrita conforme chega

As linhas vão para o arquivo do servidor **em blocos de 64**, à medida que o driver as entrega — não
há materialização do resultado inteiro em memória.

Duas consequências que valem saber:

- Uma consulta que emite linhas e falha **depois** deixa no arquivo daquele banco as linhas já
  gravadas; ela não é retentada (ver acima).
- Dois bancos do **mesmo** servidor escrevem no mesmo arquivo e podem **intercalar** blocos. A ordem
  das linhas de bancos diferentes não é determinística — quem termina primeiro escreve primeiro.

O arquivo consolidado é montado no fim, a partir dos arquivos por servidor.
