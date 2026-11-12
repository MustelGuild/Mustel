#!/bin/sh
# Prepara o bump de versão do Mustel para um PR, sem criar a tag.
#
# Uso:
#   ./prepare-release.sh              # pergunta a versão
#   ./prepare-release.sh 1.2.0        # versão direta
#   ./prepare-release.sh 1.2.0 --yes  # sem confirmação
#   ./prepare-release.sh 1.2.0 --no-push --skip-tests
#
# O script cria a branch release/v<versão>, roda `cargo release` com --no-tag,
# atualiza docs e o sbom.cdx.json, roda o gate e (opcionalmente) empurra a branch
# e abre o PR. A tag v<versão> NÃO é criada: ela é criada depois do merge do PR.
set -eu

usage() {
    cat <<'EOF'
Uso: ./prepare-release.sh [versão] [opções]

  versão        X.Y.Z (se omitida, o script pergunta)
  --yes, -y     não pede confirmação
  --no-push     para depois do commit; não empurra nem abre PR
  --skip-tests  não roda cargo test (fmt e clippy continuam)
  --help, -h    mostra esta ajuda

O script cria a branch release/v<versão>, roda `cargo release` com --no-tag,
atualiza docs/sbom, roda o gate e (opcionalmente) empurra a branch e abre o PR.
A tag v<versão> NÃO é criada; crie-a depois do merge do PR.

O script parte do branch atual e abre o PR contra main. Num branch de feature,
os commits dele entram no PR; para um PR só do bump, rode a partir de main.

Os dois scripts precisam estar commitados antes da execução: o cargo release
recusa árvore suja, inclusive arquivo não rastreado.
EOF
}

fail() {
    echo "erro: $*" >&2
    exit 1
}

version_gt() {
    old_v=$1
    new_v=$2

    o1=${old_v%%.*}
    o_rest=${old_v#*.}
    o2=${o_rest%%.*}
    o3=${o_rest#*.}

    n1=${new_v%%.*}
    n_rest=${new_v#*.}
    n2=${n_rest%%.*}
    n3=${n_rest#*.}

    if [ "$n1" -gt "$o1" ]; then return 0; fi
    if [ "$n1" -lt "$o1" ]; then return 1; fi
    if [ "$n2" -gt "$o2" ]; then return 0; fi
    if [ "$n2" -lt "$o2" ]; then return 1; fi
    [ "$n3" -gt "$o3" ]
}

replace_phrase() {
    file=$1
    old_phrase=$2
    new_phrase=$3

    if [ ! -f "$file" ]; then
        echo "aviso: arquivo não encontrado: $file" >&2
        return 0
    fi
    if ! grep -Fq -- "$old_phrase" "$file"; then
        echo "aviso: frase não encontrada em $file: $old_phrase" >&2
        return 0
    fi

    tmp=$(mktemp)
    OLD_PHRASE=$old_phrase NEW_PHRASE=$new_phrase awk '
        BEGIN {
            old = ENVIRON["OLD_PHRASE"]
            new = ENVIRON["NEW_PHRASE"]
        }
        {
            line = $0
            out = ""
            while ((i = index(line, old)) > 0) {
                out = out substr(line, 1, i - 1) new
                line = substr(line, i + length(old))
            }
            print out line
        }
    ' "$file" > "$tmp"
    chmod 644 "$tmp"
    mv "$tmp" "$file"
    echo "atualizado: $file"
}

print_tag_steps() {
    cat <<EOF

Depois do merge do PR, crie a tag:
  git switch main
  git pull --ff-only
  git tag -a v$version -m "Mustel $version"
  git push origin v$version

Depois: Actions -> publish -> Run workflow -> v$version
EOF
}

repo=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo"

version=
yes=0
no_push=0
skip_tests=0

for arg in "$@"; do
    case "$arg" in
        --yes|-y) yes=1 ;;
        --no-push) no_push=1 ;;
        --skip-tests) skip_tests=1 ;;
        --help|-h) usage; exit 0 ;;
        -*) echo "opção desconhecida: $arg" >&2; usage >&2; exit 2 ;;
        *) version=$arg ;;
    esac
done

