# Ajustes de segurança

> **O que é esta página.** O raciocínio por trás das decisões marcadas `SEC` no
> [contrato de interface](../contrato-de-interface.md). O manifesto continua sendo a lista
> executável (ID, onde se observa, teste); aqui está o porquê e o que foi verificado.

As decisões `SEC` nasceram de uma revisão de segurança depois da migração. A regra que as une: o
Mustel **nunca cai para menos proteção em silêncio**. Quando o comportamento menos protegido é uma
escolha do usuário (`Disable`, `Prefer`), ele é explícito e documentado; quando a proteção deveria
estar lá e falha, a operação falha — não vira texto puro, arquivo fora do diretório ou fórmula
executável.

## Credenciais

A senha de cada servidor vive no cofre do sistema, indexada pelo nome do servidor. O `mustel.jsonc`
guarda só o cadastro; nenhuma senha entra em log ou mensagem de erro. O `pgpass` continua existindo
como o arquivo padrão do PostgreSQL, mas só é escrito quando o usuário pede.

O `pgpass add` recusa CR, LF e NUL em qualquer campo (IF-106): o arquivo é lido linha a linha e não
tem escape para quebra de linha, então um valor com CR/LF criaria uma entrada extra — uma injeção de
linha no arquivo de credenciais.

Duas entradas existem para tirar a senha do `argv`, do histórico do shell e da lista de processos:

- `--password-stdin` (IF-252) lê **uma** linha, remove CRLF e recusa linha vazia; na linha de comando
  ele é exclusivo com `-W/--password`, e no núcleo a senha do stdin vence as demais fontes;
- `MUSTEL_PASSWORD` (IF-253) alimenta o `pgpass add` sem `--password`.

A precedência completa é `--password-stdin` → `-W` → `MUSTEL_PASSWORD` → cofre → prompt (IF-018), com
no máximo um prompt por servidor, não por banco.

Ver [Credenciais](credenciais.md) e [senhas no cofre](../how-to/senhas-no-cofre.md).

## TLS

`Prefer` e `Require` usam o store de certificados do sistema e validam **cadeia e hostname** com o
verificador padrão do `rustls`; não existe opção para aceitar qualquer certificado. `Prefer` só cai
para texto puro quando o servidor responde que não oferece TLS; erro de certificado nunca cai, e
`Require` nunca cai (IF-250, IF-251). Como o fallback é o contrato do `Prefer`, ele acontece **sem
aviso**: quem precisa de garantia escolhe `Require`.

**Esta semântica é do driver.** Nesta release, `database backup` e `database restore` chamam
binários externos que usam libpq, e o libpq não valida certificado em `prefer`/`require`. Por isso os
dois comandos só aceitam `--ssl-mode Disable` (IF-107); `Prefer` e `Require` são recusados com exit 2.
Para backup/restore com TLS, use `pg_dump`/`pg_restore` diretamente com `sslmode=verify-full` e
`sslrootcert`. A correção definitiva é tratar TLS nos binários externos.

Restaram três nomes (`Disable`, `Prefer`, `Require`). `Allow`, `VerifyCA` e `VerifyFull` são recusados
com mensagem de migração (IF-256), e um `sslMode` inválido no cadastro é erro, não `Prefer` silencioso
(IF-066).

Ver [TLS do PostgreSQL](tls-postgres.md).

## Consulta destrutiva

O classificador inspeciona **todos** os CTEs e o `EXPLAIN ANALYZE` de DML, não só o primeiro CTE. A
confirmação mostra a operação real que será executada (`DELETE`, `UPDATE`, ...). Também entram
`COPY ... FROM`/`COPY ... PROGRAM`, `CALL`, `SELECT ... INTO`, `VACUUM`, `REINDEX`, `CLUSTER`,
`REFRESH MATERIALIZED VIEW` e `DO`; `COPY ... TO STDOUT`/arquivo continua seguro. Os 21 casos
herdados do legado continuam com a mesma descrição (IF-254, IF-255, IF-261).

Ver [Classificador de query](classificador-de-query.md).

## Saída e nomes de arquivo

- **CSV**: `--csv-escape-formulas` prefixa `'` em campo que uma planilha avaliaria como fórmula
  (`=`, `+`, `-`, `@`, TAB, CR), no cabeçalho e nas linhas. É opcional: desligado, os bytes são os do
  legado (IF-064).
- **Permissões**: no Unix, o `Appender` usa `private_fs` para deixar CSVs, `_log.csv` e `_erros.csv`
  com `0600`, e o diretório da execução com `0700` (IF-108). Os dumps do `pg_dump` também são
  restringidos: arquivo `0600`, ou árvore `0700`/`0600` no `-F d`, e o diretório novo do
  `backup --all` nasce `0700` (IF-109).
