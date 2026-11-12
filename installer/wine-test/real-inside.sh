#!/bin/sh
# Roda dentro do container: compila o .iss real, instala, confere e desinstala.
# O prefixo é win32 (a imagem tem Wine 32-bit); /DWineTest=1 omite a checagem x64 só aqui.
set -eu

export WINEARCH=win32
export WINEDEBUG=-all

setup="Z:\\out\\mustel-$VERSION-$TRIPLE-setup.exe"
install_dir="$WINEPREFIX/drive_c/users/root/AppData/Local/Programs/Mustel"
appdata="$WINEPREFIX/drive_c/users/root/AppData/Roaming/Mustel"
localappdata="$WINEPREFIX/drive_c/users/root/AppData/Local/Mustel"

xvfb-run -a wineboot -i
xvfb-run -a wine 'C:\InnoSetup\ISCC.exe' \
    "/DMyAppVersion=$VERSION" \
    "/DMyAppVersion4=$VERSION.0" \
    "/DMyAppStage=Z:\stage" \
    "/DMyArch=$TRIPLE" \
    "/DMyOutputDir=Z:\out" \
    '/DWineTest=1' \
    'Z:\src\installer\mustel.iss'

xvfb-run -a wine "$setup" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-

test -f "$install_dir/mustel.exe"
test -f "$install_dir/mustel-gui.exe"
test -f "$install_dir/mustel.ico"
test -f "$install_dir/mustel.example.jsonc"
echo "instalado"

path_line=$(xvfb-run -a wine reg query 'HKCU\Environment' /v Path)
printf '%s\n' "$path_line" | grep -qF 'Programs\Mustel'
printf '%s\n' "$path_line" | grep -qF 'REG_EXPAND_SZ'
echo "PATH adicionado como REG_EXPAND_SZ"
echo "start menu:"
find "$WINEPREFIX/drive_c/users/root/AppData/Roaming/Microsoft/Windows/Start Menu/Programs" -type f -print || true
if [ -n "$(find "$WINEPREFIX/drive_c/users/root/AppData/Roaming/Microsoft/Windows/Start Menu/Programs" -name 'Mustel.lnk' -print -quit)" ]; then
    echo "atalho ok"
else
    echo "AVISO: Wine não criou o .lnk; validar o atalho no Windows (T8)"
fi

mkdir -p "$appdata" "$localappdata/logs"
printf 'user config\n' > "$appdata/mustel.jsonc"
printf 'user log\n' > "$localappdata/logs/activity.jsonl"
printf 'old\n' > "$install_dir/leftover.old"

xvfb-run -a wine 'C:\users\root\AppData\Local\Programs\Mustel\unins000.exe' /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-

test ! -f "$install_dir/mustel.exe"
test ! -f "$install_dir/leftover.old"
test -f "$appdata/mustel.jsonc"
test -f "$localappdata/logs/activity.jsonl"
if xvfb-run -a wine reg query 'HKCU\Environment' /v Path | grep -qF 'Programs\Mustel'; then
    echo "AVISO: PATH não removido pelo desinstalador no Wine; validar no Windows (T8)"
else
    echo "PATH removido"
fi
echo "desinstalado; dados preservados"
