# Referência: exit codes e variáveis de ambiente

## Exit codes

| Código | Significa |
|---|---|
| `0` | sucesso |
| `1` | erro geral (inclui erro de uso: comando/opção desconhecida, argumento obrigatório ausente) |
| `2` | validação de comando (`database backup`, `database restore`, `database pgpass add`) |
| `10` | falha de Postgres **com** SQLSTATE |
| `11` | falha de Postgres **sem** SQLSTATE (rede, servidor fora do ar, timeout de conexão) |
| `20` | erro de I/O |
| `21` | diretório não encontrado |
| `22` | arquivo não encontrado |
| `30` | acesso negado |
| `40` | estado inválido (inclui prompt sem terminal e senha ausente) |
| `41` | argumento inválido (validação do cadastro) |
| `50` | tempo esgotado |
| `130` | cancelado pelo usuário (`Ctrl+C`) |

Erro de uso escreve em **stdout**, não em stderr.

### Quirks herdados

| Situação | Código |
|---|---|
| `pg_dump` ausente no `database backup` | `3` |
| `pg_restore` ausente no `database restore` | `1` |

## Variáveis de ambiente

| Variável | Efeito |
|---|---|
| `MUSTEL_CONFIG_DIR` | diretório do `mustel.jsonc` (isolamento em teste) |
| `MUSTEL_PGPASS_FILE` | caminho do `pgpass.conf` usado pelo Mustel |
| `MUSTEL_KEYRING_TESTS` | `1` habilita os testes que escrevem no cofre real |
| `MUSTEL_PG_TEST_URL` | URL do Postgres para a suíte de integração |
| `MUSTEL_BIN` | binário usado pelo `oracle-compare` (diagnóstico) |

## Caminhos

| O quê | Windows | Linux |
|---|---|---|
| `mustel.jsonc` | `%APPDATA%\Mustel\mustel.jsonc` | `$XDG_CONFIG_HOME/Mustel/mustel.jsonc` (ou `~/.config/Mustel/`) |
| `pgpass.conf` | `%APPDATA%\postgresql\pgpass.conf` | `$XDG_CONFIG_HOME/postgresql/pgpass.conf` (ou `~/.config/postgresql/`) |

O caminho do `pgpass.conf` **não** é a convenção do `libpq` (`~/.pgpass`): é o que
`SpecialFolder.ApplicationData` devolvia no produto anterior. O Mustel mantém isso por paridade.
