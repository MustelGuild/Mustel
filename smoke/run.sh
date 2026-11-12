#!/usr/bin/env bash
# Smoke completo dentro do container Linux, sem abrir janela no desktop do dono.
#
# O CLI roda direto contra o serviço `postgres`; a GUI roda num Xvfb e as capturas ficam em
# /out/gui (montado em smoke/out/gui no host). A senha é gravada **só** no cofre efêmero deste
# container (gnome-keyring + D-Bus), nunca no cofre do dono; o pgpass também não é tocado.
set -uo pipefail

BIN=/tmp/target/debug/mustel
CONFIG=/tmp/mustel-smoke-config
OUT=/out
PASSED=0
FAILED=0

pass() {
    echo "PASS  $*"
    PASSED=$((PASSED + 1))
}

fail() {
    echo "FAIL  $*"
    FAILED=$((FAILED + 1))
}

# check <nome> <comando...>: passa se o comando sair 0; mostra a saída quando falha.
check() {
    local name="$1"
    shift
    if "$@" >/tmp/check.out 2>&1; then
        pass "$name"
    else
        fail "$name"
        sed 's/^/      /' /tmp/check.out
    fi
}

csv_lines() {
    tr -d '\r' <"$1"
}

mkdir -p "$OUT" "$OUT/gui"
rm -rf "$CONFIG" "$OUT/single" "$OUT/all" "$OUT/plain" "$OUT/escaped" "$OUT/backup" "$OUT/restored"
mkdir -p "$CONFIG"
export MUSTEL_CONFIG_DIR="$CONFIG"
unset MUSTEL_PASSWORD

echo "== CA do Postgres"
CA_FILE=/usr/local/share/ca-certificates/mustel-smoke.crt
openssl s_client -starttls postgres -connect postgres:5432 -servername postgres -showcerts \
    </dev/null 2>/tmp/s_client.log \
    | awk '/BEGIN CERTIFICATE/{n++; buf[n]=""} n{buf[n]=buf[n] $0 "\n"} END{if(n) printf "%s", buf[n]}' \
    >"$CA_FILE"
if [ -s "$CA_FILE" ] && update-ca-certificates >/tmp/update-ca.log 2>&1; then
    pass "CA do Postgres instalada no store"
else
    fail "CA do Postgres instalada no store"
    cat /tmp/s_client.log /tmp/update-ca.log 2>/dev/null
fi

echo "== cofre efêmero do container"
rm -rf /root/.local/share/keyrings
mkdir -p /root/.local/share/keyrings
dbus_env=$(dbus-launch --sh-syntax 2>/tmp/dbus.log)
eval "$dbus_env"
if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then pass "sessão D-Bus"; else fail "sessão D-Bus"; fi
keyring_env=$(printf 'smoke' | gnome-keyring-daemon --unlock --components=secrets 2>/tmp/keyring.log)
keyring_code=$?
eval "$keyring_env"
if [ "$keyring_code" -eq 0 ]; then
    pass "cofre do container desbloqueado"
else
    fail "cofre do container desbloqueado"
    cat /tmp/keyring.log
fi

echo "== compilando o binário (Linux, debug)"
if cargo build -p mustel --features gui --locked >/tmp/build.log 2>&1; then
    pass "build do binário"
else
    fail "build do binário"
    tail -40 /tmp/build.log
    echo "PASSED=$PASSED FAILED=$FAILED" | tee "$OUT/summary.txt"
    exit 1
fi
[ -x "$BIN" ] && pass "binário existe" || fail "binário existe"

echo "== cadastro e conexão"
out=$("$BIN" settings db-servers add -n Smoke -H postgres -p 5432 -U postgres -d mustel_001 \
    -W smoke --ssl-mode Prefer 2>&1)
code=$?
[ "$code" -eq 0 ] && pass "settings db-servers add" || fail "settings db-servers add: $out"

out=$("$BIN" settings db-servers ls 2>&1)
if [ $? -eq 0 ] && grep -q 'Smoke' <<<"$out"; then
    pass "settings db-servers ls mostra Smoke"
else
    fail "settings db-servers ls mostra Smoke: $out"
