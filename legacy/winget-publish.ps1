#Requires -Version 7.0
<#
.SYNOPSIS
    Publica um novo pacote no winget usando wingetcreate.

.DESCRIPTION
    Guia o processo completo de publicação de um novo pacote no repositório
    winget-pkgs via wingetcreate: valida dependências, coleta informações,
    gera manifesto, valida e abre o Pull Request no GitHub.

.EXAMPLE
    ./winget-publish.ps1
#>

$ErrorActionPreference = "Stop"

# ─────────────────────────────────────────────────────────────
# Helpers de UI
# ─────────────────────────────────────────────────────────────

function Write-Header
{
    param([string]$Mensagem)
    Write-Host ""
    Write-Host ("=" * 60) -ForegroundColor Cyan
    Write-Host "  $Mensagem" -ForegroundColor Cyan
    Write-Host ("=" * 60) -ForegroundColor Cyan
    Write-Host ""
}

function Write-Passo
{
    param([int]$Numero, [string]$Descricao)
    Write-Host ""
    Write-Host "[ Passo $Numero ] $Descricao" -ForegroundColor Yellow
    Write-Host ("-" * 50) -ForegroundColor DarkGray
}

function Write-Ok
{ param([string]$m) Write-Host "[OK]  $m" -ForegroundColor Green
}
function Write-Erro
{ param([string]$m) Write-Host "[ERRO] $m" -ForegroundColor Red
}
function Write-Info
{ param([string]$m) Write-Host "[INFO] $m" -ForegroundColor Cyan
}
function Write-Aviso
{ param([string]$m) Write-Host "[AVISO] $m" -ForegroundColor Yellow
}

function Read-Input
{
    param(
        [string]$Prompt,
        [string]$Default = "",
        [switch]$Required
    )
    do
    {
        if ($Default)
        {
            $userInput = Read-Host "$Prompt [$Default]"
            if ([string]::IsNullOrWhiteSpace($userInput))
            { $userInput = $Default
            }
        } else
        {
            $userInput = Read-Host $Prompt
        }

        if ($Required -and [string]::IsNullOrWhiteSpace($userInput))
        {
            Write-Aviso "Este campo é obrigatório."
        }
    } while ($Required -and [string]::IsNullOrWhiteSpace($userInput))

    return $userInput.Trim()
}

function Confirmar
{
    param([string]$Pergunta, [bool]$PadraoSim = $true)
    $opcoes = if ($PadraoSim)
    { "[S/n]"
    } else
    { "[s/N]"
    }
    $resposta = Read-Host "$Pergunta $opcoes"
    if ([string]::IsNullOrWhiteSpace($resposta))
    { return $PadraoSim
    }
    return $resposta -match '^[sS]'
}

# ─────────────────────────────────────────────────────────────
# Verificação de dependências
# ─────────────────────────────────────────────────────────────

function Test-Dependencias
{
    Write-Passo 1 "Verificando dependências"

    $ok = $true

    # wingetcreate
    if (Get-Command wingetcreate -ErrorAction SilentlyContinue)
    {
        $versao = (wingetcreate --version 2>&1) | Select-String -Pattern '[\d\.]+'
        Write-Ok "wingetcreate encontrado: $($versao.Matches[0].Value)"
    } else
    {
        Write-Erro "wingetcreate não encontrado."
        Write-Info "Instale via: winget install Microsoft.WingetCreate"
        $ok = $false
    }

    # gh (GitHub CLI) — necessário para abrir PR automaticamente
    if (Get-Command gh -ErrorAction SilentlyContinue)
    {
        $ghVer = (gh --version 2>&1 | Select-String 'gh version ([\d\.]+)').Matches[0].Groups[1].Value
        Write-Ok "GitHub CLI (gh) encontrado: $ghVer"

        # Verifica autenticação
        $null = gh auth status 2>&1
        if ($LASTEXITCODE -eq 0)
        {
            Write-Ok "GitHub CLI autenticado."
        } else
        {
            Write-Aviso "GitHub CLI não autenticado. Execute: gh auth login"
            $ok = $false
        }
    } else
    {
        Write-Erro "GitHub CLI (gh) não encontrado."
        Write-Host ""
        if (Confirmar "Deseja instalar o GitHub CLI agora via winget?")
        {
            Write-Info "Executando: winget install GitHub.cli ..."
            winget install --id GitHub.cli --silent --accept-package-agreements --accept-source-agreements
            if ($LASTEXITCODE -eq 0)
            {
                Write-Ok "GitHub CLI instalado com sucesso."
                Write-Aviso "Reinicie o terminal e execute este script novamente para que o 'gh' seja reconhecido."
            } else
            {
                Write-Erro "Falha ao instalar o GitHub CLI (código $LASTEXITCODE)."
                Write-Info "Instale manualmente: winget install GitHub.cli"
            }
        } else
        {
            Write-Info "Instale manualmente: winget install GitHub.cli"
        }
        $ok = $false
    }

    if (-not $ok)
    {
        Write-Host ""
        Write-Erro "Resolva as dependências acima e execute o script novamente."
        exit 3
    }

    Write-Host ""
    Write-Ok "Todas as dependências estão disponíveis."
}

