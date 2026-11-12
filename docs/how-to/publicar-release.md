# How-to: publicar uma release

> Você precisa **lançar uma versão nova** do Mustel — os binários na página de release e o pacote no
> crates.io. Este é o caminho completo, do incremento de versão até a publicação.

Canais (decisão `D17`): **GitHub Releases** (executável) e **crates.io** (`cargo install --locked mustel`).
Não há winget.

## Pré-requisitos

```bash
# Ferramenta do incremento de versão: é um SUBCOMANDO do cargo, então vive no PATH (não é
# dependência do projeto). Versão pinada para o release ser reprodutível entre máquinas.
cargo install cargo-release --version =1.1.7 --locked
# Gerador do CHANGELOG (Features/Bug fixes) a partir dos commits.
cargo install git-cliff --version =2.14.2 --locked
cargo login                   # token do crates.io, uma vez por máquina
```

Quem só consome a release **não** precisa dele.

A versão vive em **um** lugar: `[workspace.package] version` no `Cargo.toml` da raiz. Os quatro crates
herdam com `version.workspace = true`, então não há versão para sincronizar à mão.

## 1. Incrementar a versão e criar a tag

```bash
cargo release patch            # ensaio: mostra o que faria e NÃO altera nada (dry-run é o padrão)
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

Se você rodar o `cargo release` direto fora do branch `main`, ele recusa — é a guarda de
`allow-branch`.

### Preparar o bump para um PR (sem tag)

Os scripts `prepare-release.ps1` (Windows) e `prepare-release.sh` (Linux) perguntam a versão e
preparam o bump numa branch `release/v<versão>`, **sem criar a tag**:

```bash
./prepare-release.sh          # pergunta a versão
./prepare-release.sh 1.2.0    # ou informe direto
```

```powershell
./prepare-release.ps1
./prepare-release.ps1 1.2.0
```

Eles criam a branch, rodam `cargo release` com `--no-tag`, atualizam a documentação, o
`CHANGELOG.md` e o `sbom.cdx.json`, rodam `fmt`/`clippy`/testes e, com confirmação, empurram a
branch e abrem o PR. Eles partem do **branch atual** e abrem o PR contra `main`; se você estiver
num branch de feature, os commits dele entram no PR. Para um PR só do bump, rode a partir de
`main`. Use `--no-push` (`-NoPush`) para parar antes do push/PR e `--skip-tests` (`-SkipTests`)
para pular o `cargo test`. Os dois scripts precisam estar **commitados** antes da execução: o
`cargo release` recusa árvore suja, inclusive arquivo não rastreado. A tag fica para depois do
merge, no `main`:

```bash
git switch main
git pull --ff-only
git tag -a v<versao> -m "Mustel <versao>"
git push origin v<versao>
```

O `CHANGELOG.md` é gerado pelo `git-cliff` a partir dos commits do branch, com seções **Features**,
**Bug fixes**, **Segurança**, **Documentação** e **Outras mudanças**. Ele é gerado **antes** do
merge: se o PR for squashado, os commits individuais não ficam no `main`, mas o `CHANGELOG.md`
fica. Se a seção da versão já existir, o script a mantém em vez de gerar outra.

Os scripts rodam o `cargo test` com a entrada em `NUL` (`cmd /c ... < NUL`) ou `/dev/null`, para os
testes de UI sem terminal não herdarem o console interativo de quem executa. O gate continua sendo
o `cargo test --workspace`.

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

Empurrar a tag **não publica nada**: o `publish` é disparado à mão (passo 4). A tag só precisa
existir no remoto, porque o `preflight` e o `gh release create --verify-tag` conferem que ela está lá.

## 4. Disparar o `publish` (manual)

**Actions → publish → Run workflow**, informando a **tag** no campo (ex.: `v1.1.0`). O `preflight`
confere, **antes de baixar o código**, o formato `v<versao>` e se a tag existe no remoto; depois
faz checkout dela e exige que case com `[workspace.package] version`. Cada recusa sai como
`::error::` dizendo o que recebeu — em particular, tag que ainda não foi empurrada não chega nem a
virar checkout.

`.github/workflows/publish.yml` roda nesta ordem:

1. o **QA Gate** (`.github/workflows/qa-gate.yml`, chamado por `workflow_call`): `fmt`, `clippy` e
   `test` no Windows, RustSec e `cargo-deny` no Linux;
2. o `preflight`: a tag precisa ser `v<versao>` e `<versao>` precisa ser a de
   `[workspace.package] version`;
3. o empacotamento nas duas plataformas, com os mesmos scripts usados à mão — `publish.ps1` no
   Windows e `publish.sh` no Linux —, gerando o `.zip` (com o `mustel.exe` e o `mustel-gui.exe`), o
   `.tar.gz`, os `.sha256`, o `sbom.cdx.json` do alvo e o `THIRD-PARTY-LICENSES.md` dentro de cada
   pacote; no Windows, depois do `publish.ps1`, o `installer/publish-installer.ps1` gera o
   **setup** (`mustel-<versão>-x86_64-pc-windows-msvc-setup.exe`) com o Inno Setup já instalado no
   runner, e o `.sha256` ao lado;
4. o `SHA256SUMS`, com o hash de cada anexo, para conferir tudo de uma vez;
5. as notas da release, extraídas da seção da tag no `CHANGELOG.md` (se a seção não existir, o job
   falha em vez de publicar sem notas);
6. a release no GitHub, com a tag, os dois pacotes, os dois `.sha256`, o `SHA256SUMS`, o
   `sbom.cdx.json` da raiz, os **executáveis soltos do Windows** (`mustel.exe` e
   `mustel-gui.exe`), o **setup do Windows** (`mustel-...-setup.exe`) e o **executável cru do
   Linux** (`mustel`) — é esse `mustel` sem empacotamento que o updater da janela baixa.

Se a release já existir (reexecução), os assets são reenviados com `--clobber` em vez de falhar
e as notas são atualizadas com `gh release edit --notes-file`.

### Empacotar sem publicar

Os scripts continuam sendo a fonte única do empacotamento e podem rodar localmente — útil para
conferir o pacote antes de taguear:

No Windows:

```powershell
./publish.ps1
./installer/publish-installer.ps1
```

O segundo passo precisa do **Inno Setup 6 ou 7** instalado no Windows (`ISCC.exe`; o
`windows-latest` já traz o 6.7.1). Ele gera o setup e o `.sha256` a partir do stage que o
`publish.ps1` deixou; se o stage não existir, o script chama o `publish.ps1` sozinho.

No Linux:

```bash
./publish.sh
```

Os scripts geram os arquivos em `packaging/`:

- `mustel-<versao>-x86_64-pc-windows-msvc.zip` e `.sha256`;
- `mustel-<versao>-x86_64-unknown-linux-gnu.tar.gz` e `.sha256`;
- `mustel-<versao>-x86_64-pc-windows-msvc-setup.exe` e `.sha256` (Windows; o setup do instalador).

> **Licença do Inno Setup:** o Mustel é open source sem fins lucrativos; a JRSoftware não solicita
> licença para uso não comercial ([FAQ](https://jrsoftware.org/isorder.php)). A
> [licença oficial](https://raw.githubusercontent.com/jrsoftware/issrc/main/license.txt) também
> permite uso comercial (“including commercial applications”); nesse caso, a compra é solicitada,
> não exigida. O release no `windows-latest` usa o 6.7.1 pré-instalado.

Os scripts exigem o `cargo-cyclonedx` (`cargo install cargo-cyclonedx --locked`, validado com a
0.5.9) e incluem no pacote um `sbom.cdx.json` CycloneDX 1.5, gerado a partir do `Cargo.lock` para
o alvo compilado, com hash SHA-256 por componente e o grafo de dependências.

Exigem também o `cargo-about` (`cargo install cargo-about --locked --features cli`, validado com a
0.9.3), que gera o `THIRD-PARTY-LICENSES.md` com os textos completos das licenças dos componentes
compilados. A lista de licenças aceitas está em `about.toml` e é a mesma do `deny.toml`; um
componente com licença fora dela faz o release falhar. O CI instala as duas ferramentas na versão
pinada, então o pacote sai igual ao gerado à mão.

Confira que o número nos nomes é o mesmo de `Cargo.toml` e da tag:

```bash
git tag --points-at HEAD
ls packaging/
```

## 5. Conferir a release publicada

Na página **Releases** do repositório, a release `Mustel v<versao>` deve ter:

- o `.zip` e o `.sha256` do Windows;
- o `.tar.gz` e o `.sha256` do Linux;
- os executáveis soltos do Windows: `mustel.exe` e `mustel-gui.exe`;
- o setup do Windows: `mustel-...-setup.exe` e o `.sha256`;
- o executável **cru** do Linux: `mustel` (é o asset que o updater da janela baixa);
- o `SHA256SUMS`, com o hash de cada anexo (pacotes, executáveis e SBOM);
- o `sbom.cdx.json` da raiz (inventário de todos os alvos; cada pacote já leva o do seu alvo);
- as notas **Features**, **Bug fixes**, **Segurança**, **Documentação** e **Outras mudanças** no
  corpo da release.

Os nomes saem do `cargo pkgid` e do host do runner, então batem com a tag e com o `Cargo.toml` por
construção — o `preflight` recusou o contrário. Se faltar algum asset, reexecute o workflow para a
mesma tag. Pela linha de comando:

```bash
gh release view "v<versao>"
```

Para conferir os downloads depois de baixá-los, com os `.sha256` de cada pacote:

```bash
sha256sum -c ./*.sha256     # no Linux
```

## 6. Publicar no crates.io

A ordem é obrigatória — cada crate depende do anterior:

```bash
cargo publish -p mustel-core
cargo publish -p mustel-gui
cargo publish -p mustel
```

`mustel` é o comando que o usuário roda (`cargo install --locked mustel`); os outros dois vão como
**dependências**. `mustel-testkit` não é publicado (`publish = false`, fora do ciclo de release).

Cada `cargo publish` é **irreversível**: a versão publicada não pode ser reaproveitada, e uma falha
no meio exige subir a versão para tentar de novo.

## 7. Conferir

```bash
cargo install --locked mustel --force
mustel --version        # deve dizer a versão nova
```

Use o mesmo comando que o README recomenda ao usuário (`--locked`): é ele que instala o grafo de
dependências do `Cargo.lock` empacotado, e não uma resolução nova.

E na página de release: os dois arquivos, os dois `.sha256`, o `SHA256SUMS` e a nota de release
gerada.

## O que roda sozinho

| Evento | O que roda |
|---|---|
| push de tag, de branch ou pull request | **nada** |
| `workflow_dispatch` no `qa-gate.yml` | só o QA Gate |
| `workflow_dispatch` no `publish.yml` informando a tag | o pipeline de release inteiro |

O QA Gate (`.github/workflows/qa-gate.yml`: `fmt`, `clippy`, `test`, RustSec, `cargo-deny`) roda
sozinho pelo botão **Run workflow** e é chamado pelo `publish.yml` por `workflow_call` — nunca por
um evento de push de branch.

**Fora do CI:** o incremento de versão e a tag (`cargo release`), o push (`git push --follow-tags`),
o disparo do `publish` (manual, sobre a tag) e o `cargo publish`, que é irreversível e exige o token
do crates.io. O `cargo release` continua configurado para **não** publicar e **não** empurrar
(`publish = false`, `push = false`).

Decisões: `D24` (QA Gate), `D26` (scripts de empacotamento) e `D30` (release manual a partir da tag).