command -v git >/dev/null 2>&1 || fail 'git não encontrado no PATH'
command -v cargo >/dev/null 2>&1 || fail 'cargo não encontrado no PATH'
cargo release --version >/dev/null 2>&1 || fail \
    "cargo-release não encontrado; instale com: cargo install cargo-release --version =1.1.7 --locked"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail 'não estamos num repositório Git'
branch=$(git symbolic-ref --short HEAD 2>/dev/null || true)
[ -n "$branch" ] || fail 'HEAD destacado; volte para um branch antes de preparar a release'
if [ "$branch" != "main" ]; then
    echo "aviso: a branch de release parte de '$branch', não de main; o PR para main vai incluir os commits dela." >&2
fi
dirty=$(git status --porcelain)
if [ -n "$dirty" ]; then
    echo 'a árvore precisa estar limpa (inclui os próprios scripts: commite-os antes de rodar). Mudanças pendentes:' >&2
    printf '%s\n' "$dirty" >&2
    exit 1
fi

old=$(cargo pkgid -p mustel | sed 's/.*#//')
[ -n "$old" ] || fail 'não consegui obter a versão de cargo pkgid -p mustel'

if [ -z "$version" ]; then
    printf 'Versão a lançar (X.Y.Z): '
    read -r version
fi
printf '%s' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || fail \
    "versão inválida: '$version' (use X.Y.Z estável; o workflow de release só aceita vX.Y.Z)"
version_gt "$old" "$version" || fail \
    "a nova versão ($version) precisa ser maior que a atual ($old)"

rel_branch="release/v$version"
if git show-ref --verify --quiet "refs/heads/$rel_branch"; then
    fail "a branch $rel_branch já existe; apague-a ou use outra versão"
fi

echo
echo "Versão atual: $old"
echo "Nova versão:  $version"
echo "Branch:       $rel_branch"
echo 'Tag:          NÃO será criada (após o merge do PR)'
if [ "$yes" -eq 0 ]; then
    printf 'Continuar? [s/N] '
    read -r answer
    case "$answer" in
        s|S|sim|SIM|y|Y|yes|YES) ;;
        *) echo 'cancelado.'; exit 1 ;;
    esac
fi

git switch -c "$rel_branch"

echo
echo '==> Ensaio do cargo release'
cargo release "$version" --workspace --allow-branch "$rel_branch" \
    --no-tag --no-push --no-publish --no-confirm

echo
echo '==> Aplicando o bump (sem tag)'
cargo release "$version" --execute --workspace --allow-branch "$rel_branch" \
    --no-tag --no-push --no-publish --no-confirm

new=$(cargo pkgid -p mustel | sed 's/.*#//')
[ "$new" = "$version" ] || fail "o Cargo.toml não ficou em $version (cargo pkgid: $new)"

echo
echo '==> Atualizando documentação e fallbacks'
replace_phrase 'docs/contrato-de-interface.md' \
    "$(printf 'hoje `mustel %s`' "$old")" \
    "$(printf 'hoje `mustel %s`' "$version")"
replace_phrase 'docs/tutorials/primeiros-passos.md' \
    "$(printf 'algo como `mustel %s`' "$old")" \
    "$(printf 'algo como `mustel %s`' "$version")"
replace_phrase 'docs/how-to/publicar-release.md' \
    "$(printf 'ex.: `v%s`' "$old")" \
    "$(printf 'ex.: `v%s`' "$version")"
replace_phrase 'installer/mustel.iss' \
    "$(printf 'MyAppVersion "%s"' "$old")" \
    "$(printf 'MyAppVersion "%s"' "$version")"
replace_phrase 'installer/mustel.iss' \
    "$(printf 'packaging\\mustel-%s-x86_64-pc-windows-msvc' "$old")" \
    "$(printf 'packaging\\mustel-%s-x86_64-pc-windows-msvc' "$version")"
replace_phrase 'installer/wine-test/README.md' \
    "$(printf 'packaging/mustel-%s-x86_64-pc-windows-msvc' "$old")" \
    "$(printf 'packaging/mustel-%s-x86_64-pc-windows-msvc' "$version")"