# ─────────────────────────────────────────────────────────────
# Busca informações da última release no GitHub
# ─────────────────────────────────────────────────────────────

function Get-LatestRelease
{
    Write-Info "Buscando última release em liphvf/FurLab via GitHub CLI ..."

    $json = gh release view --repo liphvf/FurLab --json tagName,assets 2>&1
    if ($LASTEXITCODE -ne 0)
    {
        Write-Aviso "Não foi possível buscar a última release."
        Write-Info "Verifique se o repositório existe e o gh está autenticado."
        Write-Info "Você precisará informar as informações manualmente."
        $script:PackageId = $null
        $script:PackageVersion = $null
        $script:InstallerUrls = $null
        return
    }

    $release = $json | ConvertFrom-Json

    $script:PackageId = "Mustel.CLI"
    $script:PackageVersion = $release.tagName.TrimStart('v')
    $script:InstallerUrls = [System.Collections.Generic.List[string]]::new()

    foreach ($asset in $release.assets)
    {
        if ($asset.downloadUrl -match '\.(exe|msi|msix|appx|zip)(\?|$)')
        {
            $script:InstallerUrls.Add($asset.downloadUrl)
        }
    }

    if ($script:InstallerUrls.Count -eq 0)
    {
        Write-Aviso "Nenhum instalador encontrado na release $($release.tagName)."
        Write-Info "Você precisará informar as URLs manualmente."
    } else
    {
        Write-Ok "Release $($release.tagName) encontrada com $($script:InstallerUrls.Count) instalador(es)."
    }
}

# ─────────────────────────────────────────────────────────────
# Coleta de informações do pacote
# ─────────────────────────────────────────────────────────────

function Get-InformacoesPacote
{
    Write-Passo 2 "Informações do pacote"

    Get-LatestRelease

    Write-Host ""
    Write-Info "Identificador do pacote: Mustel.CLI"
    $script:PackageId = "Mustel.CLI"

    if ($script:PackageVersion)
    {
        $sugestao = $script:PackageVersion
        Write-Host ""
        Write-Info "Sugestão de versão (última release): $sugestao"
        $versao = Read-Input -Prompt "Versão do pacote (ENTER para aceitar sugestão)" -Default $sugestao
        $script:PackageVersion = $versao
    } else
    {
        $script:PackageVersion = Read-Input -Prompt "Versão do pacote (ex: 1.0.0)" -Required
    }

    while ($script:PackageVersion -notmatch '^\d+(\.\d+){0,3}(-[\w\.\-]+)?$')
    {
        Write-Aviso "Versão inválida. Use formato semântico: 1.0.0 ou 1.2.3.4"
        $script:PackageVersion = Read-Input -Prompt "Versão do pacote" -Required
    }

    Write-Host ""

    if ($script:InstallerUrls -and $script:InstallerUrls.Count -gt 0)
    {
        Write-Info "URLs de instaladores encontradas na release:"
        foreach ($url in $script:InstallerUrls)
        {
            Write-Host "  - $url" -ForegroundColor Gray
        }
        Write-Host ""
        if (Confirmar "Usar estas URLs? [S/n]")
        {
            return
        }
    }

    Write-Host ""
    Write-Info "Sugestão de URL para instalador (montada a partir da release):"
    $sugestaoUrl = "https://github.com/liphvf/FurLab/releases/download/v$($script:PackageVersion)/mustel.exe"
    Write-Host "  $sugestaoUrl" -ForegroundColor Gray
    Write-Host ""

    if (Confirmar "Usar esta URL sugerida? [S/n]")
    {
        $script:InstallerUrls = [System.Collections.Generic.List[string]]::new()
        $script:InstallerUrls.Add($sugestaoUrl)
        return
    }

    Write-Info "Informe as URLs dos instaladores (exe, msi, msix, appx, zip)."
    Write-Info "Pressione ENTER em branco para finalizar a lista."
    Write-Host ""

    $script:InstallerUrls = [System.Collections.Generic.List[string]]::new()
    $idx = 1
    do
    {
        $url = Read-Input -Prompt "URL do instalador $idx (ENTER para finalizar)"
        if (-not [string]::IsNullOrWhiteSpace($url))
        {
            if ($url -notmatch '^https?://')
            {
                Write-Aviso "A URL deve começar com http:// ou https://"
            } else
            {
                $script:InstallerUrls.Add($url)
                $idx++
            }
        }
    } while (-not [string]::IsNullOrWhiteSpace($url))

    if ($script:InstallerUrls.Count -eq 0)
    {
        Write-Erro "Pelo menos uma URL de instalador é obrigatória."
        exit 1
    }

    Write-Ok "$($script:InstallerUrls.Count) URL(s) informada(s)."
}

