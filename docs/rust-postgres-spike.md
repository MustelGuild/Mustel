# Camada PostgreSQL do Mustel — decisão e estado

> Decisão **D12**: usar **`tokio-postgres`** (escolha do usuário, 2026-10-15). A alternativa `sqlx` foi
> descartada e o spike deixou de ser comparação para ser implementação com evidência.
> Estado: **parcial** — ver "O que falta".

## Por que `tokio-postgres`

- **Driver, não framework**: dá acesso direto a `Row`/`Column`/`Type`, que é o que o export para CSV
  precisa para mapear tipos arbitrários. `sqlx` empurra para tipos conhecidos em tempo de compilação,
  e o `query run` executa SQL livre — não há como conhecer os tipos antes.
- **Sem `query!` compilado**: o legado aceita qualquer SQL, então macros que exigem introspecção de
  banco não servem.
- **Fronteira pequena**: a API assíncrona casa com o `tokio` já usado para processos e cancelamento.

## O que está pronto (verificado por teste)

| Peça | Onde | Cobertura |
|---|---|---|
| Tradução do `sslMode` do Npgsql (6 modos) | `postgres/tls.rs` | 3 testes: nomes, rejeição de inválido, semântica de `requires_tls`/`tries_tls`/`verifies_hostname` |
| Configuração de conexão com os defaults do legado | `postgres/settings.rs` | 4 testes: defaults (`5432`, `30`, `300`, `1`, `100`, `Prefer`), parsing do modo, `endpoint()`, `Config` do driver (host/porta/usuário/senha/banco) |
| Formatação de célula para CSV | `postgres/value.rs` | 5 testes: `NULL` → campo vazio, escalares, `float` com `.0`, listas `{1,2,três}`, `bytea` em hex |
| Conexão sem TLS + classificação de erro | `postgres/client.rs` | 1 teste: modos com TLS falham com mensagem clara em vez de degradar |

Os erros de conexão viram `CoreError::Postgres` **com** `sqlstate` quando o servidor informa um, que é
o que separa o exit `10` do `11` no oráculo.

## O que falta (bloqueios e pendências)

1. **TLS não está ligada.** `Require`, `VerifyCA` e `VerifyFull` devolvem erro explícito pedindo para
   usar `Prefer`/`Allow`/`Disable`. Ligar `tokio-postgres-rustls` + certificados do sistema é o
   próximo passo; `VerifyCA` exige um `ServerCertVerifier` próprio, porque o rustls valida hostname
   sempre e o Npgsql **não** valida em `VerifyCA`.
2. **Testes de integração não executados.** O Docker Desktop está parado nesta máquina, então o
   Postgres de teste não subiu. O teste está escrito e gateado por `MUSTEL_PG_TEST_URL`
   (`crates/mustel-core/tests/postgres_integration.rs`); com um banco no ar,
   `MUSTEL_PG_TEST_URL=postgres://postgres:dev@localhost:55432/mustel_test cargo test -p mustel-core -- --ignored`
   deve passar. Comando do container:
   `docker run -d --name mustel-t19-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=mustel_test -p 55432:5432 postgres:16`
3. **Formato exato do CSV não confirmado contra o oráculo.** O texto de cada tipo (principalmente
   `bytea`, `timestamptz` e `numeric`) é decisão de formatação que só o T32 fecha, comparando o mesmo
   dado nos dois binários. O mapeamento está isolado em `format_cell` justamente para ser corrigido
   num lugar só.
4. **Pool**: as opções (`pooling`, `min/max pool size`, `keepalive`, `connection lifetime`) já estão
   na configuração, mas ainda não há uma implementação de pool — o driver abre conexão direta. O
   pool entra quando o `query run` precisar de paralelismo real (T34), e a biblioteca será escolhida
   ali (`deadpool-postgres` ou `bb8`), reusando os mesmos campos.
