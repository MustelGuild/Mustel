# Prepara o bump de versão do Mustel para um PR, sem criar a tag.
#
# Uso:
#   ./prepare-release.ps1              # pergunta a versão
#   ./prepare-release.ps1 1.2.0        # versão direta
#   ./prepare-release.ps1 1.2.0 -Yes   # sem confirmação
#   ./prepare-release.ps1 1.2.0 -NoPush -SkipTests
#
# O fluxo é idempotente: se parar em qualquer etapa, rode de novo com a mesma
# versão. Ele retoma a branch release/v<versão> e continua do estado atual.
# Os checks (fmt, clippy e testes) rodam ANTES do bump/commit. A tag NÃO é
# criada: ela é criada depois do merge do PR, na main.
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$Version,
    [switch]$Yes,
    [switch]$NoPush,
    [switch]$SkipTests,
    [switch]$Help
)

$ErrorActionPreference = 'Stop'

function Write-Usage {
    Write-Host @'
Uso: ./prepare-release.ps1 [versão] [opções]

  versão        X.Y.Z (se omitida, o script pergunta)
  -Yes          não pede confirmação
  -NoPush       para depois do commit; não empurra nem abre PR
  -SkipTests    não roda cargo test (fmt e clippy continuam)
  -Help         mostra esta ajuda

O script cria ou retoma a branch release/v<versão>, roda fmt/clippy/testes
antes do bump, aplica o bump com `cargo release --no-tag`, atualiza
docs/CHANGELOG/SBOM e (opcionalmente) empurra a branch e abre o PR.
A tag v<versão> NÃO é criada; crie-a depois do merge do PR.

Se parar em qualquer etapa, rode de novo com a mesma versão: o script detecta
a branch e a versão já aplicada e continua. Os próprios scripts e o
cliff.toml/CHANGELOG.md precisam estar commitados antes da execução.
'@
}

function Fail([string]$Message) {
    throw $Message
}

function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        Fail "falhou (exit $LASTEXITCODE): $Command $($Arguments -join ' ')"
    }
}

function Get-CurrentVersion {
    $value = (& cargo pkgid -p mustel | Select-Object -Last 1) -replace '.*#', ''
    if ($LASTEXITCODE -ne 0 -or -not $value) {
        Fail 'não consegui obter a versão de cargo pkgid -p mustel'
    }
    return $value.Trim()
}

function Set-VersionInFile([string]$Path, [string]$Pattern, [string]$Replacement) {
    $full = Join-Path $root $Path
    if (-not (Test-Path $full)) {
        Write-Warning "arquivo não encontrado: $Path"
        return
    }
    $text = [IO.File]::ReadAllText($full)
    $new = [regex]::Replace($text, $Pattern, $Replacement)
    if ($new -ne $text) {
        [IO.File]::WriteAllText($full, $new, [Text.UTF8Encoding]::new($false))
        Write-Host "atualizado: $Path"
    } else {
        Write-Host "já na versão: $Path"
    }
}

function Invoke-Gate {
    Write-Host ''
    Write-Host '==> cargo fmt --all --check (antes do bump)' -ForegroundColor Cyan
    Invoke-Checked cargo @('fmt', '--all', '--check')

    Write-Host ''
    Write-Host '==> cargo clippy (antes do bump)' -ForegroundColor Cyan
    Invoke-Checked cargo @('clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings')

    if ($SkipTests) {
        Write-Host 'cargo test pulado (-SkipTests)' -ForegroundColor Yellow
        return
    }

    Write-Host ''
    Write-Host '==> cargo test (gate; stdin em NUL para os testes sem terminal)' -ForegroundColor Cyan
    if ($IsWindows -or $env:OS -eq 'Windows_NT') {
        # Os testes de ui_primitives exigem ausência de TTY; NUL evita herdar o console.
        & cmd.exe /c "cargo test --workspace < NUL"
        if ($LASTEXITCODE -ne 0) {
            Fail "cargo test falhou (exit $LASTEXITCODE)"
        }
    } else {
        & sh -c "cargo test --workspace -- --skip root_help_snapshot --skip query_run_help_snapshot --skip replay_database_pgpass < /dev/null"
        if ($LASTEXITCODE -ne 0) {
            Fail "cargo test falhou (exit $LASTEXITCODE)"
        }
    }
}

