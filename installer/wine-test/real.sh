#!/bin/sh
# Testa o instalador REAL (installer/mustel.iss) no container Wine.
#
# Uso:
#   sh installer/wine-test/real.sh [stage]
#
# `stage` é o diretório que o publish.ps1 gera (packaging\mustel-<versão>-<triple>) ou um
# stage sintético com os arquivos. O padrão é target/installer-stage.
# O setup de teste é compilado com /DWineTest=1 e instalado num prefixo Wine win32; o release
# Windows continua x64 (o .iss não define WineTest).
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../.." && pwd)
stage=${1:-"$repo/target/installer-stage"}
out="$repo/target/installer-build"

version=${VERSION:-$(cargo pkgid -p mustel | sed 's/.*#//')}
triple=${TRIPLE:-x86_64-pc-windows-msvc}

if [ ! -d "$stage" ]; then
    echo "stage não encontrado: $stage" >&2
    echo "passe o stage do publish.ps1 ou um stage sintético (ex.: target/installer-stage)" >&2
    exit 1
fi
# O podman exige caminho absoluto no -v; um caminho relativo viraria volume nomeado.
stage=$(CDPATH= cd -- "$stage" && pwd)
if [ -z "$version" ]; then
    echo 'não consegui obter a versão de cargo pkgid -p mustel' >&2
    exit 1
fi

mkdir -p "$out"

# O hash padrão acompanha o INNO_URL padrão (6.7.4). Ao trocar a URL, passe também INNO_SHA256
# do asset; o build falha se o hash não casar.
podman build --security-opt label=disable \
    --build-arg "INNO_URL=${INNO_URL:-https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe}" \
    --build-arg "INNO_SHA256=${INNO_SHA256:-a549929319f9120716677c685535ca4671e4b937b5fdc87e2585f32d8eb30e8c}" \
    -t mustel-inno-wine "$here"
podman run --rm --security-opt label=disable \
    -v "$repo:/src:ro" \
    -v "$stage:/stage:ro" \
    -v "$out:/out" \
    -e "VERSION=$version" \
    -e "TRIPLE=$triple" \
    mustel-inno-wine bash /src/installer/wine-test/real-inside.sh
