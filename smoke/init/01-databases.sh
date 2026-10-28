#!/usr/bin/env bash
# Cria as bases do smoke: 100 bases `mustel_001..100`, duas com nome legítimo "difícil" (hífen e
# espaço) e uma hostil, usada para provar que o `backup --all` não escreve fora do diretório.
set -euo pipefail

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname postgres <<'SQL'
CREATE ROLE smoke_app LOGIN PASSWORD 'smoke_app';
SQL

make_database() {
    local name="$1"

    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname postgres \
        -c "CREATE DATABASE \"$name\"" >/dev/null

    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$name" <<'SQL' >/dev/null
CREATE TABLE items (
    id integer PRIMARY KEY,
    name text NOT NULL,
    qty integer NOT NULL,
    price numeric(10,2) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT '2026-01-01 12:00:00+00'
);

INSERT INTO items (id, name, qty, price) VALUES
    (1, 'alpha', 1, 1.10),
    (2, 'beta', 2, 2.20),
    (3, 'gamma', 3, 3.30);

CREATE TABLE formulas (
    id integer PRIMARY KEY,
    value text NOT NULL
);

INSERT INTO formulas (id, value) VALUES
    (1, '=1+1'),
    (2, '+cmd|''/c calc''!A0'),
    (3, '-2+3'),
    (4, '@SUM(A1:A2)'),
    (5, 'normal'),
    (6, 'com, vírgula'),
    (7, 'com "aspas"'),
    (8, E'linha\nquebrada');

GRANT SELECT ON ALL TABLES IN SCHEMA public TO smoke_app;
SQL
}

for index in $(seq -w 1 100); do
    make_database "mustel_$index"
done

# Nomes que o PostgreSQL aceita e que **não** são identificadores simples: o backup precisa aceitar.
make_database "my-db"
make_database "Meu Banco"

# Nome hostil de propósito: o `backup --all` pula este banco (IF-063); o `query run` ainda conecta.
make_database '..\hostile'

date -u +"smoke" > /tmp/smoke-init.done
