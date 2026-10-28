# Ambiente de smoke

Sobe um PostgreSQL **18.6** descartável com **100+ bases** e roda o CLI inteiro contra ele. O
objetivo é provar os fluxos que os testes headless não alcançam: conexão real, descoberta de bases,
CSV, backup/restore, TLS, nomes hostis e a janela da GUI.

## O que a imagem cria

| Item | Conteúdo |
|---|---|
| `mustel_001` … `mustel_100` | cada uma com `items` (3 linhas) e `formulas` (8 valores perigosos) |
| `my-db`, `Meu Banco` | nomes legítimos que **não** são identificadores simples (hífen/espaço) |
| `..\hostile` | nome hostil de propósito: prova que o `backup --all` pula o banco (IF-063) |
| `postgres` | base administrativa (fica fora do `--all` pelos `excludePatterns` padrão) |
| `smoke_app` | usuário sem superusuário, com `SELECT` nas tabelas |

- Porta no host: **55432** (evita o 5432 do dono).
- Credenciais: `postgres` / `smoke` e `smoke_app` / `smoke_app`.
- TLS: CA local assina um certificado de servidor com SAN para `postgres`/`localhost`. O runner
  extrai a CA do handshake e a instala no store descartável; o host usa `Disable` porque não mexe
  no store do Windows.

## Como rodar

**Tudo dentro do Docker, sem abrir janela no seu desktop:**

```powershell
powershell -ExecutionPolicy Bypass -File smoke/docker-smoke.ps1
powershell -ExecutionPolicy Bypass -File smoke/docker-smoke.ps1 -Keep  # preserva volume e imagens
```

O `docker-smoke.ps1` sobe o Postgres 18.6 e um runner Linux com Rust, Xvfb, xdotool e ImageMagick,
roda o CLI e a GUI **dentro do container** e destrói tudo. A GUI é desenhada num display virtual
(Xvfb); nenhuma janela aparece no Windows. As capturas ficam em `smoke/out/gui`
(`01-query.png`, `02-connections.png`, `03-settings.png`) e o smoke compara os pixels para provar
que os cliques nas abas mudaram a tela.

O fonte é montado em `/src` somente na execução e o `target` do Cargo fica num volume nomeado
(`mustel-smoke_target`), então o `target/` do Windows não é tocado. O primeiro run compila o
workspace no Linux (alguns minutos); os seguintes reutilizam o volume (use `-Keep` para não
apagá-lo).

**Modo rápido no host** (usa o binário do Windows e só o Postgres no Docker):

```powershell
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Keep       # não destrói o container
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Gui        # abre e fecha a janela real
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -SkipBuild  # usa o binário atual
```

O script compila o binário, recria o container do zero (`down -v` + `up --build`), espera o
healthcheck (o marcador só existe depois das 100 bases), roda as verificações e destrói tudo no
`finally`. A configuração do Mustel e a saída ficam em `%TEMP%`; o cadastro, o cofre e o `pgpass` do
dono **não** são tocados. Use `-Keep` para inspecionar depois.

## O que é verificado

- `settings db-servers add` / `ls` / `test` no config isolado, com a senha no **cofre efêmero do container**;
- `query run` em um banco e com `--all` (103 bases), **sem `-W`** (senha vem do cofre do container), conferindo o CSV consolidado;
- `--csv-escape-formulas`: default intacto e prefixo `'` nos valores `=`, `+`, `-`, `@`;
- TLS no container: `Disable`, `Prefer` e `Require` conectam com a CA instalada; sem a CA,
  `Prefer`/`Require` falham sem cair para texto puro; `Prefer` cai **em silêncio** no serviço sem
  TLS e `Require` falha nele;
- `sslMode` inválido no cadastro é erro, não `Prefer` silencioso;
- usuário sem superusuário (`smoke_app`) lendo a tabela;
- `database backup` de um banco, `--all` (com o banco hostil pulado) e `--strict` (exit não-zero);
- `database restore --create-if-not-exists --no-clean` e consulta no banco restaurado;
- no modo container: a GUI abre sob Xvfb já com a descoberta feita pelo cofre (`Smoke has 107
database(s)` nas capturas), os cliques em Connections e Settings são validados por diferença de
pixels, e nada abre no desktop do dono.

## O que não é verificado aqui

- no **modo host** o `settings db-servers test` não roda, porque ele exigiria gravar no cofre do
dono; o modo container cobre esse caminho no cofre efêmero do runner, que é destruído no fim;
- o interior da GUI (autocomplete, seleção, formulários): use a skill `mustel-gui-smoke-test`;
- o sucesso de `Require` no **modo host** (sem instalar a CA no Windows); o container cobre esse
  caminho com uma CA descartável;
- `pgpass add/list/remove`: escreveria no `pgpass.conf` real.

## Reprodução manual

```powershell
docker compose -f smoke/docker-compose.yml up -d --build
# aguarde o healthcheck (docker inspect --format '{{.State.Health.Status}}' mustel-smoke-postgres)
.\target\debug\mustel.exe query run -H 127.0.0.1 -p 55432 -U postgres -W smoke --ssl-mode Disable -a `
    -c "select count(*) as n from items" -o "$env:TEMP\smoke-out"
docker compose -f smoke/docker-compose.yml down -v
```

A imagem base está fixada pelo digest em `Dockerfile`; o certificado é gerado no build. O diretório
`smoke/` é de engenharia, não documentação de produto. `smoke/out/` é ignorado pelo Git.
