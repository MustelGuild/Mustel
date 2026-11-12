# Prepara o bump de versão do Mustel para um PR, sem criar a tag.
#
# Uso:
#   ./prepare-release.ps1              # pergunta a versão
#   ./prepare-release.ps1 1.2.0        # versão direta
#   ./prepare-release.ps1 1.2.0 -Yes   # sem confirmação
#   ./prepare-release.ps1 1.2.0 -NoPush -SkipTests
#
# O script cria a branch release/v<versão>, roda `cargo release` com --no-tag,
# atualiza docs e o sbom.cdx.json, roda o gate e (opcionalmente) empurra a branch
# e abre o PR. A tag v<versão> NÃO é criada: ela é criada depois do merge do PR.
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

O script cria a branch release/v<versão>, roda `cargo release` com --no-tag,
atualiza docs/sbom, roda o gate e (opcionalmente) empurra a branch e abre o PR.
A tag v<versão> NÃO é criada; crie-a depois do merge do PR.

O script parte do branch atual e abre o PR contra main. Num branch de feature,
os commits dele entram no PR; para um PR só do bump, rode a partir de main.

Os dois scripts precisam estar commitados antes da execução: o cargo release
recusa árvore suja, inclusive arquivo não rastreado.
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

function Replace-Phrase([string]$Path, [string]$Old, [string]$New) {
    $full = Join-Path $root $Path
    if (-not (Test-Path $full)) {
        Write-Warning "arquivo não encontrado: $Path"
        return
    }
    $text = [IO.File]::ReadAllText($full)
    if (-not $text.Contains($Old)) {
        Write-Warning "frase não encontrada em ${Path}: $Old"
        return
    }
    $text = $text.Replace($Old, $New)
    [IO.File]::WriteAllText($full, $text, [Text.UTF8Encoding]::new($false))
    Write-Host "atualizado: $Path"
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

$root = $PSScriptRoot
Push-Location $root
try {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Fail 'git não encontrado no PATH'
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Fail 'cargo não encontrado no PATH'
    }
    & cargo release --version *> $null
    if ($LASTEXITCODE -ne 0) {
        Fail "cargo-release não encontrado; instale com: cargo install cargo-release --version =1.1.7 --locked"
    }

    & git rev-parse --is-inside-work-tree *> $null
    if ($LASTEXITCODE -ne 0) {
        Fail 'não estamos num repositório Git'
    }
    $currentBranch = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($currentBranch -eq 'HEAD') {
        Fail 'HEAD destacado; volte para um branch antes de preparar a release'
    }
    if ($currentBranch -ne 'main') {
        Write-Warning "a branch de release parte de '$currentBranch', não de main; o PR para main vai incluir os commits dela."
    }
    $dirty = git status --porcelain
    if ($dirty) {
        Fail "a árvore precisa estar limpa (inclui os próprios scripts: commite-os antes de rodar). Mudanças pendentes:`n$($dirty -join "`n")"
    }

    $old = (& cargo pkgid -p mustel | Select-Object -Last 1) -replace '.*#', ''
    if ($LASTEXITCODE -ne 0 -or -not $old) {
        Fail 'não consegui obter a versão de cargo pkgid -p mustel'
    }
    $old = $old.Trim()

    if (-not $Version) {
        $Version = Read-Host 'Versão a lançar (X.Y.Z)'
    }
    $Version = $Version.Trim()
    if ($Version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
        Fail "versão inválida: '$Version' (use X.Y.Z estável; o workflow de release só aceita vX.Y.Z)"
    }
    if ([version]$Version -le [version]$old) {
        Fail "a nova versão ($Version) precisa ser maior que a atual ($old)"
    }

    $relBranch = "release/v$Version"
    $existing = git branch --list $relBranch
    if ($existing) {
        Fail "a branch $relBranch já existe; apague-a ou use outra versão"
    }

    Write-Host ''
    Write-Host "Versão atual: $old"
    Write-Host "Nova versão:  $Version"
    Write-Host "Branch:       $relBranch"
    Write-Host 'Tag:          NÃO será criada (após o merge do PR)'
    if (-not $Yes) {
        $answer = Read-Host 'Continuar? [s/N]'
        if ($answer -notmatch '^(s|sim|y|yes)$') {
            Write-Host 'cancelado.'
            return
        }
    }

    Invoke-Checked git @('switch', '-c', $relBranch)

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
    Invoke-Checked cargo (@('release') + $releaseArgs + '--execute')

    $new = (& cargo pkgid -p mustel | Select-Object -Last 1) -replace '.*#', ''
    if ($LASTEXITCODE -ne 0 -or $new.Trim() -ne $Version) {
        Fail "o Cargo.toml não ficou em $Version (cargo pkgid: $new)"
    }

    Write-Host ''
    Write-Host '==> Atualizando documentação e fallbacks' -ForegroundColor Cyan
    Replace-Phrase 'docs/contrato-de-interface.md' ('hoje `mustel ' + $old + '`') ('hoje `mustel ' + $Version + '`')
    Replace-Phrase 'docs/tutorials/primeiros-passos.md' ('algo como `mustel ' + $old + '`') ('algo como `mustel ' + $Version + '`')
    Replace-Phrase 'docs/how-to/publicar-release.md' ('ex.: `v' + $old + '`') ('ex.: `v' + $Version + '`')
    Replace-Phrase 'installer/mustel.iss' ('MyAppVersion "' + $old + '"') ('MyAppVersion "' + $Version + '"')
    Replace-Phrase 'installer/mustel.iss' ('packaging\mustel-' + $old + '-x86_64-pc-windows-msvc') ('packaging\mustel-' + $Version + '-x86_64-pc-windows-msvc')
    Replace-Phrase 'installer/wine-test/README.md' ('packaging/mustel-' + $old + '-x86_64-pc-windows-msvc') ('packaging/mustel-' + $Version + '-x86_64-pc-windows-msvc')

    Write-Host ''
    Write-Host '==> Gerando o CHANGELOG' -ForegroundColor Cyan
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
    Write-Host '==> Regenerando o SBOM da raiz' -ForegroundColor Cyan
    if (-not (Get-Command cargo-cyclonedx -ErrorAction SilentlyContinue)) {
        Fail "cargo-cyclonedx não encontrado; instale com: cargo install cargo-cyclonedx --version =0.5.9 --locked"
    }
    Invoke-Checked cargo @('cyclonedx', '-f', 'json', '--spec-version', '1.5', '--override-filename', 'sbom-root.cdx', '--target', 'all')
    $sbom = Join-Path $root 'crates\mustel\sbom-root.cdx.json'
    if (-not (Test-Path $sbom)) {
        Fail "SBOM não encontrado: $sbom"
    }
    Copy-Item -Force $sbom (Join-Path $root 'sbom.cdx.json')
    Get-ChildItem -Path (Join-Path $root 'crates') -Filter 'sbom-root.cdx.json' -Recurse -Depth 1 -File |
        Remove-Item -Force
    $rootSbom = [IO.File]::ReadAllText((Join-Path $root 'sbom.cdx.json'))
    if (-not $rootSbom.Contains('"version": "' + $Version + '"')) {
        Fail "o SBOM da raiz não contém a versão $Version"
    }

    Write-Host ''
    Write-Host '==> Incluindo docs, CHANGELOG e SBOM no commit do bump' -ForegroundColor Cyan
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
        Invoke-Checked git @('commit', '--amend', '--no-edit')
    } elseif ($LASTEXITCODE -ne 0) {
        Fail "git diff --cached falhou (exit $LASTEXITCODE)"
    } else {
        Write-Host 'nenhuma mudança extra para incluir'
    }

    Write-Host ''
    Write-Host '==> cargo fmt --all --check' -ForegroundColor Cyan
    Invoke-Checked cargo @('fmt', '--all', '--check')

    Write-Host ''
    Write-Host '==> cargo clippy' -ForegroundColor Cyan
    Invoke-Checked cargo @('clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings')

    if (-not $SkipTests) {
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

    if ($NoPush) {
        Write-Host ''
        Write-Host "Branch criada: $relBranch"
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

    Invoke-Checked git @('push', '-u', 'origin', $relBranch)

    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if ($gh) {
        $body = @"
Bump da versão para $Version.

Base do PR: $currentBranch -> main.

Inclui:
- Cargo.toml, Cargo.lock e dependências internas;
- docs (contrato, tutorial e how-to de release);
- sbom.cdx.json da raiz;
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