# ─────────────────────────────────────────────────────────────
# Token GitHub (para wingetcreate submit)
# ─────────────────────────────────────────────────────────────

function Get-GitHubToken
{
    Write-Passo 3 "Token do GitHub"

    Write-Info "O wingetcreate precisa de um token GitHub (PAT) para abrir o PR."
    Write-Info "Escopo necessário: 'public_repo'  (ou 'repo' para repos privados)"
    Write-Info "Crie em: https://github.com/settings/tokens/new"
    Write-Host ""

    # Tenta ler token salvo pelo gh CLI
    $ghToken = gh auth token 2>&1
    if ($LASTEXITCODE -eq 0 -and $ghToken)
    {
        Write-Ok "Token obtido automaticamente via GitHub CLI."
        $script:GhToken = $ghToken
        return
    }

    $script:GhToken = Read-Input -Prompt "Cole seu GitHub Personal Access Token" -Required
}

# ─────────────────────────────────────────────────────────────
# Diretório de saída para o manifesto
# ─────────────────────────────────────────────────────────────

function Set-DiretorioSaida
{
    $script:OutputDir = Join-Path $PSScriptRoot "packaging\winget"

    if (-not (Test-Path $script:OutputDir))
    {
        New-Item -ItemType Directory -Path $script:OutputDir -Force | Out-Null
    }
}

# ─────────────────────────────────────────────────────────────
# Revisão antes de prosseguir
# ─────────────────────────────────────────────────────────────

function Confirm-Revisao
{
    Write-Passo 4 "Revisão das informações"

    Write-Host ""
    Write-Host "  Identificador  : " -NoNewline; Write-Host $script:PackageId -ForegroundColor White
    Write-Host "  Versão         : " -NoNewline; Write-Host $script:PackageVersion -ForegroundColor White
    Write-Host "  URLs           :" -ForegroundColor White
    foreach ($url in $script:InstallerUrls)
    {
        Write-Host "    - $url" -ForegroundColor Gray
    }
    Write-Host "  Saída          : " -NoNewline; Write-Host $script:OutputDir -ForegroundColor White
    Write-Host ""

    if (-not (Confirmar "As informações estão corretas? Deseja continuar?"))
    {
        Write-Host ""
        Write-Aviso "Operação cancelada pelo usuário."
        exit 130
    }
}

# ─────────────────────────────────────────────────────────────
# Geração manual do manifesto YAML
# ─────────────────────────────────────────────────────────────

