# How-to: publicar uma release

> Você precisa **lançar uma versão nova** do Mustel — os binários na página de release e o pacote no
> crates.io. Este é o caminho completo, do incremento de versão até a publicação.

Canais (decisão `D17`): **GitHub Releases** (executável) e **crates.io** (`cargo install mustel`).
Não há winget.

## Pré-requisitos

```bash
# Ferramenta do incremento de versão: é um SUBCOMANDO do cargo, então vive no PATH (não é
# dependência do projeto). Versão pinada para o release ser reprodutível entre máquinas.
cargo install cargo-release --version =1.1.7 --locked
cargo login                   # token do crates.io, uma vez por máquina
```

Quem só consome a release **não** precisa dele.

A versão vive em **um** lugar: `[workspace.package] version` no `Cargo.toml` da raiz. Os quatro crates
herdam com `version.workspace = true`, então não há versão para sincronizar à mão.

## 1. Incrementar a versão e criar a tag

```bash
cargo release patch            # ensaio: mostra o plano e NÃO altera nada (dry-run é o padrão)
cargo release patch --execute  # faz: bump + Cargo.lock + dependências internas + commit + tag
```

`patch` pode ser `minor`, `major` ou uma versão explícita (`cargo release 1.2.0 --execute`).

**Antes do `--execute`, a árvore precisa estar limpa** — inclusive sem arquivo não rastreado. O
`cargo release` recusa rodar com qualquer mudança pendente (é o que impede publicar um código que
não está no commit).

O `cargo-release` está configurado em `[workspace.metadata.release]` (e `[package.metadata.release]`
nos três crates) para:

- usar **versão compartilhada** no workspace;
- criar a tag `v<versao>`;
- atualizar as dependências internas (`mustel-core`, `mustel-gui`) junto;
- **não** publicar no crates.io e **não** empurrar para o remoto (são passos separados, abaixo).

> A saída do ensaio imprime `Publishing mustel-core, mustel-gui, mustel` **mesmo com**
> `publish = false`. É rótulo da etapa, impresso antes do filtro: nenhum `cargo publish` roda (o
> código só executa quando a lista de pacotes a publicar não está vazia).

Se você rodar fora do branch `main`, ele recusa — é a guarda de `allow-branch`.

## 2. Conferir antes de empurrar

```bash
cargo test --workspace   # o gate; rode com a saída redirecionada se estiver num PTY
git show --stat HEAD     # o commit do bump
git tag --list 'v*'
```

## 3. Empurrar commit e tag

```bash
git push --follow-tags
```

## 4. Empacotar localmente

Cada plataforma é empacotada na máquina dela. Não há build cruzado nem workflow de release.

No Windows:

```powershell
./publish.ps1
```

No Linux:

```bash
./publish.sh
```

Os dois scripts geram os arquivos em `packaging/`:

- `mustel-<versao>-x86_64-pc-windows-msvc.zip` e `.sha256`;
- `mustel-<versao>-x86_64-unknown-linux-gnu.tar.gz` e `.sha256`.

Confira que o número nos nomes é o mesmo de `Cargo.toml` e da tag:

```bash
git tag --points-at HEAD
ls packaging/
```

## 5. Criar a release no GitHub (manual)

A release não é criada por workflow. Depois de empurrar a tag, crie a release manualmente na página
**Releases → Draft a new release** do repositório, escolha a tag e anexe:

- o `.zip` e o `.sha256` do Windows;
- o `.tar.gz` e o `.sha256` do Linux;
- o `sbom.cdx.json` da raiz.

Alternativa por linha de comando, se você usa o GitHub CLI:

```bash
gh release create "v<versao>" \
  packaging/*.zip packaging/*.tar.gz packaging/*.sha256 \
  sbom.cdx.json \
  --title "Mustel v<versao>" --generate-notes
```

Antes de publicar, confira que a tag, a versão do `Cargo.toml` e os nomes dos artefatos são os
mesmos. O workflow antigo fazia essa conferência; agora ela é um passo manual.

## 6. Publicar no crates.io

A ordem é obrigatória — cada crate depende do anterior:

```bash
cargo publish -p mustel-core
cargo publish -p mustel-gui
cargo publish -p mustel
```

`mustel` é o comando que o usuário roda (`cargo install mustel`); os outros dois vão como
**dependências**. `mustel-testkit` não é publicado (`publish = false`, fora do ciclo de release).

Cada `cargo publish` é **irreversível**: a versão publicada não pode ser reaproveitada, e uma falha
no meio exige subir a versão para tentar de novo.

## 7. Conferir

```bash
cargo install mustel --force
mustel --version        # deve dizer a versão nova
```

E na página de release: os dois arquivos, os dois `.sha256`, e a nota de release gerada.

## O que é automático

Só o gate de qualidade em push/PR (`.github/workflows/ci.yml`: `fmt`, `clippy`, `test`). O release
é **inteiramente manual**: empacotamento por `publish.ps1`/`publish.sh`, criação da release no
GitHub, upload dos artefatos e `cargo publish` (decisões `D24`, `D26` e `D29`).

Não existe `.github/workflows/release-build.yml`; release não é disparada por tag, botão do Actions
nem `workflow_dispatch`.