- **Log de atividade**: no Unix, `logs/` é `0700` e cada `activity-*.jsonl` é `0600` via `private_fs`;
  `load` e `prune` restringem arquivos existentes. No Windows, a ACL de `%LOCALAPPDATA%` é a garantia.
- **Backup**: o nome de banco vindo do servidor (`datname`) só vira nome de arquivo se não tiver
  sintaxe de caminho. Nome hostil falha apenas aquele banco no `--all` e aborta o backup individual,
  sem escrever fora do diretório de saída (IF-063). `--strict` transforma qualquer falha do `--all`
  em saída não-zero (IF-065).
- **Processos externos**: o argv do `pg_dump`/`pg_restore` é sempre uma lista de argumentos, nunca
  uma string interpretada por shell; stdout e stderr são capturados e a árvore de processos morre no
  cancelamento.

Ver [CSV como o Npgsql](csv-como-o-npgsql.md) e [Backup e restore](../how-to/backup-e-restore.md).

## IA

O `baseUrl` de um perfil exige `https`. `http` só é aceito em loopback (`localhost`, `127.0.0.1`,
`[::1]`), e URL com credenciais é recusada ao salvar e antes de cada requisição (IF-249).

O transporte não segue redirecionamento: o `baseUrl` cadastrado é o único destino aceito. Uma
resposta `3xx` vira erro que mostra o destino (só esquema, host e porta), porque seguir um
`307`/`308` reenviaria o prompt e o DDL a um host não cadastrado — e um downgrade para `http` os
mandaria em texto puro. A IA hoje só existe na janela, então essa decisão não entra no manifesto
(CD13); o teste é `ai_transport.rs::tests::a_redirect_is_not_followed`.

Ver [Por que a IA escreve e o humano roda](ia.md).

## Cadeia de suprimentos

- `deny.toml` recusa crate yanked, licença fora da lista permitida e registry/git desconhecidos; o CI
  roda `rustsec/audit-check` e `cargo-deny check licenses sources bans`.
- `sbom.cdx.json` é CycloneDX 1.5 gerado pelo `cargo-cyclonedx` a partir do `Cargo.lock`, com
  `serialNumber`, hash SHA-256 por componente e o grafo `dependencies`. A raiz cobre todos os alvos;
  cada pacote leva o do alvo compilado.
- `THIRD-PARTY-LICENSES.md` reproduz os textos completos das licenças dos componentes compilados,
  gerado pelo `cargo-about` com a mesma lista de licenças aceitas do `deny.toml` (`about.toml`). Sem
  ele, MIT/BSD/Apache-2.0 ficariam sem o aviso de copyright que exigem. `THIRD-PARTY-NOTICES.md`
  continua sendo o resumo curto, com a fonte embutida e o MPL-2.0 do `option-ext`. Os mesmos textos
  viajam **embutidos no binário** e aparecem na aba Settings › **About**, para quem recebe só o
  executável não depender dos arquivos do pacote.
- O Dependabot acompanha as dependências, e as actions do CI são pinadas por SHA — uma tag
  re-apontada não executa código com o token do repositório.
- **Crates não mantidos (Linux).** O `keyring` 4 traz `secret-service` → `zbus` 5, e com ele
  `zvariant`, `zbus_names` e `zvariant_derive`, que o `zbus` 6 incorporou. O RustSec marca os três como
  *unmaintained*, sem CVE associado. Enquanto `secret-service` e `zbus-secret-service-keyring-store`
  não publicarem a versão com `zbus` 6, vendorizar corrigiria só este workspace e não quem instala
  pelo crates.io, e trocar o backend por `dbus-secret-service` reintroduziria o `libdbus` que a
  decisão `B1` recusa. A escolha é aguardar o upstream, com os avisos visíveis no `cargo audit`.

## Auditoria pontual

O resultado da auditoria de segurança de 2026-11-08 — achados, evidências, gravidade e remediação —
está em [Auditoria de segurança](auditoria-de-seguranca.md).

## O que foi verificado

O gate `cargo test --workspace` prende cada linha `SEC` ao teste citado no manifesto (as marcações
`CONTRACT: IF-NNN` no código e a tabela são conferidas nas duas direções). As linhas de TLS
(IF-250, IF-251) são as únicas `SEC` sem teste automatizado: handshake exige servidor real, e a
matriz foi exercitada no smoke manual de 2026-10-28 (`PASSED=42`, `FAILED=0`), incluindo CA ausente,
serviço sem TLS e hostname fora do SAN.

A política para quem encontra uma vulnerabilidade está em [`SECURITY.md`](../../SECURITY.md).