function Write-TagSteps {
    Write-Host ''
    Write-Host 'Depois do merge do PR, crie a tag:'
    Write-Host '  git switch main'
    Write-Host '  git pull --ff-only'
    Write-Host ('  git tag -a v' + $Version + ' -m "Mustel ' + $Version + '"')
    Write-Host ('  git push origin v' + $Version)
    Write-Host ''
    Write-Host ('Depois: Actions -> publish -> Run workflow -> v' + $Version)
}

if ($Help) {
    Write-Usage
    return
}

Push-Location
try {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Fail 'git não encontrado no PATH'
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Fail 'cargo não encontrado no PATH'
    }
    & git rev-parse --is-inside-work-tree *> $null
    if ($LASTEXITCODE -ne 0) {
        Fail 'não estamos num repositório Git'
    }
    $root = (git rev-parse --show-toplevel).Trim()
    Set-Location $root

    & cargo release --version *> $null
    if ($LASTEXITCODE -ne 0) {
        Fail "cargo-release não encontrado; instale com: cargo install cargo-release --version =1.1.7 --locked"
    }

    $currentBranch = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($currentBranch -eq 'HEAD') {
        Fail 'HEAD destacado; volte para um branch antes de preparar a release'
    }

    if (-not $Version) {
        $Version = Read-Host 'Versão a lançar (X.Y.Z)'
    }
    $Version = $Version.Trim()
    if ($Version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        Fail "versão inválida: '$Version' (use X.Y.Z estável; o workflow de release só aceita vX.Y.Z)"
    }

    $relBranch = "release/v$Version"
    & git show-ref --verify --quiet "refs/heads/$relBranch"
    if ($LASTEXITCODE -notin @(0, 1)) {
        Fail "git show-ref falhou (exit $LASTEXITCODE)"
    }
    $branchExists = ($LASTEXITCODE -eq 0)

    if ($currentBranch -ne $relBranch) {
        $dirty = @(git status --porcelain)
        if ($dirty.Count -gt 0) {
            Fail "a árvore precisa estar limpa para trocar para $relBranch. Mudanças pendentes:`n$($dirty -join "`n")"
        }
        if ($branchExists) {
            Write-Host "retomando a release existente: $relBranch" -ForegroundColor Cyan
            Invoke-Checked git @('switch', $relBranch)
        } else {
            if ($currentBranch -ne 'main') {
                Write-Warning "a branch de release parte de '$currentBranch', não de main; o PR para main vai incluir os commits dela."
            }
            Write-Host "criando a branch de release: $relBranch" -ForegroundColor Cyan
            Invoke-Checked git @('switch', '-c', $relBranch)
        }
    } else {
        Write-Host "retomando a release existente: $relBranch" -ForegroundColor Cyan
    }

    $currentVersion = Get-CurrentVersion
    $target = [version]$Version
    $current = [version]$currentVersion
    if ($target -lt $current) {
        Fail "a versão alvo ($Version) é menor que a atual ($currentVersion)"
    }
    $bumpDone = ($target -eq $current)
    $bumpPending = ($target -gt $current)

    $dirty = @(git status --porcelain)
    if ($dirty.Count -gt 0) {
        if ($bumpPending) {
            Fail "antes do bump, a árvore precisa estar limpa. Mudanças pendentes:`n$($dirty -join "`n")"
        }
        Write-Warning 'há mudanças não commitadas; só docs/CHANGELOG/SBOM do release serão commitados; as demais ficam fora do PR.'
    }

    Write-Host ''
    Write-Host "Versão atual: $currentVersion"
    Write-Host "Nova versão:  $Version"
    Write-Host "Branch:       $relBranch"
    if ($bumpDone) {
        Write-Host 'Bump:         já aplicado (retomando)'
    } else {
        Write-Host 'Bump:         pendente'
    }
    Write-Host 'Tag:          NÃO será criada (após o merge do PR)'
    if (-not $Yes) {
        $answer = Read-Host 'Continuar? [s/N]'
        if ($answer -notmatch '^(s|sim|y|yes)$') {
            Write-Host 'cancelado.'
            return
        }
    }

    Invoke-Gate

    if ($bumpPending) {
        Write-Host ''
        Write-Host '==> Ensaio do cargo release' -ForegroundColor Cyan
        $releaseArgs = @(
            $Version,
            '--workspace',
            '--allow-branch', $relBranch,
            '--no-tag',
            '--no-push',
            '--no-publish',
            '--no-confirm'
        )
        Invoke-Checked cargo (@('release') + $releaseArgs)

        Write-Host ''
        Write-Host '==> Aplicando o bump (sem tag)' -ForegroundColor Cyan
        & cargo (@('release') + $releaseArgs + '--execute')
        $releaseExit = $LASTEXITCODE
        $now = Get-CurrentVersion
        if ($now -ne $Version) {
            Fail "cargo release falhou (exit $releaseExit) e a versão ficou $now"
        }
        if ($releaseExit -ne 0) {
            Write-Warning "cargo release retornou $releaseExit, mas a versão ficou $Version; seguindo para o commit."
        }
    } else {
        Write-Host ''
        Write-Host "==> Bump já aplicado ($currentVersion); pulando cargo release" -ForegroundColor Cyan
    }

    Write-Host ''
    Write-Host '==> Ajustando a versão na documentação e nos fallbacks' -ForegroundColor Cyan
    Set-VersionInFile 'docs/contrato-de-interface.md' 'hoje `mustel [0-9]+\.[0-9]+\.[0-9]+`' ('hoje `mustel ' + $Version + '`')
    Set-VersionInFile 'docs/tutorials/primeiros-passos.md' 'algo como `mustel [0-9]+\.[0-9]+\.[0-9]+`' ('algo como `mustel ' + $Version + '`')
    Set-VersionInFile 'docs/how-to/publicar-release.md' 'ex\.: `v[0-9]+\.[0-9]+\.[0-9]+`' ('ex.: `v' + $Version + '`')
    Set-VersionInFile 'installer/mustel.iss' 'MyAppVersion "[0-9]+\.[0-9]+\.[0-9]+"' ('MyAppVersion "' + $Version + '"')
    Set-VersionInFile 'installer/mustel.iss' 'packaging\\mustel-[0-9]+\.[0-9]+\.[0-9]+-x86_64-pc-windows-msvc' ('packaging\mustel-' + $Version + '-x86_64-pc-windows-msvc')
    Set-VersionInFile 'installer/wine-test/README.md' 'packaging/mustel-[0-9]+\.[0-9]+\.[0-9]+-x86_64-pc-windows-msvc' ('packaging/mustel-' + $Version + '-x86_64-pc-windows-msvc')

    Write-Host ''
    Write-Host '==> Verificando o CHANGELOG' -ForegroundColor Cyan
    $changelog = Join-Path $root 'CHANGELOG.md'
    $sectionPattern = '^## \[v' + $Version + '\]'
    if ((Test-Path $changelog) -and (Select-String -Path $changelog -Pattern $sectionPattern -Quiet)) {
        Write-Host "CHANGELOG.md já tem a seção v$Version; mantendo"
    } else {
        if (-not (Get-Command git-cliff -ErrorAction SilentlyContinue)) {
            Fail "git-cliff não encontrado; instale com: cargo install git-cliff --version =2.14.2 --locked"
        }
        Invoke-Checked git-cliff @('--unreleased', '--tag', "v$Version", '--prepend', 'CHANGELOG.md')
        if (-not (Select-String -Path $changelog -Pattern $sectionPattern -Quiet)) {
            Fail "CHANGELOG.md não tem a seção v$Version"
        }
    }

    Write-Host ''
    Write-Host '==> Verificando o SBOM da raiz' -ForegroundColor Cyan
    $sbomPath = Join-Path $root 'sbom.cdx.json'
    $sbomDone = $false
    if (Test-Path $sbomPath) {
        $sbomDone = [IO.File]::ReadAllText($sbomPath).Contains('"version": "' + $Version + '"')
    }
    if ($sbomDone) {
        Write-Host "sbom.cdx.json já está na versão $Version; mantendo"
    } else {
        if (-not (Get-Command cargo-cyclonedx -ErrorAction SilentlyContinue)) {
            Fail "cargo-cyclonedx não encontrado; instale com: cargo install cargo-cyclonedx --version =0.5.9 --locked"
        }
        Invoke-Checked cargo @('cyclonedx', '-f', 'json', '--spec-version', '1.5', '--override-filename', 'sbom-root.cdx', '--target', 'all')
        $generated = Join-Path $root 'crates\mustel\sbom-root.cdx.json'
        if (-not (Test-Path $generated)) {
            Fail "SBOM não encontrado: $generated"
        }
        Copy-Item -Force $generated $sbomPath
        Get-ChildItem -Path (Join-Path $root 'crates') -Filter 'sbom-root.cdx.json' -Recurse -Depth 1 -File |
            Remove-Item -Force
        if (-not [IO.File]::ReadAllText($sbomPath).Contains('"version": "' + $Version + '"')) {
            Fail "o SBOM da raiz não contém a versão $Version"
        }
    }

    Write-Host ''
    Write-Host '==> Incluindo docs, CHANGELOG e SBOM no commit' -ForegroundColor Cyan
    $paths = @(
        'docs/contrato-de-interface.md',
        'docs/tutorials/primeiros-passos.md',
        'docs/how-to/publicar-release.md',
        'installer/mustel.iss',
        'installer/wine-test/README.md',
        'CHANGELOG.md',
        'sbom.cdx.json'
    )
    Invoke-Checked git (@('add', '--') + $paths)
    & git diff --cached --quiet
    if ($LASTEXITCODE -eq 1) {
        $headSubject = (git log -1 --format=%s).Trim()
        $staged = @(git diff --cached --name-only)
        if ($headSubject -eq "Bump Mustel to $Version") {
            Invoke-Checked git @('commit', '--amend', '--no-edit')
        } elseif ($staged -contains 'Cargo.toml') {
            Invoke-Checked git @('commit', '-m', "Bump Mustel to $Version")
        } else {
            Invoke-Checked git @('commit', '-m', "Prepara release v$Version")
        }
    } elseif ($LASTEXITCODE -ne 0) {
        Fail "git diff --cached falhou (exit $LASTEXITCODE)"
    } else {
        Write-Host 'nenhuma mudança extra para incluir'
    }

    if (-not $SkipTests) {
        Write-Host ''
        Write-Host '==> contract_decisions (links da documentação)' -ForegroundColor Cyan
        Invoke-Checked cargo @('test', '-p', 'mustel-core', '--test', 'contract_decisions')
    }

    if ($NoPush) {
        Write-Host ''
        Write-Host "Branch: $relBranch"
        Write-Host 'Tag NÃO criada.'
        Write-Host "Para abrir o PR manualmente: git push -u origin $relBranch"
        Write-TagSteps
        return
    }

    if (-not $Yes) {
        $answer = Read-Host 'Empurrar a branch e abrir o PR? [s/N]'
        if ($answer -notmatch '^(s|sim|y|yes)$') {
            Write-Host 'branch não empurrada.'
            Write-TagSteps
            return
        }
    }

    & git rev-parse --abbrev-ref --symbolic-full-name '@{u}' *> $null
    if ($LASTEXITCODE -ne 0) {
        Invoke-Checked git @('push', '-u', 'origin', $relBranch)
    } else {
        Invoke-Checked git @('push', 'origin', $relBranch)
    }

    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if ($gh) {
        $prUrl = & gh pr view $relBranch --json url --jq .url 2>$null
        if ($LASTEXITCODE -eq 0 -and $prUrl) {
            Write-Host "PR já existe: $prUrl"
        } else {
            $body = @"
Bump da versão para $Version.

Base do PR: $relBranch -> main.

Inclui:
- Cargo.toml, Cargo.lock e dependências internas;
- docs, CHANGELOG e sbom.cdx.json;
- fallbacks do instalador.

A tag v$Version NÃO é criada aqui; ela será criada após o merge do PR.
"@
            Invoke-Checked gh @(
                'pr', 'create',
                '--base', 'main',
                '--head', $relBranch,
                '--title', "Bump Mustel to $Version",
                '--body', $body
            )
        }
    } else {
        Write-Host "gh não encontrado. Abra o PR de $relBranch para main no GitHub."
    }

    Write-Host ''
    Write-Host "Branch: $relBranch"
    Write-Host "Commit: $(git rev-parse --short HEAD)"
    Write-Host "Tag:    não criada (crie v$Version após o merge do PR)"
    Write-TagSteps
}
catch {
    Write-Host "erro: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}
finally {
    Pop-Location
}
