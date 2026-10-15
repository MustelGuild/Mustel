#Requires -Version 7

param(
    [ValidateSet('info', 'warn', 'error')]
    [string] $Severity
)

# Regras sem autofix — o dotnet format não consegue corrigir automaticamente
$noAutofix = @(
    'IDE0052', 'IDE0060', 'IDE1006'
    'CA1000', 'CA1002', 'CA1031', 'CA1305', 'CA1307', 'CA1308', 'CA1310'
    'CA1502', 'CA1506', 'CA1508', 'CA1819', 'CA1849', 'CA1859', 'CA1866'
    'CA1869', 'CA1873', 'CA2000', 'CA2100', 'CA2201', 'CA2227', 'CA2254'
    'CS0103', 'MSTEST0058'
)

# Regras com risco de cascata — têm autofix mas podem gerar novos erros ao serem aplicadas
$cascadeRisk = @(
    'IDE0063', 'IDE0200', 'IDE0290'
    'IDE0300', 'IDE0301', 'IDE0302', 'IDE0303', 'IDE0304', 'IDE0305', 'IDE0306'
)

$excludes = $noAutofix + $cascadeRisk

$projects = Get-ChildItem -Path $PSScriptRoot -Recurse -Filter '*.csproj' |
    Select-Object -ExpandProperty FullName

Write-Host "Formatando $($projects.Count) projeto(s) em paralelo...`n" -ForegroundColor Cyan

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

$severityArgs = if ($Severity) { @('--severity', $Severity) } else { @() }

$results = $projects | ForEach-Object -Parallel {
    $project = $_
    $name = Split-Path $project -Leaf

    $errFile = [System.IO.Path]::GetTempFileName()
    $output = & dotnet format $project --exclude-diagnostics @($using:excludes) @($using:severityArgs) 2>$errFile
    $stderr = Get-Content $errFile -Raw
    Remove-Item $errFile -Force

    [PSCustomObject]@{
        Project = $name
        Output  = ($output | Where-Object { $_ }) -join "`n"
        Error   = $stderr
        Success = $LASTEXITCODE -eq 0
    }
} -ThrottleLimit ([Environment]::ProcessorCount)

$stopwatch.Stop()

foreach ($result in $results)
{
    Write-Host "==========================" -ForegroundColor DarkGray
    $color = if ($result.Success)
    { 'Green'
    } else
    { 'Red'
    }
    Write-Host "Formatado: $($result.Project)" -ForegroundColor $color
    if ($result.Output)
    {
        Write-Host $result.Output -ForegroundColor DarkGray
    }
    if (-not $result.Success -and $result.Error)
    {
        Write-Host $result.Error -ForegroundColor Red
    }
    Write-Host ""
}

$elapsed = $stopwatch.Elapsed.ToString("mm\:ss\.ff")
Write-Host "Finalizado em $elapsed!" -ForegroundColor Cyan
