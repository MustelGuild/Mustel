#!/usr/bin/env bash
# Smoke em container: descobre o ambiente e escolhe o runtime.
#
#   Windows            -> `docker-smoke.ps1` (PowerShell + Docker)
#   Linux com podman   -> podman nativo, sem `podman-compose` (o caso do Bazzite)
#   Linux com Docker   -> `docker compose`
#
# Sobe o Postgres 18.6 (TLS + sem TLS) e o runner Linux, roda CLI e GUI dentro dos containers
# (Xvfb) e destrói tudo. Nenhuma janela abre no desktop do dono.
#
# Uso:
#   ./smoke/container-smoke.sh           # roda e limpa
#   ./smoke/container-smoke.sh --keep    # preserva containers, rede e volume
set -euo pipefail

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/.." && pwd)
out="$here/out"
compose="$here/docker-compose.yml"

net=mustel-smoke
pg=mustel-smoke-postgres
pg_notls=mustel-smoke-postgres-notls
runner=mustel-smoke-runner
volume=mustel-smoke-target
pg_image=mustel-smoke-postgres:local
runner_image=mustel-smoke-runner:local
notls_image=docker.io/library/postgres:18.6@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6

keep=0
for arg in "$@"; do
    case "$arg" in
        --keep|-Keep) keep=1 ;;
        -h|--help)
            cat <<'EOF'
Smoke em container do Mustel.

Uso:
  ./smoke/container-smoke.sh           # roda, mostra PASS/FAIL e limpa
  ./smoke/container-smoke.sh --keep    # preserva containers, rede e volume

O script escolhe o runtime pelo ambiente: no Windows delega para o docker-smoke.ps1;
no Linux usa podman direto (Bazzite/Fedora atomic) ou, na falta dele, docker compose.
As capturas da GUI ficam em smoke/out/gui.
EOF
            exit 0
            ;;
        *) echo "argumento desconhecido: $arg" >&2; exit 2 ;;
    esac
done

finish() {
    local code=$1 runtime=$2
    echo
    if [ "$code" -eq 0 ]; then
        echo "SMOKE (container, $runtime) OK"
    else
        echo "SMOKE (container, $runtime) FALHOU: exit $code"
    fi
    echo "capturas: $out/gui"
    exit "$code"
}

# Windows: o invólucro de PowerShell é o dono do caminho com Docker.
case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW*|MSYS*|CYGWIN*)
        ps=$(command -v powershell || command -v pwsh || true)
        [ -n "$ps" ] || { echo "PowerShell não encontrado no PATH" >&2; exit 1; }
        if [ "$keep" -eq 1 ]; then
            exec "$ps" -NoProfile -ExecutionPolicy Bypass -File "$here/docker-smoke.ps1" -Keep
        fi
        exec "$ps" -NoProfile -ExecutionPolicy Bypass -File "$here/docker-smoke.ps1"
        ;;
esac

run_docker() {
    echo "==> runtime: docker compose"
    docker compose -f "$compose" down -v --remove-orphans >/dev/null 2>&1 || true

    local code=0
    docker compose -f "$compose" up --build --abort-on-container-exit --exit-code-from smoke ||
        code=$?

    if [ "$keep" -eq 0 ]; then
        docker compose -f "$compose" down -v --remove-orphans >/dev/null 2>&1 || true
    fi

    finish "$code" docker
}

wait_healthy() {
    local name=$1 tries=$2 i status
    for ((i = 1; i <= tries; i++)); do
        status=$(podman inspect --format '{{.State.Health.Status}}' "$name" 2>/dev/null || true)
        case "$status" in
            healthy) return 0 ;;
            unhealthy) break ;;
        esac
        sleep 2
    done
    echo "container '$name' não ficou saudável; últimas linhas do log:" >&2
    podman logs --tail 40 "$name" >&2 2>/dev/null || true
    return 1
}

run_podman() {
    echo "==> runtime: podman (rootless, sem podman-compose)"
    mkdir -p "$out"

    # Num host com SELinux em enforcing, relabelar o repositório com `:Z` mexeria arquivos do dono.
    # Desligar a separação só deste container resolve sem tocar em label nenhum.
    local selinux=()
    if command -v getenforce >/dev/null 2>&1 && [ "$(getenforce 2>/dev/null)" = Enforcing ]; then
        echo "    SELinux em enforcing: usando --security-opt label=disable"
        selinux=(--security-opt label=disable)
    fi

    cleanup() {
        [ "$keep" -eq 1 ] && return 0
        podman rm -f "$runner" "$pg_notls" "$pg" >/dev/null 2>&1 || true
        podman volume rm -f "$volume" >/dev/null 2>&1 || true
        podman network rm -f "$net" >/dev/null 2>&1 || true
    }
    trap cleanup EXIT INT TERM

    # Mesma semântica do `docker compose down -v` no começo do docker-smoke.ps1.
    podman rm -f "$runner" "$pg_notls" "$pg" >/dev/null 2>&1 || true
    podman network rm -f "$net" >/dev/null 2>&1 || true
    podman volume rm -f "$volume" >/dev/null 2>&1 || true

    podman network create "$net" >/dev/null
    podman volume create "$volume" >/dev/null

    echo "==> imagens"
    podman build -t "$pg_image" -f "$here/Dockerfile" "$here"
    podman build -t "$runner_image" -f "$here/runner.Dockerfile" "$here"

    echo "==> postgres (TLS)"
    podman run -d --name "$pg" --network "$net" \
        --network-alias postgres --network-alias postgres-wrong \
        -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=smoke -e POSTGRES_DB=postgres \
        -p 55432:5432 \
        --health-cmd "test -f /tmp/smoke-init.done && pg_isready -U postgres -d postgres" \
        --health-interval 3s --health-timeout 5s --health-retries 40 --health-start-period 20s \
        "$pg_image" postgres -c ssl=on \
        -c ssl_cert_file=/etc/postgresql/server.crt \
        -c ssl_key_file=/etc/postgresql/server.key >/dev/null

    echo "==> postgres (sem TLS)"
    podman run -d --name "$pg_notls" --network "$net" --network-alias postgres-notls \
        -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=smoke -e POSTGRES_DB=smoke_notls \
        --health-cmd "pg_isready -U postgres -d postgres" \
        --health-interval 3s --health-timeout 5s --health-retries 40 \
        "$notls_image" postgres -c ssl=off >/dev/null

    wait_healthy "$pg" 90 || finish 1 podman
    wait_healthy "$pg_notls" 60 || finish 1 podman

    echo "==> runner"
    local code=0
    podman run --rm --name "$runner" --network "$net" \
        ${selinux[@]+"${selinux[@]}"} \
        -v "$repo:/src:ro" \
        -v "$out:/out" \
        -v "$volume:/tmp/target" \
        -e CARGO_TARGET_DIR=/tmp/target \
        -e DISPLAY=:99 \
        -e LIBGL_ALWAYS_SOFTWARE=1 \
        "$runner_image" || code=$?

    finish "$code" podman
}

if command -v podman >/dev/null 2>&1; then
    run_podman
elif command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    run_docker
else
    cat >&2 <<'EOF'
Nenhum runtime de containers foi encontrado.

- Bazzite / Fedora atomic: o `podman` já vem na imagem; rode este script direto.
- Debian/Ubuntu: instale docker + o plugin `docker compose`.
- Windows: use `smoke/docker-smoke.ps1` (PowerShell).
EOF
    exit 1
fi
