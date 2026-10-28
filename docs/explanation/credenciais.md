# Credenciais no Mustel

## Onde as senhas ficam

No **cofre do sistema operacional**, via crate `keyring`:

| SO | Backend |
|----|---------|
| Windows | Credential Manager |
| macOS | Keychain |
| Linux | Secret Service (GNOME Keyring / KWallet) |

Serviço: `mustel`. Usuário (chave): o **nome do servidor** cadastrado em `mustel.jsonc`.

O store do `keyring` 4 fala D-Bus por `zbus`, em Rust puro (decisão `B1` em
[Decisões de engenharia](decisoes.md)) — é o que dispensa `libdbus-1-dev` no build e
`libdbus-1.so.3` em runtime. O **serviço** continua sendo o do sistema: sem uma sessão com Secret
Service o cofre não responde.

```bash
# grava/atualiza a senha de um servidor já cadastrado
mustel settings db-servers set-password dev

# ou, ao cadastrar
mustel settings db-servers add -n dev -H localhost -U postgres -W 'minha senha'
```

## O que **não** é feito

- **Nenhuma senha é escrita em arquivo.** O `mustel.jsonc` guarda apenas o cadastro do servidor
  (nome, host, porta, usuário, bancos).
- **O blob DPAPI do FurLab (`encryptedPassword`) não é migrado.** Ao migrar, cadastre a senha de novo
  com `set-password`; não há como o Mustel decifrar o que o .NET gravou.
- Remover um servidor (`settings db-servers rm`) também remove a senha correspondente do cofre.

## Quando não há cofre disponível

Em Linux sem sessão desktop (sem Secret Service), o cofre não responde. Nesse caso o Mustel:

1. falha com mensagem explícita (`credential store unavailable...`) em vez de gravar em arquivo;
2. mantém o caminho alternativo de **informar a senha por execução**, que já faz parte da interface:
   `-W/--password` em `query run`, `database backup` e `database restore`.

Exemplo de uso em servidor/CI (sem cofre):

```bash
mustel query run -i consulta.sql -H localhost -U postgres -W "$PGPASSWORD"
```

## Verificação

O round-trip contra o cofre real é um teste ignorado por padrão (para não escrever no cofre de quem
roda a suíte). Para exercitá-lo conscientemente:

```bash
MUSTEL_KEYRING_TESTS=1 cargo test -p mustel-core -- --ignored
```
