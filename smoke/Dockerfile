# Imagem do smoke: PostgreSQL com TLS e um conjunto fixo de bases.
#
# O certificado é de uma CA local e serve só para exercitar os modos de `--ssl-mode`; o runner
# instala a CA no store e prova `Prefer`/`Require` com validação de cadeia + hostname.
FROM postgres:18.6@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6

RUN apt-get update \
    && apt-get install -y --no-install-recommends openssl \
    && rm -rf /var/lib/apt/lists/*

RUN openssl req -new -x509 -days 3650 -nodes -sha256 \
        -subj "/CN=Mustel Smoke CA" \
        -out /etc/postgresql/ca.crt \
        -keyout /etc/postgresql/ca.key \
    && openssl req -new -nodes -sha256 -subj "/CN=postgres" \
        -out /tmp/server.csr \
        -keyout /etc/postgresql/server.key \
    && printf '%s\n' \
        'subjectAltName=DNS:localhost,DNS:postgres,IP:127.0.0.1' \
        'basicConstraints=critical,CA:FALSE' \
        'keyUsage=critical,digitalSignature,keyEncipherment' \
        'extendedKeyUsage=serverAuth' > /tmp/server.ext \
    && openssl x509 -req -days 3650 -sha256 \
        -in /tmp/server.csr \
        -CA /etc/postgresql/ca.crt \
        -CAkey /etc/postgresql/ca.key \
        -CAcreateserial \
        -extfile /tmp/server.ext \
        -out /tmp/server.crt \
    && cat /tmp/server.crt /etc/postgresql/ca.crt > /etc/postgresql/server.crt \
    && chown postgres:postgres /etc/postgresql/server.crt /etc/postgresql/server.key \
        /etc/postgresql/ca.crt /etc/postgresql/ca.key \
    && chmod 600 /etc/postgresql/server.key /etc/postgresql/ca.key \
    && rm -f /tmp/server.csr /tmp/server.ext /tmp/server.crt

COPY init/ /docker-entrypoint-initdb.d/
