#!/bin/sh
# Gera o pacote de release do Mustel para a plataforma atual (D26).
#
# Uso:
#   ./publish.sh              # build release + pacote em packaging/
#   ./publish.sh --no-build   # reempacota o binário já construído
#
# A **publicação é manual**: o script só produz o artefato e o SHA256. O equivalente no
# Windows é o `publish.ps1`, com o mesmo layout de saída.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$root"

build=1
for arg in "$@"; do
    case "$arg" in
        --no-build) build=0 ;;
        -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "argumento desconhecido: $arg" >&2; exit 2 ;;
    esac
done

command -v cargo >/dev/null 2>&1 || { echo "cargo não encontrado no PATH" >&2; exit 1; }

if [ "$build" -eq 1 ]; then
    echo "==> cargo build --release --locked -p mustel"
    cargo build --release --locked -p mustel
fi

version=$(cargo pkgid -p mustel | sed 's/.*#//')
triple=$(rustc -vV | sed -n 's/^host: //p')
[ -n "$version" ] && [ -n "$triple" ] || { echo "não consegui obter versão/triple" >&2; exit 1; }

stage="packaging/mustel-$version-$triple"
rm -rf "$stage"
mkdir -p "$stage"

cp "target/release/mustel" "$stage/"
cp mustel.example.jsonc "$stage/"
cp LICENSE "$stage/"
cat > "$stage/README.txt" <<EOF
Mustel $version ($triple)

1. Copie o binário 'mustel' para um diretório no PATH.
2. Copie 'mustel.example.jsonc' para o diretório de configuração e ajuste:
   Linux/macOS: \$XDG_CONFIG_HOME/Mustel/mustel.jsonc (ou ~/.config/Mustel/mustel.jsonc)
   Windows:     %APPDATA%\\Mustel\\mustel.jsonc
3. 'mustel --help' lista os comandos.

Dependências de sistema (a GUI usa egui/winit): 'libdbus-1.so.3' é a única
obrigatória para o binário iniciar — Debian/Ubuntu: 'sudo apt install libdbus-1-3'.
Para abrir a janela, o ambiente precisa também de X11 ou Wayland com OpenGL.
Quem usa só o CLI não precisa da janela, mas a libdbus continua necessária porque
é carregada na inicialização.

Licença: GPL-3.0-only (ver LICENSE).
EOF

archive="$stage.tar.gz"
tar -C packaging -czf "$archive" "$(basename "$stage")"

if command -v sha256sum >/dev/null 2>&1; then
    sum=$(sha256sum "$archive" | cut -d' ' -f1)
elif command -v shasum >/dev/null 2>&1; then
    sum=$(shasum -a 256 "$archive" | cut -d' ' -f1)
else
    echo "sem sha256sum/shasum: não gerei o checksum" >&2
    sum=""
fi

if [ -n "$sum" ]; then
    printf '%s  %s\n' "$sum" "$(basename "$archive")" > "$archive.sha256"
fi

echo
echo "pacote:    $archive"
[ -n "$sum" ] && echo "sha256:    $sum"
echo "publicação: manual (D26) — envie o arquivo acima onde precisar"
