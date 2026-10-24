<#
.SYNOPSIS
    Cria (ou recria) no Postgres o ambiente de teste da seleção de bases por query.

.DESCRIPTION
    A aba **Query** pode marcar bases em massa a partir de uma query salva. Essa query roda num
    banco do **cadastro** — o "configurador" — e devolve quais bases de quais servidores marcar.
    Este script monta esse ambiente: a base `configurador`, a tabela `tenants` e vistas prontas para
    cada cenário que a tela sabe tratar.

    É **idempotente**: apaga e recria a base a cada execução.

    Os servidores referenciados nas linhas são lidos do **seu** `mustel.jsonc`, então o ambiente
    continua correto se você adicionar ou renomear servidores. Se dois servidores compartilham o
    mesmo host e porta (o caso de `Local` e `Local 2`), as vistas usam a coluna `server` para
    desempatar; há uma vista sem ela, de propósito, para você ver o erro de ambiguidade.

    Os valores de senha nas connection strings são **fictícios**: a execução usa o cofre do sistema
    (`keyring`), nunca a senha da linha.

.PARAMETER Database
    Nome da base a criar. Padrão: `configurador`.

.PARAMETER ConnectionString
    Conexão do `psql` (ex.: `postgres://postgres:dev@127.0.0.1:5432/postgres`). Sem ela, o script usa
    o `psql` do PATH em `127.0.0.1:5432` com o usuário `postgres`, ou o Docker.

.PARAMETER Container
    Roda o `psql` dentro deste container Docker (ex.: `postgres-ptbr`) em vez de usar o do PATH.
    Sem `-Container`, o script procura um container com a porta 5432 publicada.

.PARAMETER Password
    Senha do `psql` do PATH. Sem ela, o `psql` roda com `-w` e falha em vez de pedir senha (o
    prompt travaria um script).

.PARAMETER Config
    Caminho do `mustel.jsonc`. Padrão: o do Mustel no SO atual (`%APPDATA%\Mustel` no Windows).

.EXAMPLE
    ./scripts/selection-fixture.ps1

.EXAMPLE
    ./scripts/selection-fixture.ps1 -Container postgres-ptbr

.EXAMPLE
    ./scripts/selection-fixture.ps1 -Database configurador -ConnectionString "postgres://postgres:dev@127.0.0.1:5432/postgres"
#>
[CmdletBinding()]
param(
    [ValidatePattern('^[a-z_][a-z0-9_]*$')]
    [string]$Database = 'configurador',

    [string]$ConnectionString = '',

    [string]$Container = '',

    [string]$Password = '',

    [string]$Config = ''
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) { Write-Host "==> $Message" -ForegroundColor Cyan }

# --- cadastro -----------------------------------------------------------------------------------

if (-not $Config) {
    $base = if ($env:MUSTEL_CONFIG_DIR) { $env:MUSTEL_CONFIG_DIR } else { Join-Path $env:APPDATA 'Mustel' }
    $Config = Join-Path $base 'mustel.jsonc'
}
if (-not (Test-Path $Config)) {
    throw "Cadastro não encontrado em '$Config'. Cadastre um servidor com 'mustel settings db-servers add' ou passe -Config."
}

$json = Get-Content -Raw -Path $Config
try {
    $settings = $json | ConvertFrom-Json
} catch {
    throw "Não consegui ler '$Config' como JSON: $($_.Exception.Message)"
}

$servers = @($settings.servers)
if ($servers.Count -eq 0) {
    throw "O cadastro em '$Config' não tem servidor nenhum. Cadastre um antes de montar o ambiente."
}

$first = $servers[0]
$second = if ($servers.Count -gt 1) { $servers[1] } else { $servers[0] }

Write-Step "cadastro: $Config"
Write-Step "servidores: $(($servers | ForEach-Object { "$($_.name) ($($_.host):$($_.port))" }) -join ', ')"

# --- SQL ----------------------------------------------------------------------------------------

function SqlLit([string]$Value) {
    if ($null -eq $Value) { return 'NULL' }
    "'" + $Value.Replace("'", "''") + "'"
}

function SqlNullable([string]$Value) {
    if ([string]::IsNullOrEmpty($Value)) { return 'NULL' }
    SqlLit $Value
}

