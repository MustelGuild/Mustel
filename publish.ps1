<#
.SYNOPSIS
    Gera o pacote de release do Mustel para a plataforma atual (D26).

.DESCRIPTION
    Builda o binário release (com --locked), monta a pasta do pacote com binário,
    configuração de exemplo e LICENSE, gera o .zip e o SHA256 em packaging/.
    A **publicação é manual**: o script só produz o artefato. O equivalente no Unix é
    o publish.sh, com o mesmo layout de saída.

.PARAMETER NoBuild
    Reempacota o binário já construído, sem rodar o cargo build.

.EXAMPLE
    ./publish.ps1

.EXAMPLE
    ./publish.ps1 -NoBuild
#>
[CmdletBinding()]
param(
    [switch] $NoBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $root
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo não encontrado no PATH'
    }

    if (-not $NoBuild) {
        Write-Host '==> cargo build --release --locked -p mustel'
        cargo build --release --locked -p mustel
        if ($LASTEXITCODE -ne 0) { throw "cargo build falhou (exit $LASTEXITCODE)" }
    }

    $version = (cargo pkgid -p mustel | Select-Object -Last 1) -replace '.*#', ''
    $triple = (rustc -vV | Select-String -Pattern '^host: (.*)$').Matches[0].Groups[1].Value
    if (-not $version -or -not $triple) { throw 'não consegui obter versão/triple' }

    $exe = if ($IsWindows -or $env:OS -eq 'Windows_NT') { 'mustel.exe' } else { 'mustel' }
    $binary = Join-Path $root "target\release\$exe"
    if (-not (Test-Path $binary)) { throw "binário não encontrado: $binary (rode sem -NoBuild)" }

    $stage = Join-Path $root "packaging\mustel-$version-$triple"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null

    Copy-Item $binary $stage
    Copy-Item (Join-Path $root 'mustel.example.jsonc') $stage
    Copy-Item (Join-Path $root 'LICENSE') $stage

    $readme = @"
Mustel $version ($triple)

1. Copie o binário '$exe' para um diretório no PATH.
2. Copie 'mustel.example.jsonc' para o diretório de configuração e ajuste:
   Windows:     %APPDATA%\Mustel\mustel.jsonc
   Linux/macOS: \$XDG_CONFIG_HOME/Mustel/mustel.jsonc (ou ~/.config/Mustel/mustel.jsonc)
3. 'mustel --help' lista os comandos.

Licença: GPL-3.0-only (ver LICENSE).
"@
    Set-Content -Path (Join-Path $stage 'README.txt') -Value $readme -Encoding utf8

    $archive = "$stage.zip"
    if (Test-Path $archive) { Remove-Item -Force $archive }
    Compress-Archive -Path $stage -DestinationPath $archive

    $hash = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLowerInvariant()
    "$hash  $(Split-Path -Leaf $archive)" | Set-Content -Path "$archive.sha256" -Encoding ascii

    Write-Host ''
    Write-Host "pacote:  $archive"
    Write-Host "sha256:  $hash"
    Write-Host 'publicação: manual (D26) — envie o arquivo acima onde precisar'
}
finally {
    Pop-Location
}
