# Gera o instalador Windows do Mustel com o Inno Setup.
#
# Uso (no Windows, depois do publish.ps1 ou deixando este script chamá-lo):
#   ./installer/publish-installer.ps1
#
# Saída: packaging\mustel-<versão>-<triple>-setup.exe e o .sha256 ao lado.
# O ISCC.exe (Inno Setup 6 ou 7) é Windows-only; o teste local no Linux é installer/wine-test.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$iss = Join-Path $root 'installer\mustel.iss'
$out = Join-Path $root 'packaging'

Push-Location $root
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'cargo não encontrado no PATH'
    }

    $version = (cargo pkgid -p mustel | Select-Object -Last 1) -replace '.*#', ''
    if ($LASTEXITCODE -ne 0 -or -not $version) {
        throw 'não consegui obter a versão de cargo pkgid -p mustel'
    }
    $version = $version.Trim()

    $triple = (rustc -vV | Select-String -Pattern '^host: (.*)$').Matches[0].Groups[1].Value
    if (-not $triple) {
        throw 'não consegui obter o host de rustc -vV'
    }

    # A versão do VERSIONINFO do setup precisa de quatro números; o release é estável.
    $version4 = ($version -replace '[-+].*$', '') + '.0'

    $stage = Join-Path $root "packaging\mustel-$version-$triple"
    if (-not (Test-Path $stage)) {
        Write-Host '==> stage ausente; rodando publish.ps1'
        & (Join-Path $root 'publish.ps1')
        if ($LASTEXITCODE -ne 0) {
            throw "publish.ps1 falhou (exit $LASTEXITCODE)"
        }
        if (-not (Test-Path $stage)) {
            throw "stage não encontrado depois do publish.ps1: $stage"
        }
    }

    # 6 primeiro: é o que o windows-latest do release traz. O 7 só é usado se for o único instalado.
    $candidates = @()
    if ($env:ProgramFiles) {
        $candidates += (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
        $candidates += (Join-Path $env:ProgramFiles 'Inno Setup 7\ISCC.exe')
    }
    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ($programFilesX86) {
        $candidates += (Join-Path $programFilesX86 'Inno Setup 6\ISCC.exe')
        $candidates += (Join-Path $programFilesX86 'Inno Setup 7\ISCC.exe')
    }
    # Instalação "current user only" (oferecida pelo Inno 7): não fica no Program Files.
    if ($env:LOCALAPPDATA) {
        $candidates += (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
        $candidates += (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 7\ISCC.exe')
    }
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $iscc) {
        throw 'Inno Setup 6/7 (ISCC.exe) não encontrado; instale o Inno Setup no Windows'
    }

    New-Item -ItemType Directory -Force -Path $out | Out-Null

    Write-Host "==> ISCC: $iscc"
    & $iscc `
        "/DMyAppVersion=$version" `
        "/DMyAppVersion4=$version4" `
        "/DMyAppStage=$stage" `
        "/DMyArch=$triple" `
        "/DMyOutputDir=$out" `
        $iss
    if ($LASTEXITCODE -ne 0) {
        throw "ISCC falhou (exit $LASTEXITCODE)"
    }

    $setup = Join-Path $out "mustel-$version-$triple-setup.exe"
    if (-not (Test-Path $setup)) {
        throw "setup não encontrado depois do ISCC: $setup"
    }

    $hash = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash.ToLowerInvariant()
    "$hash  $(Split-Path -Leaf $setup)" | Set-Content -Path "$setup.sha256" -Encoding ascii

    Write-Host ''
    Write-Host "instalador: $setup"
    Write-Host "sha256:     $hash"
}
finally {
    Pop-Location
}
