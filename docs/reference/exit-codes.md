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

### Cancelamento (`Ctrl+C`)

O Mustel instala um listener de `Ctrl+C` que cancela um token compartilhado; o comando em andamento
observa esse token e encerra com **130**.

| Situação | Comportamento |
|---|---|
| `Ctrl+C` durante `query run` | cancela **todos** os bancos em andamento, em todos os servidores |
| `Ctrl+C` duas vezes | mesmo efeito da primeira; não trava nem duplica o cancelamento |
| banco interrompido no meio | **não** entra no `_erros.csv` (o produto anterior também não registrava o interrompido) |
| `Ctrl+C` durante uma nova tentativa | não inicia outra tentativa |

O que já foi gravado **pode** ficar nos arquivos por servidor: o cancelamento não apaga arquivo já
aberto. Medido em dois cenários, porém, o que se vê é diferente: numa consulta que ainda não emitiu
linhas (o caso comum de `pg_sleep` ou de um `SELECT` longo), **nenhum arquivo é criado** e o
comando termina com 130 e `Error: operation cancelled`.

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
