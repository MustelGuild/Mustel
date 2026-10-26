# Smoke de ponta a ponta do Mustel contra o ambiente Docker.
#
# Sobe a imagem de `smoke/Dockerfile` (Postgres 17 com TLS e 100+ bases), roda os fluxos do CLI e
# destrói o ambiente no fim. A configuração do Mustel e a saída ficam em %TEMP%, então o cadastro,
# o cofre e o pgpass do dono não são tocados.
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1
#   powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Keep      # não destrói o container
#   powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -Gui       # inclui abrir a janela
#   powershell -ExecutionPolicy Bypass -File smoke/smoke.ps1 -SkipBuild # usa o binário atual
param(
    [switch]$Keep,
    [switch]$Gui,
    [switch]$SkipBuild,
    [int]$Port = 55432
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$OutputEncoding = [Text.Encoding]::UTF8
$root = Split-Path -Parent $PSScriptRoot
$compose = Join-Path $PSScriptRoot 'docker-compose.yml'
$bin = Join-Path $root 'target\debug\mustel.exe'
$config = Join-Path $env:TEMP ('mustel-smoke-' + [guid]::NewGuid().ToString('N'))
$out = Join-Path $env:TEMP ('mustel-smoke-out-' + [guid]::NewGuid().ToString('N'))
$script:failed = 0
$script:previousConfig = $env:MUSTEL_CONFIG_DIR
$script:previousPassword = $env:MUSTEL_PASSWORD

function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if ($ok) {
        Write-Output ("PASS  " + $name)
    } else {
        Write-Output ("FAIL  " + $name + "  " + $detail)
        $script:failed++
    }
}

function Invoke-Mustel([string[]]$arguments) {
    $text = & $bin @arguments 2>&1 | Out-String
    [pscustomobject]@{ Code = $LASTEXITCODE; Text = $text }
}

function Find-Csv([string]$directory, [string]$filter) {
    Get-ChildItem -Recurse $directory -Filter $filter -ErrorAction SilentlyContinue |
        Select-Object -First 1
}

function Wait-Healthy([int]$seconds = 420) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        $status = (docker inspect --format '{{.State.Health.Status}}' mustel-smoke-postgres 2>$null)
        if ($LASTEXITCODE -ne 0) { return $false }
        if ($status -eq 'healthy') { return $true }
        if ($status -eq 'unhealthy') { return $false }
        Start-Sleep -Seconds 3
    }
    return $false
}

