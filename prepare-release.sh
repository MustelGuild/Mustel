#!/bin/sh
# Prepara o bump de versão do Mustel para um PR, sem criar a tag.
#
# Uso:
#   ./prepare-release.sh              # pergunta a versão
#   ./prepare-release.sh 1.2.0        # versão direta
#   ./prepare-release.sh 1.2.0 --yes  # sem confirmação
#   ./prepare-release.sh 1.2.0 --no-push --skip-tests
#
# O fluxo é idempotente: se parar em qualquer etapa, rode de novo com a mesma
# versão. Ele retoma a branch release/v<versão> e continua do estado atual.
# Os checks (fmt, clippy e testes) rodam ANTES do bump/commit. A tag NÃO é
# criada: ela é criada depois do merge do PR, na main.
set -eu

usage() {
    cat <<'EOF'
Uso: ./prepare-release.sh [versão] [opções]

  versão        X.Y.Z (se omitida, o script pergunta)
  --yes, -y     não pede confirmação
  --no-push     para depois do commit; não empurra nem abre PR
  --skip-tests  não roda cargo test (fmt e clippy continuam)
  --help, -h    mostra esta ajuda

O script cria ou retoma a branch release/v<versão>, roda fmt/clippy/testes
antes do bump, aplica o bump com `cargo release --no-tag`, atualiza
docs/CHANGELOG/SBOM e (opcionalmente) empurra a branch e abre o PR.
A tag v<versão> NÃO é criada; crie-a depois do merge do PR.

Se parar em qualquer etapa, rode de novo com a mesma versão: o script detecta
a branch e a versão já aplicada e continua. Os próprios scripts e o
cliff.toml/CHANGELOG.md precisam estar commitados antes da execução.
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

get_version() {
    value=$(cargo pkgid -p mustel 2>/dev/null | sed 's/.*#//')
    [ -n "$value" ] || fail 'não consegui obter a versão de cargo pkgid -p mustel'
    printf '%s\n' "$value"
}

set_version_in_file() {
    file=$1
    pattern=$2
    replacement=$3

    if [ ! -f "$file" ]; then
        echo "aviso: arquivo não encontrado: $file" >&2
        return 0
    fi

    tmp=$(mktemp)
    sed -E "s|$pattern|$replacement|g" "$file" > "$tmp"
    chmod 644 "$tmp"
    mv "$tmp" "$file"
    echo "versão verificada: $file"
}

run_gate() {
    echo
    echo '==> cargo fmt --all --check (antes do bump)'
    cargo fmt --all -- --check || fail 'cargo fmt falhou'

    echo
    echo '==> cargo clippy (antes do bump)'
    cargo clippy --workspace --all-targets --all-features -- -D warnings || fail 'cargo clippy falhou'

    if [ "$skip_tests" -eq 1 ]; then
        echo 'cargo test pulado (--skip-tests)' >&2
        return 0
    fi

    echo
    echo '==> cargo test (gate; stdin em /dev/null para os testes sem terminal)'
    # Os testes de ui_primitives exigem ausência de TTY. Redirecionar a entrada
    # evita que herdem o terminal de quem roda o script.
    case "$(uname -s)" in
        Linux|Darwin)
            cargo test --workspace -- \
                --skip root_help_snapshot \
                --skip query_run_help_snapshot \
                --skip replay_database_pgpass < /dev/null || fail 'cargo test falhou'
            ;;
        *)
            cargo test --workspace < /dev/null || fail 'cargo test falhou'
            ;;
    esac
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
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail 'não estamos num repositório Git'
repo=$(git rev-parse --show-toplevel)
cd "$repo"

cargo release --version >/dev/null 2>&1 || fail \
    "cargo-release não encontrado; instale com: cargo install cargo-release --version =1.1.7 --locked"

current_branch=$(git symbolic-ref --short HEAD 2>/dev/null || true)
[ -n "$current_branch" ] || fail 'HEAD destacado; volte para um branch antes de preparar a release'

