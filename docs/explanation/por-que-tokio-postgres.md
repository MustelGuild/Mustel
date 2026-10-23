# Por que `tokio-postgres`

> Decisão **D12**: usar **`tokio-postgres`** (escolha do usuário, 2026-10-15). A alternativa `sqlx`
> foi descartada. Estado: **implementado**; as pendências antigas aparecem no fim, com o estado atual.

## O motivo

- **Driver, não framework**: dá acesso direto a `Row`/`Column`/`Type`, que é o que o export para CSV
  precisa para mapear tipos arbitrários. `sqlx` empurra para tipos conhecidos em tempo de compilação,
  e o `query run` executa SQL livre — não há como conhecer os tipos antes.
- **Sem `query!` compilado**: o produto aceita qualquer SQL, então macros que exigem introspecção de
  banco não servem.
- **Fronteira pequena**: a API assíncrona casa com o `tokio` já usado para processos e cancelamento.

## O que está pronto (verificado por teste)

| Peça | Onde | Cobertura |
|---|---|---|
| Tradução do `sslMode` (6 modos) | `postgres/tls.rs` | nomes, rejeição de inválido, semântica de `requires_tls`/`tries_tls`/`verifies_hostname` |
| Configuração de conexão com os defaults herdados | `postgres/settings.rs` | defaults (`5432`, `30`, `300`, `1`, `100`, `Prefer`), parsing do modo, endpoint, `Config` do driver |
| Formatação de célula para CSV | `postgres/value.rs` | `NULL` → vazio, escalares, `float` com `.0`, listas, `bytea` em hex, datas en-US |
| Conexão e classificação de erro | `postgres/client.rs` | erro com SQLSTATE vira exit 10; sem SQLSTATE, 11 |

## O que falta (estado atual, 2026-10-22)

1. **TLS não está ligada.** `Require`, `VerifyCA` e `VerifyFull` devolvem erro explícito pedindo para
   usar `Prefer`/`Allow`/`Disable`. Ligar `tokio-postgres-rustls` + certificados do sistema é o
   próximo passo; `VerifyCA` exige um `ServerCertVerifier` próprio, porque o rustls valida hostname
   sempre e o Npgsql **não** valida em `VerifyCA`.
2. ~~Testes de integração não executados~~ — **resolvido**: a suíte roda contra Postgres real, gateada
   por `MUSTEL_PG_TEST_URL` (8/8 verdes, inclusive contra PostgreSQL 18.6).
3. ~~Formato do CSV não confirmado~~ — **resolvido**: conferido byte a byte contra o baseline
   congelado e pinado em `postgres/value.rs` e `query/csv_writer.rs`.
4. **Pool**: as opções (`pooling`, `min/max pool size`, `keepalive`, `connection lifetime`) estão na
   superfície porque o `query run` as aceita, mas **não há implementação de pool** — o driver abre uma
   conexão direta por banco. Paralelismo não precisa de pool (o grau é limitado por semáforo, não por
   reuso de conexão), e as consultas são longas, que é o caso em que pool ajuda menos. O que tem
   efeito é o `--keepalive`, que vai para o socket; as demais ficam inertes e documentadas em
   `postgres/settings.rs`.
