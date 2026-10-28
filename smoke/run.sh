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

# Sem o CA, Prefer/Require precisam falhar; o Prefer não pode cair para texto puro.
mv "$CA_FILE" /tmp/mustel-smoke-ca.crt
update-ca-certificates --fresh >/tmp/update-ca-fresh.log 2>&1
"$BIN" query run -c 'select 1' --ssl-mode Require -o "$OUT/tls-require-no-ca" \
    >/tmp/require-no-ca.log 2>&1
code=$?
err_csv=$(find "$OUT/tls-require-no-ca" -name '*_erros.csv' | head -1)
if [ "$code" -ne 0 ] && { [ -z "$err_csv" ] || grep -qi 'certificate\|TLS' "$err_csv"; }; then
    pass "Require falha sem o CA"
else
    fail "Require falha sem o CA: $code"
fi
"$BIN" query run -c 'select 1' --ssl-mode Prefer -o "$OUT/tls-prefer-no-ca" \
    >/tmp/prefer-no-ca.log 2>&1
code=$?
if [ "$code" -ne 0 ]; then
    pass "Prefer falha sem o CA (não cai para texto puro)"
else
    fail "Prefer falha sem o CA (não cai para texto puro): $code"
fi
mv /tmp/mustel-smoke-ca.crt "$CA_FILE"
update-ca-certificates >/tmp/update-ca-restore.log 2>&1

# Serviço sem TLS: Prefer cai com aviso; Require falha.
ad_hoc='Host=postgres-notls;Port=5432;Database=smoke_notls;Username=postgres'
"$BIN" query run --npgsql-connection-string "$ad_hoc" -W smoke \
    -c 'select 1' --ssl-mode Prefer -o "$OUT/tls-prefer-notls" >/tmp/prefer-notls.log 2>&1
code=$?
if [ "$code" -eq 0 ] && grep -q 'without TLS' /tmp/prefer-notls.log; then
    pass "Prefer sem TLS cai com aviso"
else
    fail "Prefer sem TLS cai com aviso: $code"
fi
"$BIN" query run --npgsql-connection-string "$ad_hoc" -W smoke \
    -c 'select 1' --ssl-mode Require -o "$OUT/tls-require-notls" >/tmp/require-notls.log 2>&1
code=$?
err_csv=$(find "$OUT/tls-require-notls" -name '*_erros.csv' | head -1)
if [ "$code" -ne 0 ] && { [ -z "$err_csv" ] || grep -Eqi 'does not support TLS|TLS handshake' "$err_csv"; }; then
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
common=(-H postgres -p 5432 -U postgres)
mkdir -p "$OUT/backup"
one="$OUT/backup/mustel_001.dump"
"$BIN" database backup mustel_001 -o "$one" "${common[@]}" >/tmp/backup.log 2>&1
if [ $? -eq 0 ] && [ -s "$one" ]; then pass "backup de um banco"; else fail "backup de um banco"; fi

"$BIN" database backup -a -o "$OUT/backup/all" "${common[@]}" >/tmp/backup-all.log 2>&1
code=$?
dumps=$(find "$OUT/backup/all" -name '*.dump' 2>/dev/null | wc -l)
if [ "$code" -eq 0 ] && [ "$dumps" -ge 100 ]; then
    pass "backup --all ($dumps dumps)"
else
    fail "backup --all: exit=$code dumps=$dumps"
fi
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
mkdir -p /tmp/xdg
chmod 700 /tmp/xdg
export XDG_RUNTIME_DIR=/tmp/xdg
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
Xvfb :99 -screen 0 1440x900x24 >/tmp/xvfb.log 2>&1 &
XVFB_PID=$!
sleep 2
"$BIN" gui >/tmp/gui.log 2>&1 &
GUI_PID=$!
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
    sleep 1
    if import -display :99 -window root "$OUT/gui/01-query.png" 2>/tmp/import.log &&
        [ -s "$OUT/gui/01-query.png" ]; then
        pass "captura da tela inicial"
    else
        fail "captura da tela inicial"
    fi
    # As abas ficam no topo, na linha y≈17 da janela (1440x900). Depois de cada clique, a captura
    # seguinte precisa diferir da anterior: `compare -metric AE` conta os pixels diferentes.
    xdotool mousemove --sync 200 17 click 1 2>/dev/null
    sleep 1
    import -display :99 -window root "$OUT/gui/02-connections.png" 2>/dev/null
    xdotool mousemove --sync 290 17 click 1 2>/dev/null
    sleep 1
    import -display :99 -window root "$OUT/gui/03-settings.png" 2>/dev/null

    changed() {
        local before="$1" after="$2"
        local pixels
        pixels=$(compare -metric AE "$before" "$after" null: 2>&1 | awk '{print $1}')
        [ "${pixels:-0}" -gt 1000 ] 2>/dev/null
    }
    if [ -s "$OUT/gui/02-connections.png" ] && changed "$OUT/gui/01-query.png" "$OUT/gui/02-connections.png"; then
        pass "clique em Connections mudou a tela"
    else
        fail "clique em Connections mudou a tela"
    fi
    if [ -s "$OUT/gui/03-settings.png" ] && changed "$OUT/gui/02-connections.png" "$OUT/gui/03-settings.png"; then
        pass "clique em Settings mudou a tela"
    else
        fail "clique em Settings mudou a tela"
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

echo ""
echo "PASSED=$PASSED FAILED=$FAILED"
{
    echo "PASSED=$PASSED FAILED=$FAILED"
    echo "binário: $("$BIN" --version 2>/dev/null)"
} >"$OUT/summary.txt"

[ "$FAILED" -eq 0 ]
