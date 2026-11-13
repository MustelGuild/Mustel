# Auditoria de segurança — 2026-11-08

> **O que é esta página.** O resultado da auditoria pontual do Mustel: escopo, achados, evidência,
> gravidade, remediação e riscos aceitos. As decisões de superfície marcadas `SEC` estão em
> [Ajustes de segurança](seguranca.md).

## Escopo e método

A revisão cobriu o workspace Rust (`mustel-core`, `mustel`, `mustel-gui`, `mustel-testkit`), os
scripts de release e do instalador, os workflows do GitHub Actions, o harness `installer/wine-test`,
a documentação e a cadeia de suprimentos (`Cargo.lock`, `deny.toml`, SBOM e licenças).

Foi uma revisão manual apoiada pelo que o gate já executa: `cargo audit`/RustSec, `cargo-deny`
(licenças, fontes e bans), `cargo-about`/SBOM e inspeção de Dockerfile e scripts. Não houve pentest
nem análise dinâmica de binário; o objetivo era achar exposição local, escrita fora do diretório e
falhas de cadeia de suprimentos, não descobrir vulnerabilidades no PostgreSQL ou no Windows.

> A numeração segue a ordem dos achados. `M1` está citado no teste do log e `M4` no histórico da
> correção da licença; o relatório original não era versionado, então a evidência reprodutível é o
> código, o commit e o teste citados.

## Achados

| ID | Gravidade | Achado | Evidência | Remediação | Status |
|---|---|---|---|---|---|
| M1 | Média | O log de atividade guarda nomes de servidores, hosts e mensagens de erro, mas no Unix o diretório `logs/` e os arquivos `activity-*.jsonl` podiam ficar legíveis por outros usuários com o modo padrão. | `crates/mustel-core/src/activity/store.rs::tests::the_log_directory_and_files_are_private`; [referência do log](../reference/activity-log.md). | `logs/` nasce `0700` e cada arquivo `0600` via `private_fs`; `load`/`prune` restringem arquivos existentes. Commit `7a58087`. | corrigido |
| M2 | Média | O salvamento do relatório da Activity podia sobrescrever um arquivo existente ou seguir um symlink; sem pasta de estado, o fallback não garantia um diretório privado. | `crates/mustel-core/src/private_fs.rs::write_new` (`create_new`, `0600`) e o teste `write_new_refuses_an_existing_file`; `activity/paths.rs::reports_dir_or_private_fallback`/`private_dir`; `activity/reports.rs::prune` e o teste `it_prunes_only_old_reports_and_restricts_the_rest`. | `create_new` recusa sobrescrever e seguir symlink; fallback privado (home ou temporário `0700`); retenção de 30 dias; a tela avisa quando não há pasta privada. Commit `d2e16c9`. | corrigido |
| M3 | Baixa (harness de teste) | O harness Wine executava o instalador baixado sem conferir o SHA-256 e usava a imagem base sem digest; o prefixo era criado em `/wine` antes do usuário. | `installer/wine-test/Dockerfile` (`FROM ubuntu:24.04@sha256:…`, `ARG INNO_SHA256`, `useradd wine`, `USER wine`); `run.sh`/`real.sh` repassam `INNO_SHA256`. | Base pinada por digest; hash do Inno Setup conferido antes de executar; prefixo criado pelo usuário `wine`. Commit `0a6c4fe`. | corrigido |
| M4 | Informativo | A documentação afirmava que o compilador free do Inno Setup era “Non-commercial use only”. | A [licença oficial](https://raw.githubusercontent.com/jrsoftware/issrc/main/license.txt) permite uso comercial; a JRSoftware pede (não exige) licença para uso comercial. O texto atual está em [Decisões](decisoes.md) (`D31`) e [publicar uma release](../how-to/publicar-release.md). | Texto corrigido; o achado foi **descartado** por não ser uma vulnerabilidade. Commits `8a125ed`/`9bc1178`. | descartado |

### O que cada correção mudou

- **M1 — permissões do log.** O diretório `logs/` passou a ser criado `0700` e os arquivos `0600`
  no Unix; os arquivos que já existiam são restringidos no `load` e na poda. No Windows a garantia
  é a ACL de `%LOCALAPPDATA%`.
- **M2 — gravação do relatório.** `private_fs::write_new` abre com `create_new` (recusa caminho
  existente e symlink) e modo `0600`; o diretório de relatórios usa um fallback privado em vez de
  `/tmp` direto; a retenção poda os `.log` antigos e restringe os que ficam.
- **M3 — harness do instalador.** A imagem base está pinada pelo digest, o instalador do Inno
  Setup é conferido por SHA-256 antes de rodar e o prefixo Wine é criado pelo usuário `wine`; isso
  evita executar um payload não verificado e deixa o prefixo com o dono do runtime.
- **M4 — licença.** Não era um achado de segurança: a afirmação da documentação estava errada e
  foi removida. O Mustel é open source sem fins lucrativos; a licença oficial permite uso
  comercial, com compra **solicitada** (não exigida) para usuários comerciais.

## Riscos aceitos

| Risco | Por que fica | Onde está registrado |
|---|---|---|
| `secret-service`/`zbus` 5 não mantidos (RustSec, sem CVE) | Vendorizar corrigiria só este workspace e não quem instala pelo crates.io; trocar por `dbus-secret-service` reintroduziria o `libdbus` que a decisão `B1` recusa. A escolha é aguardar o upstream. | [Ajustes de segurança](seguranca.md), [Decisões](decisoes.md) |
| `pg_dump`/`pg_restore` não validam certificado em `prefer`/`require` (libpq) | Os comandos só aceitam `--ssl-mode Disable`; a correção definitiva é tratar TLS nos binários externos. | [Ajustes de segurança](seguranca.md), [TLS](tls-postgres.md) |
| Instalador Windows sem assinatura de código | O SmartScreen pode avisar; a assinatura é um follow-up (SignPath/OSS, Azure Trusted Signing ou OV/EV). | [Instalador do Windows](../reference/instalador-windows.md) |
| Sem pentest nem análise dinâmica de binário | O escopo desta auditoria foi revisão estática e cadeia de suprimentos. | esta página |

## O que foi verificado

Cada achado acima tem uma evidência versionada:

- **M1:** `the_log_directory_and_files_are_private` (`cfg(unix)`);
- **M2:** `write_new_refuses_an_existing_file`, `it_prunes_only_old_reports_and_restricts_the_rest`
  e `a_missing_directory_is_zero`;
- **M3:** o `Dockerfile` com digest e o `INNO_SHA256` conferido no build; não é teste de unidade;
- **M4:** revisão da licença oficial e do texto publicado.

O gate `cargo test --workspace` continua sendo a verificação executável; as marcações `CONTRACT`
ligam cada linha `SEC` do [contrato](../contrato-de-interface.md) ao teste. A política para relatar
uma vulnerabilidade está em [`SECURITY.md`](../../SECURITY.md).