fi

echo "== settings db-servers test (cofre)"
out=$("$BIN" settings db-servers test -n Smoke 2>&1)
code=$?
if [ "$code" -eq 0 ]; then
    pass "settings db-servers test usa o cofre"
else
    fail "settings db-servers test usa o cofre: $out"
fi

echo "== query run: um banco"
out=$("$BIN" query run -c 'select count(*) as n from items' -o "$OUT/single" 2>&1)
code=$?
[ "$code" -eq 0 ] && pass "query run (um banco) exit 0" || fail "query run (um banco): $out"
csv=$(find "$OUT/single" -name 'consolidated_*.csv' | head -1)
if [ -n "$csv" ]; then pass "CSV consolidado existe"; else fail "CSV consolidado existe"; fi
if [ -n "$csv" ] && csv_lines "$csv" | grep -Fxq 'Smoke,mustel_001,3'; then
    pass "CSV tem mustel_001 com 3"
else
    fail "CSV tem mustel_001 com 3"
fi

echo "== query run: todas as bases (--all)"
out=$("$BIN" query run -a -c 'select count(*) as n from items' -o "$OUT/all" 2>&1)
code=$?
[ "$code" -eq 0 ] && pass "query run --all exit 0" || fail "query run --all: $out"
if grep -qE 'Success:[[:space:]]+103' <<<"$out"; then
    pass "resumo com 103 sucessos"
else
    fail "resumo com 103 sucessos: $out"
fi
all_csv=$(find "$OUT/all" -name 'consolidated_*.csv' | head -1)
for db in mustel_100 my-db 'Meu Banco' '..\hostile'; do
    if [ -n "$all_csv" ] && csv_lines "$all_csv" | grep -Fq ",${db},3"; then
        pass "CSV inclui $db"
    else
        fail "CSV inclui $db"
    fi
done

echo "== CSV: fórmulas"
"$BIN" query run -d mustel_001 -c 'select value from formulas order by id' \
    -o "$OUT/plain" >/tmp/plain.log 2>&1
plain_csv=$(find "$OUT/plain" -name 'consolidated_*.csv' | head -1)
if [ -n "$plain_csv" ] && csv_lines "$plain_csv" | grep -Fxq 'Smoke,mustel_001,=1+1'; then
    pass "CSV default mantém =1+1 (sem prefixo)"
else
    fail "CSV default mantém =1+1 (sem prefixo)"
fi

"$BIN" query run -d mustel_001 --csv-escape-formulas \
    -c 'select value from formulas order by id' -o "$OUT/escaped" >/tmp/escaped.log 2>&1
escaped_csv=$(find "$OUT/escaped" -name 'consolidated_*.csv' | head -1)
for line in "Smoke,mustel_001,'=1+1" "Smoke,mustel_001,'@SUM(A1:A2)" \
    "Smoke,mustel_001,'-2+3" "Smoke,mustel_001,'+cmd|'/c calc'!A0"; do
    if [ -n "$escaped_csv" ] && csv_lines "$escaped_csv" | grep -Fxq "$line"; then
        pass "escape presente: $line"
    else
        fail "escape presente: $line"
    fi
done

echo "== TLS"
"$BIN" query run -c 'select 1' --ssl-mode Require -o "$OUT/tls-require" >/tmp/require.log 2>&1
[ $? -eq 0 ] && pass "Require conecta com o CA confiado" ||
    fail "Require conecta com o CA confiado"
"$BIN" query run -c 'select 1' --ssl-mode Disable -o "$OUT/tls-disable" >/dev/null 2>&1
[ $? -eq 0 ] && pass "ssl-mode Disable conecta" || fail "ssl-mode Disable conecta"
"$BIN" query run -c 'select 1' --ssl-mode Prefer -o "$OUT/tls-prefer" >/dev/null 2>&1
[ $? -eq 0 ] && pass "ssl-mode Prefer conecta" || fail "ssl-mode Prefer conecta"

