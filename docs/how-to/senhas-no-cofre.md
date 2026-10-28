# Guardar e trocar a senha de um servidor

**Situação:** você cadastrou um servidor, ou precisa trocar a senha dele, sem deixá-la em arquivo.

## Guardar a senha

```bash
mustel settings db-servers set-password dev
```

O Mustel pede a senha sem eco e grava no **cofre do sistema operacional**, com o serviço `mustel` e a
chave igual ao nome do servidor.

Em automação, sem prompt:

```bash
mustel settings db-servers set-password dev -W "$SENHA"
```

Para não deixar a senha em `argv`:

```bash
printf '%s\n' "$SENHA" | mustel settings db-servers set-password dev --password-stdin
```

## Guardar junto com o cadastro

```bash
mustel settings db-servers add -n dev -H localhost -U postgres -W "$SENHA"
```

Ou, sem a senha no histórico:

```bash
printf '%s\n' "$SENHA" | mustel settings db-servers add -n dev -H localhost -U postgres --password-stdin
```

O `-W` do `add` grava no cofre, não no `mustel.jsonc`. Abra o arquivo e confira: não há campo de senha.

## Trocar a senha

Rodar `set-password` de novo sobrescreve o segredo. Não há comando separado de “alterar”.

## Servidor renomeado?

O CLI não tem `rename`. No terminal, remover e cadastrar de novo **apaga o segredo** (a chave é o
nome). Se você fez isso e a senha sumiu, basta rodar `set-password` com o nome novo.

## Conferir que a senha não vazou

```bash
mustel settings db-servers ls          # a tabela não tem coluna de senha
cat "$APPDATA/Mustel/mustel.jsonc"     # nenhum campo de senha no arquivo
```

## Sem cofre disponível (Linux headless)

Sem Secret Service, o cofre não responde. O Mustel falha com mensagem explícita em vez de gravar em
arquivo. O contorno é passar a senha por execução:

```bash
mustel query run -i consulta.sql -H localhost -U postgres -W "$PGPASSWORD"
```

Mais contexto em [Credenciais](../explanation/credenciais.md).
