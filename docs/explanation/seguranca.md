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

Restaram três nomes (`Disable`, `Prefer`, `Require`). `Allow`, `VerifyCA` e `VerifyFull` são recusados
com mensagem de migração (IF-256), e um `sslMode` inválido no cadastro é erro, não `Prefer` silencioso
(IF-066).

Ver [TLS do PostgreSQL](tls-postgres.md).

## Consulta destrutiva

O classificador inspeciona **todos** os CTEs e o `EXPLAIN ANALYZE` de DML, não só o primeiro CTE. A
confirmação mostra a operação real que será executada (`DELETE`, `UPDATE`, ...). Os 21 casos herdados
do legado continuam com a mesma descrição (IF-254, IF-255).

Ver [Classificador de query](classificador-de-query.md).

## Saída e nomes de arquivo

- **CSV**: `--csv-escape-formulas` prefixa `'` em campo que uma planilha avaliaria como fórmula
  (`=`, `+`, `-`, `@`, TAB, CR), no cabeçalho e nas linhas. É opcional: desligado, os bytes são os do
  legado (IF-064).
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

Ver [Por que a IA escreve e o humano roda](ia.md).

## Cadeia de suprimentos

- `deny.toml` recusa crate yanked, licença fora da lista permitida e registry/git desconhecidos; o CI
  roda `rustsec/audit-check` e `cargo-deny check licenses sources bans`.
- `sbom.cdx.json` descreve o grafo de dependências, e `THIRD-PARTY-NOTICES.md` cobre as licenças que
  exigem aviso (incluindo a fonte embutida).
- O Dependabot acompanha as dependências, e as actions do CI são pinadas por SHA — uma tag
  re-apontada não executa código com o token do repositório.

## O que foi verificado

O gate `cargo test --workspace` prende cada linha `SEC` ao teste citado no manifesto (as marcações
`CONTRACT: IF-NNN` no código e a tabela são conferidas nas duas direções). As linhas de TLS
(IF-250, IF-251) são as únicas `SEC` sem teste automatizado: handshake exige servidor real, e a
matriz foi exercitada no smoke manual de 2026-10-28 (`PASSED=42`, `FAILED=0`), incluindo CA ausente,
serviço sem TLS e hostname fora do SAN.

A política para quem encontra uma vulnerabilidade está em [`SECURITY.md`](../../SECURITY.md).