echo
echo '==> Gerando o CHANGELOG'
if [ -f CHANGELOG.md ] && grep -q "^## \[v$version\]" CHANGELOG.md; then
    echo "CHANGELOG.md já tem a seção v$version; mantendo"
else
    command -v git-cliff >/dev/null 2>&1 || fail \
        "git-cliff não encontrado; instale com: cargo install git-cliff --version =2.14.2 --locked"
    git-cliff --unreleased --tag "v$version" --prepend CHANGELOG.md
    grep -q "^## \[v$version\]" CHANGELOG.md || fail \
        "CHANGELOG.md não tem a seção v$version"
fi

echo
echo '==> Regenerando o SBOM da raiz'
command -v cargo-cyclonedx >/dev/null 2>&1 || fail \
    "cargo-cyclonedx não encontrado; instale com: cargo install cargo-cyclonedx --version =0.5.9 --locked"
cargo cyclonedx -f json --spec-version 1.5 --override-filename sbom-root.cdx --target all
[ -f crates/mustel/sbom-root.cdx.json ] || fail 'SBOM não encontrado em crates/mustel/sbom-root.cdx.json'
cp crates/mustel/sbom-root.cdx.json sbom.cdx.json
rm -f crates/*/sbom-root.cdx.json
grep -q "\"version\": \"$version\"" sbom.cdx.json || fail \
    "o SBOM da raiz não contém a versão $version"

echo
echo '==> Incluindo docs, CHANGELOG e SBOM no commit do bump'
git add -- docs/contrato-de-interface.md docs/tutorials/primeiros-passos.md \
    docs/how-to/publicar-release.md installer/mustel.iss \
    installer/wine-test/README.md CHANGELOG.md sbom.cdx.json
if ! git diff --cached --quiet; then
    git commit --amend --no-edit
else
    echo 'nenhuma mudança extra para incluir'
fi

echo
echo '==> cargo fmt --all --check'
cargo fmt --all -- --check

echo
echo '==> cargo clippy'
cargo clippy --workspace --all-targets --all-features -- -D warnings

if [ "$skip_tests" -eq 0 ]; then
    echo
    echo '==> cargo test (gate; stdin em /dev/null para os testes sem terminal)'
    # Os testes de ui_primitives exigem ausência de TTY. Redirecionar a entrada
    # evita que herdem o terminal de quem roda o script.
    case "$(uname -s)" in
        Linux|Darwin)
            cargo test --workspace -- \
                --skip root_help_snapshot \
                --skip query_run_help_snapshot \
                --skip replay_database_pgpass < /dev/null
            ;;
        *)
            cargo test --workspace < /dev/null
            ;;
    esac
fi

if [ "$no_push" -eq 1 ]; then
    echo
    echo "Branch criada: $rel_branch"
    echo 'Tag NÃO criada.'
    echo "Para abrir o PR manualmente: git push -u origin $rel_branch"
    print_tag_steps
    exit 0
fi

if [ "$yes" -eq 0 ]; then
    printf 'Empurrar a branch e abrir o PR? [s/N] '
    read -r answer
    case "$answer" in
        s|S|sim|SIM|y|Y|yes|YES) ;;
        *) echo 'branch não empurrada.'; print_tag_steps; exit 0 ;;
    esac
fi

git push -u origin "$rel_branch"

if command -v gh >/dev/null 2>&1; then
    body=$(cat <<EOF
Bump da versão para $version.

Base do PR: $branch -> main.

Inclui:
- Cargo.toml, Cargo.lock e dependências internas;
- docs (contrato, tutorial e how-to de release);
- sbom.cdx.json da raiz;
- fallbacks do instalador.

A tag v$version NÃO é criada aqui; ela será criada após o merge do PR.
EOF
)
    gh pr create --base main --head "$rel_branch" \
        --title "Bump Mustel to $version" --body "$body"
else
    echo "gh não encontrado. Abra o PR de $rel_branch para main no GitHub."
fi

echo
echo "Branch: $rel_branch"
echo "Commit: $(git rev-parse --short HEAD)"
echo "Tag:    não criada (crie v$version após o merge do PR)"
print_tag_steps