# Uma connection string no estilo Npgsql, montada a partir das mesmas partes que vão nas colunas
# separadas. `Password` só entra quando o usuário é o do cadastro: a linha do `ana` fica **sem**
# senha de propósito, para exercitar o aviso de credencial (R4b).
function NpgsqlString($HostName, $PortNumber, $DbName, $User, $Password) {
    $parts = @("Host=$HostName", "Port=$PortNumber", "Database=$DbName")
    if ($User) {
        $parts += "Username=$User"
        if ($Password) { $parts += "Password=$Password" }
    } else {
        $parts += "Username=$($first.username)"
        $parts += "Password=$FixturePassword"
    }
    $parts -join ';'
}

$FixturePassword = 'senha-ficticia'

# Uma linha por cenário, na mesma tabela `tenants`: **as duas formas preenchidas** — a connection
# string e as colunas separadas. `ambiente = 'invalido'` fica fora das vistas principais: é o
# material do erro agregado de host desconhecido.
$rows = @(
    @{ id = 'dev-colunas'; amb = 'prod'; srv = $first.name; host = $first.host; port = $first.port
       db = 'dev'; user = ''; pass = '' },
    @{ id = 'test2-colunas'; amb = 'homolog'; srv = $first.name; host = $first.host; port = $first.port
       db = 'test2'; user = ''; pass = '' },
    @{ id = 'base-configurador'; amb = 'prod'; srv = $first.name; host = $first.host; port = $first.port
       db = $Database; user = ''; pass = '' },
    @{ id = 'dev-outro-servidor'; amb = 'prod'; srv = $second.name; host = $second.host
       port = $second.port; db = 'dev'; user = ''; pass = '' },
    @{ id = 'test2-outro-servidor'; amb = 'homolog'; srv = $second.name; host = $second.host
       port = $second.port; db = 'test2'; user = ''; pass = '' },
    @{ id = 'base-excluida-postgres'; amb = 'prod'; srv = $first.name; host = $first.host
       port = $first.port; db = 'postgres'; user = ''; pass = '' },
    @{ id = 'base-excluida-template'; amb = 'prod'; srv = $first.name; host = $first.host
       port = $first.port; db = 'template0'; user = ''; pass = '' },
    @{ id = 'usuario-diferente-sem-senha'; amb = 'prod'; srv = $second.name; host = $second.host
       port = $second.port; db = $Database; user = 'ana'; pass = '' },
    @{ id = 'linha-sem-base'; amb = 'prod'; srv = $first.name; host = $first.host
       port = $first.port; db = ''; user = ''; pass = '' },
    @{ id = 'host-desconhecido'; amb = 'invalido'; srv = ''; host = '10.9.9.9'
       port = 5432; db = 'dev'; user = ''; pass = '' }
)

$inserts = foreach ($row in $rows) {
    $port = if ($row.port) { $row.port } else { 'NULL' }
    # A string existe sempre que houver host e base; a linha incompleta fica sem ela, como no dado
    # real (nem todo registro tem tudo).
    $cs = if ($row.host -and $row.db) {
        NpgsqlString $row.host $row.port $row.db $row.user $row.pass
    } else { '' }
    "    ({0}, {1}, {2}, {3}, {4}, {5}, {6}, {7}, {8})" -f (SqlLit $row.id), (SqlLit $row.amb),
        (SqlNullable $cs), (SqlNullable $row.srv), (SqlNullable $row.host), $port,
        (SqlNullable $row.db), (SqlNullable $row.user), (SqlNullable $row.pass)
}

$sql = @"
\set ON_ERROR_STOP on

DROP DATABASE IF EXISTS "$Database" WITH (FORCE);
CREATE DATABASE "$Database";
\connect "$Database"

CREATE TABLE tenants (
    nome              text PRIMARY KEY,
    ambiente          text NOT NULL,
    connection_string text,
    server            text,
    host              text,
    port              int,
    database          text,
    username          text,
    password          text
);

INSERT INTO tenants
    (nome, ambiente, connection_string, server, host, port, database, username, password)
VALUES
$($inserts -join ",`n");

COMMENT ON TABLE tenants IS
    'Cadastro de teste: cada linha diz onde um tenant vive. Lido pela query de seleção da aba Query.';

-- **Colunas separadas** (o caminho explícito): server desempata dois servidores no mesmo
-- host/porta, username/password sobrescrevem os do cadastro quando existem.
CREATE VIEW v_selecao AS
    SELECT nome, server, host, port, database, username, password
    FROM tenants
    WHERE ambiente <> 'invalido';

-- **Connection string Npgsql**: a linha inteira vem numa coluna, com server ao lado porque a
-- string — por definição — não sabe o nome do servidor do cadastro.
CREATE VIEW v_selecao_string AS
    SELECT nome, connection_string, server
    FROM tenants
    WHERE ambiente <> 'invalido' AND connection_string IS NOT NULL;

