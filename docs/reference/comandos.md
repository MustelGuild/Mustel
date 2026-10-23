# Referência: árvore de comandos

Esta página descreve **o que existe**, não como usar. Para aprender, comece pelo
[tutorial](../tutorials/primeiros-passos.md); para resolver um problema, veja os
[how-tos](../how-to/).

## Raiz

```text
mustel [COMMAND]
```

Sem subcomando: com terminal interativo abre a GUI; **sem** terminal imprime a ajuda e sai **0**.

| Comando | O que faz |
|---|---|
| `query run` | executa SQL e exporta CSV |
| `database backup` | backup com `pg_dump` |
| `database restore` | restore com `pg_restore` |
| `database pgpass add` / `list` / `remove` | gerencia o `pgpass.conf` |
| `settings db-servers ls` / `add` / `rm` / `test` / `set-password` | cadastro e cofre |
| `gui` | abre a janela |

Opções globais: `-h, --help` e `-V, --version`.

## `query run`

Detalhe completo em [query run](query-run.md).

| Flag | Default | O que faz |
|---|---|---|
| `-i, --input <file>` | — | arquivo SQL; exclusivo com `-c` |
| `-c, --command <sql>` | — | SQL inline; exclusivo com `-i` |
| `-o, --output <path>` | `./results` | diretório de saída |
| `--npgsql-connection-string <s>` | — | conexão ad-hoc no formato do Npgsql; dispensa o cadastro |
| `-H, --host` / `-p, --port` / `-d, --database` / `-U, --username` / `-W, --password` | do cadastro | overrides de conexão |
| `--ssl-mode <mode>` | `Prefer` | `Disable`, `Allow`, `Prefer`, `Require`, `VerifyCA`, `VerifyFull` |
| `--timeout <s>` | `30` | timeout de conexão |
| `--command-timeout <s>` | `300` | timeout da consulta |
| `--pooling [<bool>]` | `true` | aceito; **sem efeito** (não há pool) |
| `--min-pool-size` / `--max-pool-size` | `1` / `100` | aceitos; **sem efeito** |
| `--keepalive <s>` | `0` | keepalive do socket |
| `--connection-lifetime <s>` | `0` | aceito; **sem efeito** |
| `-a, --all` | desligado | descobre e usa todos os bancos |
| `--exclude <nomes>` | — | bancos a excluir, por **nome exato** |
| `--no-confirm` | desligado | não pede confirmação em consulta destrutiva |

## `database backup`

| Flag | O que faz |
|---|---|
| `[database]` (posicional) | banco a salvar; dispensável com `--all` |
| `-H` / `-p` / `-U` / `-W` | conexão |
| `--ssl-mode` / `--timeout` / `--command-timeout` | aceitos por compatibilidade; **não** vão para o `pg_dump` |
| `-a, --all` | todos os bancos do servidor |
| `--exclude <padrão>` | tabelas a excluir inteiras (repetível) |
| `--exclude-table-data <padrão>` | excluir só os dados (repetível) |
| `-o, --output <path>` | arquivo ou diretório |
| `-F, --format <fmt>` | formato do `pg_dump` |
| `-v, --verbose` | saída verbosa do `pg_dump` |

## `database restore`

| Flag | O que faz |
|---|---|
| `[database]` `[file]` | banco e dump; o arquivo default é `<database>.dump` |
| `-H` / `-p` / `-U` / `-W` | conexão |
| `--ssl-mode` / `--timeout` / `--command-timeout` | conexão e limites |
| `-a, --all` | todos os dumps do diretório |
| `-d, --directory <path>` | diretório dos dumps (com `--all`) |
| `--no-clean` | não derruba objetos antes (o default derruba) |
| `--no-owner` / `--no-acl` | não restaura dono / permissões |
| `--single-transaction` | tudo ou nada |
| `-j, --jobs <n>` | paralelismo (só formato custom) |
| `-v, --verbose` | saída verbosa |
| `--create-if-not-exists` | cria o banco alvo quando ele não existe |

## `database pgpass`

| Comando | Argumentos |
|---|---|
| `add` | `[database]` **ou** `--database`; `--host`, `--port`, `--username`, `--password`; `*` é curinga |
| `list` | nenhum |
| `remove` | `<database>` obrigatório; `--host`, `--port`, `--username` |

Default dos campos: `localhost`, `5432`, `postgres`.

### O curinga `*`

Cada um dos campos `host`, `port` e `username` aceita `*`, que significa "qualquer valor" na
resolução do `pgpass.conf`. O banco **não** é validado como curinga — ele é a chave do `remove`.

O `*` é aceito **sozinho** no campo. Ele não é um padrão: `*` não casa com `postgres`, e
`loc*host` não é curinga nenhum (é host inválido).

### O que é validado

| Campo | Aceita | Recusa |
|---|---|---|
| `host` | nome DNS, `localhost`, IPv4, IPv6, `*` | espaço (`"a b"`), caractere fora de letra/dígito/hífen/ponto, vazio, mais de 253 |
| `port` | número de **1 a 65535**, `*` | `0`, `65536`, texto, espaço |
| `username` | qualquer texto (inclusive com espaço) e `*` | vazio |
| `database` | qualquer nome não vazio (a chave) | vazio |

O `pgpass add` valida **só host e porta** — é o que o produto anterior fazia. `username` não passa
por validação, então `--username 'a b'` é gravado como veio (verificado: exit 0). Já o
`settings db-servers add`, que cadastra no `mustel.jsonc`, **valida** o usuário.

Um campo inválido no `add` sai com **exit 2** e a mensagem do produto anterior — não grava nada no
arquivo.

## `settings db-servers`

| Comando | Argumentos |
|---|---|
| `ls` | nenhum |
| `add` | `-n <nome>`, `-i`, `-H`, `-p`, `-U`, `-W`, `-d <bancos>`, `--ssl-mode`, `--timeout`, `--max-parallelism`, `--fetch-all`, `--exclude-patterns` |
| `rm` | `-n <nome>` |
| `test` | `-n <nome>` |
| `set-password` | `[nome]`; `-W` para não usar prompt |
