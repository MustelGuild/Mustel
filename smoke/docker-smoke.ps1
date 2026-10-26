# Smoke headless: sobe Postgres 18.6 + runner Linux, roda CLI e GUI dentro do Docker (Xvfb) e
# destrói tudo. Nenhuma janela abre no desktop do dono; as capturas ficam em smoke/out/gui.
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File smoke/docker-smoke.ps1
#   powershell -ExecutionPolicy Bypass -File smoke/docker-smoke.ps1 -Keep
param([switch]$Keep)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$OutputEncoding = [Text.Encoding]::UTF8

$compose = Join-Path $PSScriptRoot 'docker-compose.yml'
$out = Join-Path $PSScriptRoot 'out'
New-Item -ItemType Directory -Force -Path $out | Out-Null

docker compose -f $compose down -v --remove-orphans 2>&1 | Out-Null
docker compose -f $compose up --build --abort-on-container-exit --exit-code-from smoke
$code = $LASTEXITCODE

if (-not $Keep) {
    docker compose -f $compose down -v --remove-orphans 2>&1 | Out-Null
}

Write-Output ''
if ($code -eq 0) {
    Write-Output 'SMOKE (container) OK'
} else {
    Write-Output ("SMOKE (container) FALHOU: exit " + $code)
}
Write-Output ("capturas: " + (Join-Path $out 'gui'))
exit $code
