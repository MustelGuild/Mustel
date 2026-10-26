# Ambiente de smoke

Sobe um PostgreSQL 17 descartável com **100+ bases** e roda o CLI inteiro contra ele. O objetivo é
provar os fluxos que os testes headless não alcançam: conexão real, descoberta de bases, CSV,
backup/restore, TLS, nomes hostis e a janela da GUI.

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
- TLS: certificado autoassinado em `/etc/postgresql/server.crt` (o servidor aceita TLS; o cliente
  ainda não implementa `Require` — o smoke confere que ele falha explicitamente).

## Como rodar

```powershell
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Keep       # não destrói o container
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Gui        # abre e fecha a janela
powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -SkipBuild  # usa o binário atual
```

O script compila o binário, recria o container do zero (`down -v` + `up --build`), espera o
healthcheck (o marcador só existe depois das 100 bases), roda as verificações e destrói tudo no
`finally`. A configuração do Mustel e a saída ficam em `%TEMP%`; o cadastro, o cofre e o `pgpass` do
dono **não** são tocados. Use `-Keep` para inspecionar depois.

## O que é verificado

- `settings db-servers add` / `ls` no config isolado;
- `query run` em um banco e com `--all` (103 bases), conferindo o CSV consolidado;
- `--csv-escape-formulas`: default intacto e prefixo `'` nos valores `=`, `+`, `-`, `@`;
- TLS: `Disable` e `Prefer` conectam; `Require` falha com o `sslMode` explicado no `_erros.csv`;
- `sslMode` inválido no cadastro é erro, não `Prefer` silencioso;
- usuário sem superusuário (`smoke_app`) lendo a tabela;
- `database backup` de um banco, `--all` (com o banco hostil pulado) e `--strict` (exit não-zero);
- `database restore --create-if-not-exists --no-clean` e consulta no banco restaurado;
- `-Gui`: a janela abre e fecha.

## O que não é verificado aqui

- `settings db-servers test`: ele resolve a senha pelo cofre/prompt, e o script **não** grava no
  cofre do dono. A conectividade é provada pelo `query run` com `-W`;
- o interior da GUI (autocomplete, seleção, formulários): use a skill `mustel-gui-smoke-test`;
- o sucesso de `--ssl-mode Require`: depende de D1 (TLS do Postgres), ainda pendente;
- `pgpass add/list/remove`: escreveria no `pgpass.conf` real.

## Reprodução manual

```powershell
docker compose -f smoke/docker-compose.yml up -d --build
# aguarde o healthcheck (docker inspect --format '{{.State.Health.Status}}' mustel-smoke-postgres)
.\target\debug\mustel.exe query run -H 127.0.0.1 -p 55432 -U postgres -W smoke -a `
    -c "select count(*) as n from items" -o "$env:TEMP\smoke-out"
docker compose -f smoke/docker-compose.yml down -v
```

A imagem base está fixada pelo digest em `Dockerfile`; o certificado é gerado no build. O diretório
`smoke/` é de engenharia, não documentação de produto.