wrong_ad_hoc='Host=postgres-wrong;Port=5432;Database=mustel_001;Username=postgres'
"$BIN" query run --npgsql-connection-string "$wrong_ad_hoc" -W smoke \
    -c 'select 1' --ssl-mode Require -o "$OUT/tls-wrong-host" >/tmp/wrong-host.log 2>&1
code=$?
err_csv=$(find "$OUT/tls-wrong-host" -name '*_erros.csv' | head -1)
if [ "$code" -ne 0 ] && { [ -z "$err_csv" ] || grep -Eqi 'certificate|hostname|TLS handshake' "$err_csv"; }; then
    pass "Require rejeita hostname fora do SAN"
else
    fail "Require rejeita hostname fora do SAN: $code"
fi

# Sem o CA, Prefer/Require precisam falhar; o Prefer não pode cair para texto puro.
mv "$CA_FILE" /tmp/mustel-smoke-ca.crt
update-ca-certificates --fresh >/tmp/update-ca-fresh.log 2>&1
"$BIN" query run -c 'select 1' --ssl-mode Require -o "$OUT/tls-require-no-ca" \
    >/tmp/require-no-ca.log 2>&1
code=$?
err_csv=$(find "$OUT/tls-require-no-ca" -name '*_erros.csv' | head -1)
if [ "$code" -eq 1 ] && { [ -z "$err_csv" ] || grep -qi 'certificate\|TLS' "$err_csv"; }; then
    pass "Require falha sem o CA"
else
    fail "Require falha sem o CA: $code"
fi
"$BIN" query run -c 'select 1' --ssl-mode Prefer -o "$OUT/tls-prefer-no-ca" \
    >/tmp/prefer-no-ca.log 2>&1
code=$?
if [ "$code" -eq 1 ]; then
    pass "Prefer falha sem o CA (não cai para texto puro)"
else
    fail "Prefer falha sem o CA (não cai para texto puro): $code"
fi
"$BIN" settings db-servers test -n Smoke >/tmp/test-no-ca.log 2>&1
code=$?
[ "$code" -eq 40 ] && pass "settings test sem o CA sai 40" ||
    fail "settings test sem o CA sai 40: $code"
mv /tmp/mustel-smoke-ca.crt "$CA_FILE"
update-ca-certificates >/tmp/update-ca-restore.log 2>&1

# Serviço sem TLS: Prefer cai em silêncio; Require falha.
ad_hoc='Host=postgres-notls;Port=5432;Database=smoke_notls;Username=postgres'
"$BIN" query run --npgsql-connection-string "$ad_hoc" -W smoke \
    -c 'select 1' --ssl-mode Prefer -o "$OUT/tls-prefer-notls" >/tmp/prefer-notls.log 2>&1
code=$?
if [ "$code" -eq 0 ] && ! grep -qi 'without TLS' /tmp/prefer-notls.log; then
    pass "Prefer sem TLS cai sem aviso"
else
    fail "Prefer sem TLS cai sem aviso: $code"
fi
"$BIN" query run --npgsql-connection-string "$ad_hoc" -W smoke \
    -c 'select 1' --ssl-mode Require -o "$OUT/tls-require-notls" >/tmp/require-notls.log 2>&1
code=$?
err_csv=$(find "$OUT/tls-require-notls" -name '*_erros.csv' | head -1)
if [ "$code" -eq 1 ] && { [ -z "$err_csv" ] || grep -Eqi 'does not support TLS|TLS handshake' "$err_csv"; }; then
    pass "Require falha sem TLS"
else
    fail "Require falha sem TLS: $code"
fi
echo "== sslMode inválido no cadastro"
config_file="$CONFIG/mustel.jsonc"
cp "$config_file" /tmp/mustel.jsonc.bak
sed -i 's/"sslMode": "Prefer"/"sslMode": "Requiree"/' "$config_file"
out=$("$BIN" query run -c 'select 1' -o "$OUT/invalid-ssl" 2>&1)
code=$?
if grep -q 'Requiree' "$config_file" && [ "$code" -ne 0 ] && grep -q 'invalid ssl mode' <<<"$out"; then
    pass "sslMode inválido é erro, não Prefer"
else
    fail "sslMode inválido é erro, não Prefer: $out"
fi
cp /tmp/mustel.jsonc.bak "$config_file"

