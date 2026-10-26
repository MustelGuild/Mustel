# Imagem do smoke: PostgreSQL com TLS e um conjunto fixo de bases.
#
# O certificado é autoassinado e serve só para exercitar os modos de `--ssl-mode`; o `Require`
# ainda falha no cliente (D1 pendente) e o smoke confere que ele falha **explicitamente**.
FROM postgres:17@sha256:b8faeff7712663c2773a190181e1246e37e364b887611c04658c28b44e2408fe

RUN apt-get update \
    && apt-get install -y --no-install-recommends openssl \
    && rm -rf /var/lib/apt/lists/*

RUN openssl req -new -x509 -days 3650 -nodes -subj "/CN=localhost" \
        -out /etc/postgresql/server.crt \
        -keyout /etc/postgresql/server.key \
    && chown postgres:postgres /etc/postgresql/server.crt /etc/postgresql/server.key \
    && chmod 600 /etc/postgresql/server.key

COPY init/ /docker-entrypoint-initdb.d/
