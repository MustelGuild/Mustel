#!/bin/sh
# Constrói a imagem Wine+Inno Setup e roda o smoke do instalador no container.
# Uso: sh installer/wine-test/run.sh
# Saída: target/wine-test/wine-test-setup.exe (ignorado pelo Git).
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../.." && pwd)
out="$repo/target/wine-test"

mkdir -p "$out"

# O hash padrão acompanha o INNO_URL padrão (6.7.4). Ao trocar a URL, passe também INNO_SHA256
# do asset; o build falha se o hash não casar.
podman build --security-opt label=disable \
    --build-arg INNO_URL="${INNO_URL:-https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe}" \
    --build-arg INNO_SHA256="${INNO_SHA256:-a549929319f9120716677c685535ca4671e4b937b5fdc87e2585f32d8eb30e8c}" \
    -t mustel-inno-wine "$here"
podman run --rm --security-opt label=disable \
    -v "$here:/test:ro" \
    -v "$out:/out" \
    mustel-inno-wine bash /test/inside.sh
