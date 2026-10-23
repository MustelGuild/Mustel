# Usar em automação (CI, scripts, cron)

**Situação:** você quer rodar o Mustel sem ninguém na frente do teclado.

## Regra de ouro

Sem terminal, **o Mustel nunca fica esperando resposta**. Todo prompt falha com mensagem clara e um
exit code; quem chama decide o que fazer. Se um comando travar num script, é bug.

## O que você precisa fornecer

| Prompt que apareceria | Como evitar |
|---|---|
| Senha do servidor | `-W "$SENHA"`, ou a senha no cofre via `set-password`, ou `MUSTEL_PGPASS_FILE` |
| Seleção de servidores | `--npgsql-connection-string`, ou deixar **um** servidor no cadastro (com um só, não pergunta), ou `--all` |
| Confirmação de consulta destrutiva | `--no-confirm` |
| Nome do banco no `database backup` | passar o posicional, ou `--all` |

## Exemplo de pipeline

```bash
set -euo pipefail

mustel query run -i consulta.sql --all --no-confirm -W "$PGPASSWORD" -o ./saida

# o consolidado é o único arquivo que interessa para o passo seguinte
csv=$(find ./saida -name 'consolidated_*.csv' | sort | tail -1)
wc -l "$csv"
```

## Exit codes que importam

| Código | Significa | Ação típica |
|---|---|---|
| `0` | sucesso | seguir |
| `1` | erro geral / erro de uso | falhar o job |
| `2` | validação de comando (`backup`/`restore`/`pgpass add`) | corrigir os argumentos |
| `10` | Postgres **com** SQLSTATE | erro do banco (permissão, SQL inválido) |
| `11` | Postgres **sem** SQLSTATE | rede, servidor fora do ar |
| `40`/`41` | estado inválido / argumento inválido | corrigir o cadastro |
| `50` | timeout | subir o limite |
| `130` | cancelado | — |

A lista completa está na [referência de exit codes](../reference/exit-codes.md).

## Cuidado com o `pg_dump`

`database backup` e `database restore` chamam os binários **externos** `pg_dump`/`pg_restore`. Eles
precisam ser da **mesma major version** do servidor; caso contrário o comando falha com
`server version mismatch`. Confira no runner:

```bash
pg_dump --version
psql --version
```

> O Mustel procura o binário **primeiro no `PATH`** e só depois em `C:\Program Files\PostgreSQL\<versao>\bin`.
> Se houver duas versões instaladas, a do `PATH` ganha — e pode ser a errada.