function Get-InstallerSha256
{
    param([string]$Url)

    Write-Info "Baixando instalador para calcular SHA256: $Url"

    $tempFile = Join-Path $env:TEMP "winget-temp-$([guid]::NewGuid().ToString().Substring(0,8)).exe"

    try
    {
        Invoke-WebRequest -Uri $Url -OutFile $tempFile -UseBasicParsing -ErrorAction Stop
        $hash = (Get-FileHash -Path $tempFile -Algorithm SHA256).Hash.ToLower()
        Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
        return $hash
    } catch
    {
        Write-Aviso "Não foi possível baixar o instalador ($($_.Exception.Message))."
        Write-Info "Calculando SHA256 localmente..."
        $path = Read-Input -Prompt "Caminho completo do arquivo instalador" -Required
        if (Test-Path $path)
        {
            return (Get-FileHash -Path $path -Algorithm SHA256).Hash.ToLower()
        }
        Write-Erro "Arquivo não encontrado."
        exit 1
    }
}

function Get-InstallerType
{
    param([string]$Url)

    $ext = [System.IO.Path]::GetExtension($Url.Split('?')[0]).ToLower()
    switch ($ext)
    {
        '.msi'
        { return 'msi'
        }
        '.msix'
        { return 'msix'
        }
        '.appx'
        { return 'appx'
        }
        '.exe'
        { return 'exe'
        }
        default
        { return 'exe'
        }
    }
}

function Get-InstallerScope
{
    param([string]$InstallerType)
    if ($InstallerType -eq 'msi')
    { return 'machine'
    }
    return 'user'
}

function Get-InstallerArchitecture
{
    param([string]$Url)
    $lower = $Url.ToLower()
    if ($lower -match 'x64|amd64|win-x64')
    { return 'x64'
    }
    if ($lower -match 'x86|win32|win-x86')
    { return 'x86'
    }
    if ($lower -match 'arm64')
    { return 'arm64'
    }
    return 'x64'
}

function New-ManifestYaml
{
    Write-Passo 5 "Gerando manifesto YAML"

    $versionDir = Join-Path $script:OutputDir $script:PackageVersion
    if (-not (Test-Path $versionDir))
    {
        New-Item -ItemType Directory -Path $versionDir -Force | Out-Null
    }

    # DefaultLocale manifest
    $defaultLocalePath = Join-Path $versionDir "Mustel.CLI.locale.en-US.yaml"
    $defaultLocaleYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.1.12.0.schema.json
PackageIdentifier: $($script:PackageId)
PackageVersion: $($script:PackageVersion)
PackageLocale: en-US
Publisher: Mustel
PackageName: Mustel
ShortDescription: Command-line utility for PostgreSQL queries, database backup/restore and server settings.
License: GPL-3.0
ManifestType: defaultLocale
ManifestVersion: 1.12.0
"@
    Set-Content -Path $defaultLocalePath -Value $defaultLocaleYaml -Encoding UTF8
    Write-Ok "Manifesto locale gerado: $defaultLocalePath"

    # Version manifest
    $versionManifestPath = Join-Path $versionDir "Mustel.CLI.yaml"
    $versionYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.1.12.0.schema.json
PackageIdentifier: $($script:PackageId)
PackageVersion: $($script:PackageVersion)
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.12.0
"@
    Set-Content -Path $versionManifestPath -Value $versionYaml -Encoding UTF8
    Write-Ok "Manifesto version gerado: $versionManifestPath"

    # Installer manifest
    $installerPath = Join-Path $versionDir "Mustel.CLI.installer.yaml"
    $installerEntries = [System.Collections.Generic.List[string]]::new()
    $idx = 0

    foreach ($url in $script:InstallerUrls)
    {
        $idx++
        $installerType = Get-InstallerType $url
        $architecture = Get-InstallerArchitecture $url
        $sha256 = Get-InstallerSha256 $url

        $entry = "  - Architecture: ${architecture}`n    InstallerType: portable`n    InstallerUrl: ${url}`n    InstallerSha256: ${sha256}`n    Commands:`n    - mustel"
        $installerEntries.Add($entry)

        Write-Ok "Installer ${idx}: ${architecture} / ${installerType} / SHA256: $($sha256.Substring(0,16))..."
    }

    $installerYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.1.12.0.schema.json
PackageIdentifier: $($script:PackageId)
PackageVersion: $($script:PackageVersion)
InstallerLocale: en-US
Installers:
$($installerEntries -join "`n")
ManifestType: installer
ManifestVersion: 1.12.0
"@
    Set-Content -Path $installerPath -Value $installerYaml -Encoding UTF8

    Write-Host ""
    Write-Ok "Manifestos gerados em: $versionDir"
}