echo "== usuário sem superusuário"
"$BIN" query run -U smoke_app -W smoke_app -d mustel_001 \
    -c 'select count(*) as n from items' -o "$OUT/app" >/tmp/app.log 2>&1
[ $? -eq 0 ] && pass "smoke_app lê items" || fail "smoke_app lê items"

echo "== backup e restore"
common=(-H postgres -p 5432 -U postgres --ssl-mode Disable)
blocked_common=(-H postgres -p 5432 -U postgres)
mkdir -p "$OUT/backup"

"$BIN" database backup mustel_001 -o "$OUT/backup/tls-blocked" --ssl-mode Require \
    "${blocked_common[@]}" >/tmp/tls-blocked.log 2>&1
code=$?
if [ "$code" -eq 2 ] && grep -q 'TLS is not supported' /tmp/tls-blocked.log; then
    pass "backup recusa Require nesta release"
else
    fail "backup recusa Require nesta release: exit=$code"
fi
one="$OUT/backup/mustel_001.dump"
"$BIN" database backup mustel_001 -o "$one" "${common[@]}" >/tmp/backup.log 2>&1
if [ $? -eq 0 ] && [ -s "$one" ]; then pass "backup de um banco"; else fail "backup de um banco"; fi
mode=$(stat -c '%a' "$one" 2>/dev/null)
[ "$mode" = "600" ] && pass "dump 0600" || fail "dump mode $mode"

"$BIN" database backup -a -o "$OUT/backup/all" "${common[@]}" >/tmp/backup-all.log 2>&1
code=$?
dumps=$(find "$OUT/backup/all" -name '*.dump' 2>/dev/null | wc -l)
if [ "$code" -eq 0 ] && [ "$dumps" -ge 100 ]; then
    pass "backup --all ($dumps dumps)"
else
    fail "backup --all: exit=$code dumps=$dumps"
fi
mode=$(stat -c '%a' "$OUT/backup/all" 2>/dev/null)
[ "$mode" = "700" ] && pass "diretório do backup --all 0700" || fail "diretório do backup --all mode $mode"
if find "$OUT" -name 'hostile.dump' | grep -q .; then
    fail "nome hostil não escapou do diretório"
else
    pass "nome hostil não escapou do diretório"
fi

out=$("$BIN" database backup -a --strict -o "$OUT/backup/strict" "${common[@]}" 2>&1)
code=$?
if [ "$code" -ne 0 ] && grep -q 'database(s) failed' <<<"$out"; then
    pass "--strict falha com banco hostil"
else
    fail "--strict falha com banco hostil: $out"
fi

db="smoke_restored_$(tr -dc 'a-f0-9' </dev/urandom | head -c 8)"
"$BIN" database restore "$db" "$one" "${common[@]}" --create-if-not-exists --no-clean \
    >/tmp/restore.log 2>&1
if [ $? -eq 0 ]; then pass "restore cria e restaura"; else fail "restore cria e restaura"; fi

cp "$config_file" /tmp/mustel.jsonc.before-restore
sed -i "s/\"mustel_001\"/\"$db\"/" "$config_file"
"$BIN" query run -c 'select count(*) as n from items' -o "$OUT/restored" \
    >/tmp/restored.log 2>&1
restored_csv=$(find "$OUT/restored" -name 'consolidated_*.csv' | head -1)
if [ -n "$restored_csv" ] && csv_lines "$restored_csv" | grep -Fq ",${db},3"; then
    pass "consulta no banco restaurado"
else
    fail "consulta no banco restaurado"
fi
cp /tmp/mustel.jsonc.before-restore "$config_file"

echo "== GUI headless (Xvfb)"
mkdir -p /tmp/xdg "$OUT/gui"
chmod 700 /tmp/xdg
export XDG_RUNTIME_DIR=/tmp/xdg
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
Xvfb :99 -screen 0 1440x900x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
sleep 2
( cd /tmp && exec "$BIN" gui >/tmp/gui.log 2>&1 ) &
GUI_PID=$!

