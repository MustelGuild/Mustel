# TLS do PostgreSQL

O Mustel usa o `tokio-postgres` com o conector `tokio-postgres-rustls`. Quando a conexão usa TLS,
o certificado do servidor é validado com o verificador padrão do `rustls`: **cadeia de confiança e
hostname**. Não existe modo que aceite qualquer certificado.

## Os três modos

O `sslMode` acompanha os três valores do driver:

| Modo | TLS | Valida o certificado | Cai para texto puro? |
|---|---|---|---|
| `Disable` | não | — | já é texto puro |
| `Prefer` | se o servidor oferecer | sim | sim, **somente** quando o servidor diz que não oferece TLS |
| `Require` | obrigatório | sim | nunca |

`Prefer` é o default. Se o servidor oferece TLS mas o certificado é inválido (CA desconhecida,
hostname errado, certificado expirado), a conexão **falha**; ela não cai para texto puro. O driver
só faz o fallback quando recebe a resposta "sem TLS" do servidor.

`Require` falha se o servidor não oferecer TLS, se o certificado não for confiável ou se o hostname
não casar.

## De onde vem a confiança

O `RootCertStore` é montado com `rustls-native-certs`, que lê o store do sistema operacional. Um
certificado self-signed ou assinado por uma CA privada só é aceito depois que essa CA é instalada no
SO. Em containers, isso significa instalar o certificado na imagem e rodar `update-ca-certificates`
(ou o equivalente da imagem).

Não há `sslRootCert`/`Root Certificate` nesta versão: um arquivo de CA por conexão é um plano
separado. Também não há `trustServerCertificate`: desligar a validação não é uma opção.

## Migração dos nomes antigos

O produto anterior aceitava seis nomes do Npgsql. O Mustel passou a aceitar três, para coincidir
com o driver e não manter verificadores próprios:

| Antes | Depois |
|---|---|
| `VerifyFull` | `Require` (mesma validação de cadeia + hostname) |
| `VerifyCA` | não tem equivalente exato; use `Require` com o hostname correto |
| `Allow` | não tem equivalente; use `Prefer` se TLS-first for aceitável, ou `Disable` se texto puro for intencional |
| `trustServerCertificate` | removido |

`Allow`, `VerifyCA`, `VerifyFull` e qualquer valor desconhecido são recusados com mensagem de
migração, tanto na flag quanto no `mustel.jsonc` e na GUI. A flag `query run --ssl-mode` inválida
não cai mais em `Prefer`.

## O que observar no smoke

O ambiente de `smoke/` sobe o PostgreSQL 18.6 com um certificado próprio. Para os checks de TLS:

- `Disable` deve mostrar `pg_stat_ssl.ssl = f`;
- `Prefer` e `Require`, com a CA do servidor no store do runner, devem mostrar `t`;
- `Require` sem a CA deve falhar;
- `Prefer` sem a CA também deve falhar, sem cair para texto puro;
- `Prefer` contra um serviço sem TLS deve mostrar `f` e emitir o aviso;
- `Require` contra um serviço sem TLS deve falhar.

A matriz é manual; não há workflow de CI para o smoke.

## Por que rustls

`rustls` é puro Rust (com o provedor `aws-lc-rs`), não depende do OpenSSL do sistema e deixa a
política de validação explícita no `ClientConfig`. O `tokio-postgres` cuida do protocolo e do
fallback do `Prefer`; o `tokio-postgres-rustls` faz a ponte entre os dois.

## Referência

- [`query run`](../reference/query-run.md)
- [`database`](../reference/comandos.md)
- [`--help` do CLI](../reference/comandos.md)