if [ -z "$version" ]; then
    printf 'Versão a lançar (X.Y.Z): '
    read -r version
fi
printf '%s' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || fail \
    "versão inválida: '$version' (use X.Y.Z estável; o workflow de release só aceita vX.Y.Z)"

rel_branch="release/v$version"
branch_exists=0
if git show-ref --verify --quiet "refs/heads/$rel_branch"; then
    branch_exists=1
fi

if [ "$current_branch" != "$rel_branch" ]; then
    dirty=$(git status --porcelain)
    if [ -n "$dirty" ]; then
        fail "a árvore precisa estar limpa para trocar para $rel_branch. Mudanças pendentes:
$dirty"
    fi
    if [ "$branch_exists" -eq 1 ]; then
        echo "retomando a release existente: $rel_branch"
        git switch "$rel_branch"
    else
        if [ "$current_branch" != "main" ]; then
            echo "aviso: a branch de release parte de '$current_branch', não de main; o PR para main vai incluir os commits dela." >&2
        fi
        echo "criando a branch de release: $rel_branch"
        git switch -c "$rel_branch"
    fi
else
    echo "retomando a release existente: $rel_branch"
fi

current=$(get_version)
if [ "$current" = "$version" ]; then
    bump_done=1
    bump_pending=0
elif version_gt "$current" "$version"; then
    bump_done=0
    bump_pending=1
else
    fail "a versão alvo ($version) é menor que a atual ($current)"
fi

dirty=$(git status --porcelain)
if [ -n "$dirty" ]; then
    if [ "$bump_pending" -eq 1 ]; then
        fail "antes do bump, a árvore precisa estar limpa. Mudanças pendentes:
$dirty"
    fi
    echo 'aviso: há mudanças não commitadas; só docs/CHANGELOG/SBOM do release serão commitados; as demais ficam fora do PR.' >&2
fi

echo
echo "Versão atual: $current"
echo "Nova versão:  $version"
echo "Branch:       $rel_branch"
if [ "$bump_done" -eq 1 ]; then
    echo 'Bump:         já aplicado (retomando)'
else
    echo 'Bump:         pendente'
fi
echo 'Tag:          NÃO será criada (após o merge do PR)'
if [ "$yes" -eq 0 ]; then
    printf 'Continuar? [s/N] '
    read -r answer
    case "$answer" in
        s|S|sim|SIM|y|Y|yes|YES) ;;
        *) echo 'cancelado.'; exit 1 ;;
    esac
fi

run_gate

if [ "$bump_pending" -eq 1 ]; then
    echo
    echo '==> Ensaio do cargo release'
    cargo release "$version" --workspace --allow-branch "$rel_branch" \
        --no-tag --no-push --no-publish --no-confirm || fail 'cargo release (ensaio) falhou'

    echo
    echo '==> Aplicando o bump (sem tag)'
    set +e
    cargo release "$version" --execute --workspace --allow-branch "$rel_branch" \
        --no-tag --no-push --no-publish --no-confirm
    release_rc=$?
    set -e
    now=$(get_version)
    if [ "$now" != "$version" ]; then
        fail "cargo release falhou (exit $release_rc) e a versão ficou $now"
    fi
    if [ "$release_rc" -ne 0 ]; then
        echo "aviso: cargo release retornou $release_rc, mas a versão ficou $version; seguindo para o commit." >&2
    fi
else
    echo
    echo "==> Bump já aplicado ($current); pulando cargo release"
fi

echo
echo '==> Ajustando a versão na documentação e nos fallbacks'
set_version_in_file 'docs/contrato-de-interface.md' \
    'hoje `mustel [0-9]+\.[0-9]+\.[0-9]+`' \
    'hoje `mustel '"$version"'`'
set_version_in_file 'docs/tutorials/primeiros-passos.md' \
    'algo como `mustel [0-9]+\.[0-9]+\.[0-9]+`' \
    'algo como `mustel '"$version"'`'
set_version_in_file 'docs/how-to/publicar-release.md' \
    'ex\.: `v[0-9]+\.[0-9]+\.[0-9]+`' \
    'ex.: `v'"$version"'`'