# --- OCR: a captura é a única prova do que a janela desenhou ----------------------------------
# O tema pode ser escuro (texto claro); o `tesseract` espera texto escuro em fundo claro, então a
# imagem é normalizada e, quando o fundo é escuro, negada.
shot() { import -display :99 -window root "$1" 2>/dev/null; }
# O texto da janela é pequeno; dobrar a imagem antes do OCR é o que faz o `tesseract` enxergar
# rótulos de botão e de linha. As coordenadas devolvidas por `word_at` voltam na escala original.
OCR_SCALE=2
ocr_image() {
    local img="$1" out="${1%.*}.ocr.png" mean
    mean=$(convert "$img" -colorspace Gray -format '%[fx:mean*100]' info: 2>/dev/null)
    if [ "${mean%%.*}" -lt 50 ] 2>/dev/null; then
        convert "$img" -colorspace Gray -negate -auto-level -resize 200% "$out" 2>/dev/null
    else
        convert "$img" -colorspace Gray -auto-level -resize 200% "$out" 2>/dev/null
    fi
    printf '%s\n' "$out"
}
ocr_text() { tesseract "$(ocr_image "$1")" stdout --psm 3 2>/dev/null; }
# `word_at <png> <regex>` devolve "x y" do centro da primeira palavra que casa, na escala da tela.
word_at() {
    tesseract "$(ocr_image "$1")" stdout --psm 3 -c tessedit_create_tsv=1 2>/dev/null |
        awk -F'\t' -v pat="$2" -v scale="$OCR_SCALE" 'NR > 1 && $12 != "" && tolower($12) ~ tolower(pat) {
            printf "%d %d\n", ($7 + $9 / 2) / scale, ($8 + $10 / 2) / scale; exit
        }'
}
click_at() { xdotool mousemove --sync "$1" "$2" click 1 2>/dev/null; }
click_word() {
    local png="$1" pattern="$2" dx="${3:-0}" dy="${4:-0}" pos
    pos=$(word_at "$png" "$pattern")
    [ -n "$pos" ] || return 1
    click_at $((${pos% *} + dx)) $((${pos#* } + dy))
}
# Espera um texto aparecer na tela (a GUI é assíncrona: descoberta, consulta, job).
wait_for_text() {
    local pattern="$1" limit="${2:-30}" i
    for i in $(seq 1 "$limit"); do
        shot /tmp/gui.png
        if ocr_text /tmp/gui.png | grep -qi -- "$pattern"; then return 0; fi
        sleep 1
    done
    ocr_text /tmp/gui.png >/tmp/gui-ocr.txt 2>/dev/null
    return 1
}

wid=""
for _ in $(seq 1 40); do
    wid=$(xdotool search --name '^Mustel$' 2>/dev/null | head -1)
    [ -n "$wid" ] && break
    sleep 1
done
if [ -n "$wid" ]; then
    pass "janela da GUI abriu"
    xdotool windowsize "$wid" 1440 900 2>/dev/null
    xdotool windowmove "$wid" 0 0 2>/dev/null
    # Sem window manager no Xvfb, o foco de teclado não vai sozinho para a janela.
    xdotool windowfocus "$wid" 2>/dev/null
    sleep 2

    # 1. A descoberta terminou e o status diz quantas bases o servidor tem.
    if wait_for_text 'database(s)' 90; then
        text=$(ocr_text /tmp/gui.png)
        if grep -qi 'Smoke' <<<"$text"; then
            pass "GUI mostra a descoberta de bases"
        else
            fail "GUI mostra a descoberta de bases (sem 'Smoke' no status)"
        fi
    else
        fail "GUI mostra a descoberta de bases"
        cp /tmp/gui.png "$OUT/gui/00-sem-descoberta.png"
    fi
    shot "$OUT/gui/01-query.png"

    # 2. Marca `mustel_001` na sidebar (a lista já abre nas primeiras bases) e confere o contador,
    # que é o que habilita o `Run query`. O clique vai no quadrado do checkbox, à esquerda do nome.
    if click_word "$OUT/gui/01-query.png" 'mustel.001' -18 0; then
        sleep 1
        shot /tmp/gui.png
        if ocr_text /tmp/gui.png | grep -qi 'marked'; then
            pass "marca mustel_001 na sidebar"
        else
            fail "marca mustel_001 na sidebar"
            cp /tmp/gui.png "$OUT/gui/02-marcacao.png"
            ocr_text /tmp/gui.png | head -6 | sed 's/^/      /'
        fi
    else
        fail "marca mustel_001 na sidebar (a linha não estava visível)"
    fi

    # 3. Digita a consulta no editor e clica em `Run query`.
    shot /tmp/gui.png
    click_word /tmp/gui.png '^SQL$' 60 30 || fail "acha o editor SQL"
    sleep 1
    xdotool key --clearmodifiers ctrl+a 2>/dev/null
    # O resultado é `42`; o texto digitado (`6*7`) **não** contém 42 — o OCR não pode passar lendo o
    # próprio editor (era o falso-positivo do check antigo, com `smoke-ok`).
    xdotool type --delay 20 "select 6*7 as answer from items limit 1" 2>/dev/null
    sleep 1
    shot /tmp/gui.png
    if click_word /tmp/gui.png '^Run$'; then
        if wait_for_text '42' 120; then
            pass "consulta roda pela janela e mostra o resultado"
        else
            fail "consulta roda pela janela e mostra o resultado"
            cp /tmp/gui.png "$OUT/gui/03-sem-resultado.png"
        fi
        gui_csv=$(find /tmp/results -name 'consolidated_*.csv' 2>/dev/null | head -1)
        if [ -n "$gui_csv" ] && csv_lines "$gui_csv" | grep -Fxq 'Smoke,mustel_001,42'; then
            pass "GUI gravou o CSV da consulta"
        else
            fail "GUI gravou o CSV da consulta"
        fi
    else
        fail "acha o botão 'Run query'"
    fi

    # 4. As abas trocam a tela e o conteúdo de cada uma é o esperado.
    shot /tmp/gui.png
    if click_word /tmp/gui.png 'Connections'; then
        sleep 2
        shot "$OUT/gui/02-connections.png"
        if ocr_text "$OUT/gui/02-connections.png" | grep -qi 'Add server'; then
            pass "aba Connections mostra o cadastro de servidores"
        else
            fail "aba Connections mostra o cadastro de servidores"
        fi
    else
        fail "acha a aba Connections"
    fi

    shot /tmp/gui.png
    if click_word /tmp/gui.png '^Settings$'; then
        sleep 2
        shot "$OUT/gui/03-settings.png"
        if ocr_text "$OUT/gui/03-settings.png" | grep -qi 'Appearance'; then
            pass "aba Settings mostra a aparência"
        else
            fail "aba Settings mostra a aparência"
        fi
    else
        fail "acha a aba Settings"
    fi

    # 5. Voltar para Query preserva a consulta e o resultado.
    shot /tmp/gui.png
    if click_word /tmp/gui.png '^Query'; then
        sleep 2
        shot "$OUT/gui/04-de-volta.png"
        if ocr_text "$OUT/gui/04-de-volta.png" | grep -q '42'; then
            pass "voltar para Query preserva o resultado"
        else
            fail "voltar para Query preserva o resultado"
        fi
    else
        fail "acha a aba Query"
    fi
else
    fail "janela da GUI abriu"
    cp /tmp/gui.log "$OUT/gui/gui.log" 2>/dev/null
    tail -30 /tmp/gui.log
fi

kill "$GUI_PID" 2>/dev/null
wait "$GUI_PID" 2>/dev/null
kill "$XVFB_PID" 2>/dev/null
wait "$XVFB_PID" 2>/dev/null

# Diagnóstico: o texto que o OCR leu da última captura da GUI.
[ -f /tmp/gui-ocr.txt ] && cp /tmp/gui-ocr.txt "$OUT/gui/ultima-tela.txt" 2>/dev/null

echo ""
echo "PASSED=$PASSED FAILED=$FAILED"
{
    echo "PASSED=$PASSED FAILED=$FAILED"
    echo "binário: $("$BIN" --version 2>/dev/null)"
} >"$OUT/summary.txt"

[ "$FAILED" -eq 0 ]