-- Sem server: com dois servidores no mesmo 127.0.0.1:5432 isto aborta listando os candidatos.
CREATE VIEW v_selecao_ambigua AS
    SELECT nome, host, port, database, username, password
    FROM tenants
    WHERE ambiente <> 'invalido';

-- Inclui o host que não está no cadastro: aborta a carga inteira com o erro agregado.
CREATE VIEW v_selecao_host_desconhecido AS
    SELECT nome, server, host, port, database
    FROM tenants;

-- A string sozinha não desempata dois servidores no mesmo host/porta.
CREATE VIEW v_selecao_string_sem_server AS
    SELECT nome, connection_string
    FROM tenants
    WHERE ambiente <> 'invalido' AND connection_string IS NOT NULL;

-- A porta como texto: o leitor aceita os dois tipos.
CREATE VIEW v_selecao_porta_texto AS
    SELECT nome, server, host, port::text AS port, database
    FROM tenants
    WHERE ambiente <> 'invalido';

SELECT nome, ambiente, COALESCE(server, '-') AS server, COALESCE(database, '-') AS database,
       CASE WHEN connection_string IS NULL THEN '-' ELSE 'string' END AS forma
FROM tenants ORDER BY nome;
"@

# --- execução -----------------------------------------------------------------------------------

function Invoke-Psql([string]$Text) {
    $Text | & psql @PsqlArgs
    if ($LASTEXITCODE -ne 0) { throw "psql terminou com código $LASTEXITCODE" }
}

$PsqlArgs = @()
$mode = ''

# Ordem: conexão explícita → container explícito → container com a 5432 publicada → psql do PATH.
# O Docker vem antes do `psql` porque o Postgres de desenvolvimento deste projeto mora num
# container, e o `psql` do host pediria senha interativamente.
if ($ConnectionString) {
    $mode = "psql em $ConnectionString"
    # As opções vêm **antes** do dbname: o `psql` trata o primeiro argumento solto como a conexão e
    # ignora flags que venham depois dele.
    $PsqlArgs = @('-w', '-v', 'ON_ERROR_STOP=1', '-q', $ConnectionString)
} else {
    if (-not $Container) {
        $published = docker ps --format '{{.Names}} {{.Ports}}' 2>$null |
            Where-Object { $_ -match ':5432->' } |
            ForEach-Object { ($_ -split ' ')[0] } |
            Select-Object -First 1
        if ($published) { $Container = $published }
    }

    if ($Container) {
        $mode = "docker exec $Container psql"
    } elseif (Get-Command psql -ErrorAction SilentlyContinue) {
        $mode = 'psql do PATH (127.0.0.1:5432)'
        if ($Password) { $env:PGPASSWORD = $Password }
        $PsqlArgs = @('-h', '127.0.0.1', '-p', '5432', '-U', 'postgres', '-d', 'postgres', '-w',
                      '-v', 'ON_ERROR_STOP=1', '-q')
    } else {
        throw 'Não achei Postgres. Passe -Container <nome> ou -ConnectionString <url>.'
    }
}

Write-Step "alvo: $mode"
Write-Step "base: $Database"

if ($Container -and -not $ConnectionString) {
    # O `psql` dentro do container não precisa de senha (autenticação local por socket).
    $Text = $sql
    $Text | & docker exec -i $Container psql -U postgres -d postgres -v ON_ERROR_STOP=1 -q
    if ($LASTEXITCODE -ne 0) {
        throw "docker exec psql terminou com código $LASTEXITCODE. O container '$Container' tem Postgres?"
    }
} else {
    Invoke-Psql $sql
}

Write-Host ''
Write-Host 'Ambiente pronto.' -ForegroundColor Green
Write-Host ''
Write-Host 'Na aba Settings, cadastre a query:' -ForegroundColor Yellow
Write-Host "  Name      : Producao"
Write-Host "  Server    : $($first.name)"
Write-Host "  Database  : $Database"
Write-Host '  SQL       : select * from v_selecao'
Write-Host ''
Write-Host 'Outras vistas para ver os outros caminhos:' -ForegroundColor Yellow
Write-Host '  v_selecao_string               mesma tabela, lida pela connection string Npgsql'
Write-Host '  v_selecao_ambigua               dois servidores no mesmo host/porta -> erro com candidatos'
Write-Host '  v_selecao_host_desconhecido     host fora do cadastro -> erro agregado (10.9.9.9)'
Write-Host '  v_selecao_string_sem_server     string sozinha -> ambiguidade'
Write-Host '  v_selecao_porta_texto           porta como texto em vez de inteiro'