# ─────────────────────────────────────────────────────────────
# Validação do manifesto
# ─────────────────────────────────────────────────────────────

function Invoke-Validacao
{
    Write-Passo 6 "Validando manifesto"

    $manifestPath = Join-Path $script:OutputDir $script:PackageVersion

    if (-not (Test-Path $manifestPath))
    {
        Write-Aviso "Não foi possível localizar o manifesto gerado para validação automática."
        Write-Info "Execute manualmente: winget validate <caminho>"
        return
    }

    Write-Info "Validando: $manifestPath"
    & winget validate $manifestPath

    if ($LASTEXITCODE -ne 0)
    {
        Write-Erro "Validação falhou. Corrija os erros no manifesto e reenvie."
        if (-not (Confirmar "Deseja continuar mesmo com erros de validação?" $false))
        {
            exit 1
        }
    } else
    {
        Write-Ok "Manifesto válido."
    }
}

# ─────────────────────────────────────────────────────────────
# Submissão do PR
# ─────────────────────────────────────────────────────────────

function Invoke-Submit
{
    Write-Passo 7 "Submetendo Pull Request ao winget-pkgs"

    Write-Host ""
    Write-Info "O wingetcreate submit vai:"
    Write-Host "  1. Fazer fork do microsoft/winget-pkgs (se necessário)" -ForegroundColor Gray
    Write-Host "  2. Criar uma branch com o manifesto" -ForegroundColor Gray
    Write-Host "  3. Abrir um Pull Request" -ForegroundColor Gray
    Write-Host ""

    if (-not (Confirmar "Deseja enviar o PR agora?"))
    {
        Write-Aviso "Submissão cancelada. O manifesto está salvo em: $($script:OutputDir)"
        Write-Info "Para enviar manualmente: wingetcreate submit --token <TOKEN> <caminho>"
        exit 130
    }

    # Localiza manifesto gerado
    $manifestPath = Join-Path $script:OutputDir $script:PackageVersion

    if (-not (Test-Path $manifestPath))
    {
        Write-Erro "Manifesto não encontrado em: $manifestPath"
        exit 1
    }

    Write-Info "Executando wingetcreate submit ..."
    & wingetcreate submit --token $script:GhToken $manifestPath

    if ($LASTEXITCODE -ne 0)
    {
        Write-Erro "Submissão falhou (código $LASTEXITCODE)."
        Write-Info "Verifique o token e tente: wingetcreate submit --token <TOKEN> `"$manifestPath`""
        exit 1
    }

    Write-Host ""
    Write-Ok "Pull Request aberto com sucesso!"
    Write-Info "Acompanhe em: https://github.com/microsoft/winget-pkgs/pulls"
}

# ─────────────────────────────────────────────────────────────
# Sumário final
# ─────────────────────────────────────────────────────────────

function Write-Sumario
{
    Write-Host ""
    Write-Host ("=" * 60) -ForegroundColor Green
    Write-Host "  Publicacao concluida!" -ForegroundColor Green
    Write-Host ("=" * 60) -ForegroundColor Green
    Write-Host ""
    Write-Host "  Pacote   : $($script:PackageId)" -ForegroundColor White
    Write-Host "  Versao   : $($script:PackageVersion)" -ForegroundColor White
    Write-Host "  Manifesto: $($script:OutputDir)" -ForegroundColor White
    Write-Host ""
    Write-Host "  Proximos passos:" -ForegroundColor Cyan
    Write-Host "  - Aguarde a revisao automatica (pipelines) do winget-pkgs."
    Write-Host "  - Um mantenedor revisara e aprovara seu PR."
    Write-Host "  - Apos merge, o pacote fica disponivel em horas via winget."
    Write-Host ""
}

# ─────────────────────────────────────────────────────────────
# Fluxo principal
# ─────────────────────────────────────────────────────────────

Write-Header "Publicacao de Pacote no Winget"
Write-Info "Este script guia o processo completo via wingetcreate."
Write-Info "Voce precisara de: URL(s) do instalador e um GitHub PAT (public_repo)."

Test-Dependencias
Get-InformacoesPacote
Get-GitHubToken
Set-DiretorioSaida
Confirm-Revisao
New-ManifestYaml
Invoke-Validacao
Invoke-Submit
Write-Sumario
