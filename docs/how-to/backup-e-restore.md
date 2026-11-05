# Fazer backup e restore de um banco

> **TLS nesta release:** `database backup` e `database restore` não usam TLS validado. Eles só
> aceitam `--ssl-mode Disable`; `Prefer` e `Require` são recusados com exit 2. Use `Disable` apenas
> em rede confiável, ou `pg_dump`/`pg_restore` diretamente com `sslmode=verify-full` e `sslrootcert`.
> `query run` e a GUI continuam com TLS validado.

**Situação:** você precisa salvar um banco em arquivo e depois colocá-lo de volta.

## Antes de tudo: a versão do cliente

`database backup` e `database restore` chamam `pg_dump` e `pg_restore` **externos**. Eles precisam
ser da **mesma major version** do servidor:

```bash
pg_dump --version    # precisa bater com a major do servidor
```

Se não bater, o comando falha com `aborting because of server version mismatch`. O Mustel procura o
binário **primeiro no `PATH`** e só depois em `C:\Program Files\PostgreSQL\<versao>\bin` — com duas
versões instaladas, a do `PATH` ganha.

## Backup

```bash
# um banco, no diretório atual
mustel database backup meu_banco -W "$SENHA" --ssl-mode Disable

# escolhendo o arquivo de saída
mustel database backup meu_banco -o ./backups -W "$SENHA" --ssl-mode Disable

# todos os bancos do servidor
mustel database backup --all -o ./backups -W "$SENHA" --ssl-mode Disable
```

O nome do arquivo e a extensão seguem o padrão do produto anterior (`<banco>_<timestamp>.dump`). Com
`--all`, uma falha num banco **avisa e continua**; o comando termina com sucesso.

## Restore

```bash
# banco existente, arquivo explícito
mustel database restore meu_banco ./backups/meu_banco_2026-10-22.dump -W "$SENHA" --ssl-mode Disable

# todos os dumps de um diretório
mustel database restore --all -d ./backups -W "$SENHA" --ssl-mode Disable
```

O `restore` usa `--clean` por padrão (derruba os objetos antes de recriar). Flags úteis:

| Flag | Efeito |
|---|---|
| `--no-clean` | não derruba os objetos antes |
| `--no-owner` / `--no-acl` | não restaura dono nem permissões |
| `--single-transaction` | tudo ou nada |
| `-j N` | restauração paralela (só formato custom) |

## Banco que ainda não existe

Restaurar num banco inexistente **falha** — é o comportamento herdado. Para criar na hora:

```bash
mustel database restore banco_novo ./backups/banco_novo.dump --create-if-not-exists --ssl-mode Disable -W "$SENHA"
```

> `--create-if-not-exists` é adição do Mustel. O binário anterior tentava criar o banco por `psql`
> num passo que nunca disparava, então restaurar para banco novo sempre falhava.

## Binário ausente

Se `pg_dump`/`pg_restore` não estiverem instalados, o comportamento varia **de propósito**, herdado do
produto anterior:

| Comando | Exit code |
|---|---|
| `database backup` | `3` |
| `database restore` | `1` |