set_version_in_file 'installer/mustel.iss' \
    'MyAppVersion "[0-9]+\.[0-9]+\.[0-9]+"' \
    'MyAppVersion "'"$version"'"'
set_version_in_file 'installer/mustel.iss' \
    'packaging\\mustel-[0-9]+\.[0-9]+\.[0-9]+-x86_64-pc-windows-msvc' \
    'packaging\\mustel-'"$version"'-x86_64-pc-windows-msvc'
set_version_in_file 'installer/wine-test/README.md' \
    'packaging/mustel-[0-9]+\.[0-9]+\.[0-9]+-x86_64-pc-windows-msvc' \
    'packaging/mustel-'"$version"'-x86_64-pc-windows-msvc'

echo
echo '==> Verificando o CHANGELOG'
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
echo '==> Verificando o SBOM da raiz'
if [ -f sbom.cdx.json ] && grep -q "\"version\": \"$version\"" sbom.cdx.json; then
    echo "sbom.cdx.json já está na versão $version; mantendo"
else
    command -v cargo-cyclonedx >/dev/null 2>&1 || fail \
        "cargo-cyclonedx não encontrado; instale com: cargo install cargo-cyclonedx --version =0.5.9 --locked"
    cargo cyclonedx -f json --spec-version 1.5 --override-filename sbom-root.cdx --target all
    [ -f crates/mustel/sbom-root.cdx.json ] || fail 'SBOM não encontrado em crates/mustel/sbom-root.cdx.json'
    cp crates/mustel/sbom-root.cdx.json sbom.cdx.json
    rm -f crates/*/sbom-root.cdx.json
    grep -q "\"version\": \"$version\"" sbom.cdx.json || fail \
        "o SBOM da raiz não contém a versão $version"
fi

echo
echo '==> Incluindo docs, CHANGELOG e SBOM no commit'
git add -- docs/contrato-de-interface.md docs/tutorials/primeiros-passos.md \
    docs/how-to/publicar-release.md installer/mustel.iss \
    installer/wine-test/README.md CHANGELOG.md sbom.cdx.json
if ! git diff --cached --quiet; then
    head_subject=$(git log -1 --format=%s)
    staged=$(git diff --cached --name-only)
    if [ "$head_subject" = "Bump Mustel to $version" ]; then
        git commit --amend --no-edit
    elif printf '%s\n' "$staged" | grep -qx 'Cargo.toml'; then
        git commit -m "Bump Mustel to $version"
    else
        git commit -m "Prepara release v$version"
    fi
else
    echo 'nenhuma mudança extra para incluir'
fi

if [ "$skip_tests" -eq 0 ]; then
    echo
    echo '==> contract_decisions (links da documentação)'
    cargo test -p mustel-core --test contract_decisions || fail 'contract_decisions falhou'
fi

if [ "$no_push" -eq 1 ]; then
    echo
    echo "Branch: $rel_branch"
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

if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
    git push origin "$rel_branch"
else
    git push -u origin "$rel_branch"
fi

if command -v gh >/dev/null 2>&1; then
    if url=$(gh pr view "$rel_branch" --json url --jq .url 2>/dev/null); then
        echo "PR já existe: $url"
    else
        body=$(cat <<EOF
Bump da versão para $version.

Base do PR: $rel_branch -> main.

Inclui:
- Cargo.toml, Cargo.lock e dependências internas;
- docs, CHANGELOG e sbom.cdx.json;
- fallbacks do instalador.

A tag v$version NÃO é criada aqui; ela será criada após o merge do PR.
EOF
)
        gh pr create --base main --head "$rel_branch" \
            --title "Bump Mustel to $version" --body "$body"
    fi
else
    echo "gh não encontrado. Abra o PR de $rel_branch para main no GitHub."
fi

echo
echo "Branch: $rel_branch"
echo "Commit: $(git rev-parse --short HEAD)"
echo "Tag:    não criada (crie v$version após o merge do PR)"
print_tag_steps