try {
    New-Item -ItemType Directory -Force -Path $config, $out | Out-Null
    $env:MUSTEL_CONFIG_DIR = $config
    Remove-Item Env:MUSTEL_PASSWORD -ErrorAction SilentlyContinue

    if (-not $SkipBuild) {
        Write-Output '== compilando o binário'
        cargo build -p mustel --features gui --manifest-path (Join-Path $root 'Cargo.toml')
        Check 'build do binário' ($LASTEXITCODE -eq 0)
    }
    Check 'binário existe' (Test-Path $bin) $bin

    Write-Output '== recriando o ambiente Docker'
    docker compose -f $compose down -v --remove-orphans 2>&1 | Out-Null
    docker compose -f $compose up -d --build postgres
    Check 'docker compose up' ($LASTEXITCODE -eq 0)
    Check 'container saudável (100+ bases)' (Wait-Healthy) 'veja docker logs mustel-smoke-postgres'

    Write-Output '== cadastro e conexão'
    $add = Invoke-Mustel @(
        'settings', 'db-servers', 'add', '-n', 'Smoke', '-H', '127.0.0.1', '-p', "$Port",
        '-U', 'postgres', '-d', 'mustel_001', '--ssl-mode', 'Prefer'
    )
    Check 'settings db-servers add' ($add.Code -eq 0) $add.Text

    $list = Invoke-Mustel @('settings', 'db-servers', 'ls')
    Check 'settings db-servers ls mostra Smoke' ($list.Code -eq 0 -and $list.Text -match 'Smoke') $list.Text

    Write-Output '== query run: um banco'
    $single = Join-Path $out 'single'
    $run = Invoke-Mustel @(
        'query', 'run', '-W', 'smoke', '-c', 'select count(*) as n from items', '-o', $single
    )
    Check 'query run (um banco) exit 0' ($run.Code -eq 0) $run.Text
    $csv = Find-Csv $single 'consolidated_*.csv'
    $lines = if ($csv) { Get-Content $csv.FullName } else { @() }
    Check 'CSV consolidado existe' ($null -ne $csv)
    Check 'CSV tem mustel_001 com 3' (($lines -join "`n") -match 'Smoke,mustel_001,3') ($lines -join "`n")

    Write-Output '== query run: todas as bases (--all)'
    $all = Join-Path $out 'all'
    $every = Invoke-Mustel @(
        'query', 'run', '-W', 'smoke', '-a', '-c', 'select count(*) as n from items', '-o', $all
    )
    Check 'query run --all exit 0' ($every.Code -eq 0) $every.Text
    Check 'resumo com 103 sucessos' ($every.Text -match 'Success:\s*(\d+)' -and [int]$Matches[1] -ge 103) $every.Text
    $allCsv = Find-Csv $all 'consolidated_*.csv'
    $allText = if ($allCsv) { Get-Content $allCsv.FullName -Raw } else { '' }
    foreach ($database in @('mustel_100', 'my-db', 'Meu Banco', '..\hostile')) {
        Check ("CSV inclui " + $database) ($allText.Contains(",${database},3")) $database
    }

    Write-Output '== CSV: fórmulas'
    $plain = Join-Path $out 'plain'
    $p = Invoke-Mustel @(
        'query', 'run', '-W', 'smoke', '-d', 'mustel_001', '-c',
        'select value from formulas order by id', '-o', $plain
    )
    $plainCsv = Find-Csv $plain 'consolidated_*.csv'
    $plainLines = if ($plainCsv) { Get-Content $plainCsv.FullName } else { @() }
    Check 'CSV default mantém =1+1 (sem prefixo)' ($p.Code -eq 0 -and $plainLines -contains 'Smoke,mustel_001,=1+1') ($plainLines -join "`n")

    $escaped = Join-Path $out 'escaped'
    $e = Invoke-Mustel @(
        'query', 'run', '-W', 'smoke', '-d', 'mustel_001', '--csv-escape-formulas', '-c',
        'select value from formulas order by id', '-o', $escaped
    )
    $escapeCsv = Find-Csv $escaped 'consolidated_*.csv'
    $escapeLines = if ($escapeCsv) { Get-Content $escapeCsv.FullName } else { @() }
    $expected = @(
        "Smoke,mustel_001,'=1+1",
        "Smoke,mustel_001,'@SUM(A1:A2)",
        "Smoke,mustel_001,'-2+3",
        "Smoke,mustel_001,'+cmd|'/c calc'!A0"
    )
    foreach ($line in $expected) {
        Check ("escape presente: " + $line) ($e.Code -eq 0 -and ($escapeLines -contains $line)) ($escapeLines -join "`n")
    }

    Write-Output '== TLS'
    $require = Join-Path $out 'tls-require'
    $r = Invoke-Mustel @('query', 'run', '-W', 'smoke', '-c', 'select 1', '--ssl-mode', 'Require', '-o', $require)
    Check 'Require falha explicitamente (D1 pendente)' ($r.Code -ne 0) $r.Text
    $errors = Find-Csv $require '*_erros.csv'
    $errorText = if ($errors) { Get-Content $errors.FullName -Raw } else { '' }
    Check 'erro do Require explica o sslMode' ($errorText -match 'sslMode') $errorText

    $disable = Invoke-Mustel @('query', 'run', '-W', 'smoke', '-c', 'select 1', '--ssl-mode', 'Disable', '-o', (Join-Path $out 'tls-disable'))
    Check 'ssl-mode Disable conecta' ($disable.Code -eq 0) $disable.Text
    $prefer = Invoke-Mustel @('query', 'run', '-W', 'smoke', '-c', 'select 1', '--ssl-mode', 'Prefer', '-o', (Join-Path $out 'tls-prefer'))
    Check 'ssl-mode Prefer conecta' ($prefer.Code -eq 0) $prefer.Text

    Write-Output '== sslMode inválido no cadastro'
    $configFile = Join-Path $config 'mustel.jsonc'
    $original = [IO.File]::ReadAllText($configFile)
    $broken = [regex]::Replace($original, '("sslMode"\s*:\s*")[^"]*(")', '${1}Requiree${2}')
    [IO.File]::WriteAllText($configFile, $broken)
    $invalid = Invoke-Mustel @('query', 'run', '-W', 'smoke', '-c', 'select 1', '-o', (Join-Path $out 'invalid-ssl'))
    Check 'sslMode inválido é erro, não Prefer' (($broken -ne $original) -and $invalid.Code -ne 0 -and $invalid.Text -match 'invalid ssl mode') ($invalid.Text)
    [IO.File]::WriteAllText($configFile, $original)

    Write-Output '== usuário sem superusuário'
    $app = Invoke-Mustel @(
        'query', 'run', '-U', 'smoke_app', '-W', 'smoke_app', '-d', 'mustel_001', '-c',
        'select count(*) as n from items', '-o', (Join-Path $out 'app')
    )
    Check 'smoke_app lê items' ($app.Code -eq 0) $app.Text

    Write-Output '== backup e restore'
    $backupRoot = Join-Path $out 'backup'
    New-Item -ItemType Directory -Force -Path $backupRoot | Out-Null
    $common = @('-H', '127.0.0.1', '-p', "$Port", '-U', 'postgres', '-W', 'smoke')
    $singleDump = Join-Path $backupRoot 'mustel_001.dump'
    $backup = Invoke-Mustel (@('database', 'backup', 'mustel_001', '-o', $singleDump) + $common)
    Check 'backup de um banco' ($backup.Code -eq 0 -and (Test-Path $singleDump)) $backup.Text

    $allDumps = Join-Path $backupRoot 'all'
    $backupAll = Invoke-Mustel (@('database', 'backup', '-a', '-o', $allDumps) + $common)
    $dumpCount = (Get-ChildItem $allDumps -Filter *.dump -ErrorAction SilentlyContinue).Count
    Check 'backup --all' ($backupAll.Code -eq 0 -and $dumpCount -ge 100) "$($backupAll.Text) dumps=$dumpCount"
    $hostile = Get-ChildItem -Recurse $out -Filter 'hostile.dump' -ErrorAction SilentlyContinue
    Check 'nome hostil não escapou do diretório' (-not $hostile) ($hostile | Out-String)

    $strict = Invoke-Mustel (@('database', 'backup', '-a', '--strict', '-o', (Join-Path $backupRoot 'strict')) + $common)
    Check '--strict falha com banco hostil' ($strict.Code -ne 0 -and $strict.Text -match 'database\(s\) failed') $strict.Text

    $restoredDb = 'smoke_restored_' + [guid]::NewGuid().ToString('N').Substring(0, 8)
    $restore = Invoke-Mustel (@('database', 'restore', $restoredDb, $singleDump, '--create-if-not-exists', '--no-clean') + $common)
    Check 'restore cria e restaura' ($restore.Code -eq 0) $restore.Text

    # `-d` troca o banco da conexão, mas os alvos do `query run` vêm da lista do cadastro: aponta o
    # cadastro para o banco restaurado só nesta checagem e devolve o arquivo depois.
    $beforeRestore = [IO.File]::ReadAllText($configFile)
    $pointing = $beforeRestore.Replace('"mustel_001"', '"' + $restoredDb + '"')
    [IO.File]::WriteAllText($configFile, $pointing)
    $restored = Join-Path $out 'restored'
    $restoredQuery = Invoke-Mustel @('query', 'run', '-W', 'smoke', '-c', 'select count(*) as n from items', '-o', $restored)
    $restoredCsv = Find-Csv $restored 'consolidated_*.csv'
    $restoredText = if ($restoredCsv) { Get-Content $restoredCsv.FullName -Raw } else { '' }
    Check 'consulta no banco restaurado' ($restoredQuery.Code -eq 0 -and $restoredText -match ",$restoredDb,3") $restoredText
    [IO.File]::WriteAllText($configFile, $beforeRestore)

    if ($Gui) {
        Write-Output '== GUI (abre a janela e fecha)'
        Add-Type @"
using System; using System.Runtime.InteropServices;
public class SmokeWindow {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
}
"@
        $guiProcess = Start-Process -FilePath $bin -ArgumentList 'gui' -PassThru
        $handle = [IntPtr]::Zero
        $deadline = (Get-Date).AddSeconds(20)
        while ((Get-Date) -lt $deadline -and $handle -eq [IntPtr]::Zero) {
            Start-Sleep -Seconds 1
            $handle = [SmokeWindow]::FindWindow([NullString]::Value, 'Mustel')
        }
        Check 'janela da GUI abriu' ($handle -ne [IntPtr]::Zero)
        [void]$guiProcess.CloseMainWindow()
        Start-Sleep -Seconds 2
        if (-not $guiProcess.HasExited) { Stop-Process -Id $guiProcess.Id -Force }
    }
} catch {
    Write-Output ("ERRO: " + $_.Exception.Message)
    $script:failed++
} finally {
    if (-not $Keep) {
        docker compose -f $compose down -v --remove-orphans 2>&1 | Out-Null
    }
    if ($null -eq $script:previousConfig) { Remove-Item Env:MUSTEL_CONFIG_DIR -ErrorAction SilentlyContinue } else { $env:MUSTEL_CONFIG_DIR = $script:previousConfig }
    if ($null -eq $script:previousPassword) { Remove-Item Env:MUSTEL_PASSWORD -ErrorAction SilentlyContinue } else { $env:MUSTEL_PASSWORD = $script:previousPassword }
}

Write-Output ''
if ($script:failed -eq 0) {
    Write-Output 'SMOKE OK'
    if ($Keep) { Write-Output "ambiente mantido no container mustel-smoke-postgres (porta $Port)" }
} else {
    Write-Output ("SMOKE FALHOU: " + $script:failed + " verificação(ões)")
}
Write-Output ("config: " + $config)
Write-Output ("saída:  " + $out)
exit $(if ($script:failed -eq 0) { 0 } else { 1 })
