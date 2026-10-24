# Plano — Pendências de release do Mustel (winget, crates.io, workflow)

> **Status:** pendente
> **Criado em:** 2026-10-21
> **Atualizado em:** 2026-10-21
> **Complexidade:** baixa (três tarefas pequenas, todas bloqueadas por recurso externo)
> **Depende de (outros planos):** nada — o empacotamento (`publish.ps1`/`publish.sh`) está pronto e
> verificado (`D26` em [`docs/explanation/decisoes.md`](../docs/explanation/decisoes.md))

---

## 1. Visão Geral

O empacotamento do Mustel está pronto e verificado nas duas plataformas suportadas: **Windows**
(`publish.ps1` → `mustel-1.0.0-x86_64-pc-windows-msvc.zip`) e **Linux** (`publish.sh` em container →
`mustel-1.0.0-x86_64-unknown-linux-gnu.tar.gz`), cada um com SHA256 e o binário executado a partir da
extração. Este plano guarda o que **sobrou de fora** dessa entrega, porque cada item depende de um
recurso que não existe neste ambiente: uma URL de release pública, um token e uma execução de CI.

**Fora de escopo por decisão (2026-10-21):** instaladores **macOS** (x64/arm64). Exigem runner Apple e
não entram no release até que isso exista.

---

## 2. Escopo

- [ ] **P1** — Manifesto e validação do **winget** (`Mustel.CLI`)
  - **O que falta**: o `winget-publish.ps1` **gera** o manifesto em `packaging\winget` a partir de uma
    URL de release, via `wingetcreate`; nada disso existe ainda. O `winget` está instalado na máquina
    (v1.29.380), o `wingetcreate` não.
  - **Pré-requisito**: um release real no GitHub com o `.zip` e o `.sha256` (ver P3).
  - **Passos**: `winget install Microsoft.WingetCreate` → rodar `./winget-publish.ps1` (ele coleta
    versão, URL e SHA; installer `portable`, command `mustel`) → `winget validate` no manifesto
    gerado → abrir o PR no `microsoft/winget-pkgs` (o script já faz via `wingetcreate submit`).
  - **Concluído quando**: `winget validate` passa no manifesto gerado e `winget install Mustel.CLI`
    instala e deixa `mustel --version` respondendo numa máquina limpa.

- [ ] **P2** — Publicação no **crates.io**
  - **O que falta**: token e a ordem obrigatória. `mustel-core` empacota e verifica hoje
    (`cargo publish --dry-run -p mustel-core` → 41 arquivos, 414 KiB); `mustel-gui` e `mustel`
    **não podem** ser preparados antes, porque dependem do core por caminho+versão e o
    `--dry-run` deles falha com `no matching package named mustel-core`.
  - **Passos**: `cargo login` (token do usuário) → `cargo publish -p mustel-core` →
    `cargo publish -p mustel-gui` → `cargo publish -p mustel`. Cada um é **irreversível** (a versão
    publicada não pode ser reaproveitada).
  - **Concluído quando**: `cargo install mustel` funciona nas duas plataformas suportadas e
    `mustel --version` responde a partir da instalação.
  - **Ressalva**: o `mustel-gui` carrega `eframe`/`glow`; publicar não muda a dependência de sistema
    descoberta no T44 (`libdbus-1.so.3` em runtime no Linux) — ela precisa continuar documentada.

- [ ] **P3** — Primeiro **release real** e primeira execução do workflow
  - **O que falta**: os dois artefatos existem localmente em `packaging/` (que é gitignored), mas
    nunca foram publicados; `.github/workflows/release-build.yml` nunca rodou.
  - **Passos**: criar a tag/release no GitHub (D18: o repositório segue como `liphvf/FurLab`) e subir
    `.zip`, `.tar.gz` e os `.sha256` → disparar o workflow **à mão** (`workflow_dispatch`) e conferir
    que ele produz os mesmos artefatos → comparar o SHA do workflow com o local (o release deve ser
    reprodutível no que importa: mesmo conjunto de arquivos).
  - **Concluído quando**: a release publicada tem os artefatos das duas plataformas com SHA256, e o
    workflow rodou uma vez com sucesso.

---

## 3. Decisões e restrições

- **D24** (herdada): não há gates automáticos de qualidade no CI; o workflow de release é **só**
  empacotamento, disparado à mão.
- **D26/D27** (herdadas): publicação **manual**; o CI só produz o artefato.
- **macOS**: fora de escopo até haver runner Apple. Quando voltar, é um P4 neste plano, com os
  instaladores x64 e arm64.
- **Linux**: pacote `gnu` (dinâmico) — a decisão "Linux com GUI" torna `musl` inviável, e a
  dependência de runtime (`libdbus-1-3`) está no `README.txt` do pacote.

---

## 4. Como verificar cada item

| Item | Comando | Esperado |
|---|---|---|
| P1 | `winget validate packaging\winget\**` | sem erros |
| P1 | `winget install Mustel.CLI` (máquina limpa) | `mustel --version` responde |
| P2 | `cargo install mustel` | instala e `mustel --version` responde |
| P3 | GitHub → Actions → release-build → Run workflow | job verde, artefatos com `.sha256` |
| P3 | `sha256sum` do artefato publicado × o local | mesmo conteúdo na mesma versão |
