# FurLab (.NET) — congelado

Este diretório é a implementação **.NET** do FurLab, **congelada** desde a migração para Rust
(T48). Ela não recebe funcionalidade nova, mas **não é código morto**: tem dois papéis ativos.

## 1. É o oráculo de paridade

O Mustel é uma reescrita, e o comportamento dele é **observado neste binário**, não inferido da
documentação. Toda decisão de paridade é conferida rodando os dois e comparando:

```bash
# publica este projeto em oracle/bin e captura o baseline
cargo run -p mustel-testkit --bin oracle-capture

# compara o Mustel com o baseline
cargo run -p mustel-testkit --bin oracle-compare
```

O processo e as evidências estão em [`../oracle/README.md`](../oracle/README.md); as divergências
aceitas, em [`../oracle/known-divergences.md`](../oracle/known-divergences.md).

## 2. Ainda é o produto dos grupos não migrados

A migração cobriu `query`, `database` e `settings` (mais a GUI). Os demais grupos continuam
existindo **só aqui**, até a segunda onda (D21):

`file`, `claude`, `opencode`, `winget`, `windowsfeatures`, `docker`.

## Como compilar

```bash
cd legacy
dotnet build FurLab.slnx -c Release
```

Requer **.NET SDK 10**. A versão vem do Nerdbank.GitVersioning ([`version.json`](version.json), base
`1.1` + altura do Git) — não é escrita à mão.

## Scripts — atenção aos nomes

Há **dois** `publish.ps1` no repositório, de produtos diferentes:

| caminho | o que publica |
|---|---|
| [`../publish.ps1`](../publish.ps1) | o **Mustel** (Rust): binário + pacote + SHA256 (T44) |
| [`publish.ps1`](publish.ps1) | o **FurLab .NET** deste diretório (`nuget`, `exe` self-contained ou `all`) |

Além deles:

- [`format.ps1`](format.ps1) — `dotnet format` neste diretório;
- [`winget-publish.ps1`](winget-publish.ps1) — publica o pacote **`FurLab.CLI`** no winget via
  `wingetcreate` (o pacote do legado; o do Mustel é `Mustel.CLI` e ainda não foi publicado — ver
  [`../MustelPlans/release-pendencias.md`](../MustelPlans/release-pendencias.md)).

## O que **não** está mais aqui

- os workflows de build/publicação NuGet saíram na remoção de CI (D24); o único workflow do
  repositório hoje é o de **empacotamento do Mustel**, disparado à mão (D27);
- o binário publicado pelo harness vai para `../oracle/bin/` e `../oracle/workdir/` (ambos ignorados
  pelo Git).

## Estado do porte

O plano da migração, as decisões e o que falta estão em
[`../MustelPlans/migracao-rust.md`](../MustelPlans/migracao-rust.md). Resumo: o escopo migrado está
fechado e a paridade verificada por harness; o backlog é a segunda onda dos grupos acima.
