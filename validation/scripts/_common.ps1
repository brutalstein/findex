# Dot-sourced by the run-*.ps1 entry points. Resolves Python, the cargo target
# directory and the shared results directory for one validation run.
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Harness = Join-Path $Repo 'validation\harness'

$rustup = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path (Join-Path $rustup 'cargo.exe')) {
    # rust-toolchain.toml is honoured only through rustup proxies; a standalone
    # gnullvm install earlier on PATH produces binaries needing libunwind.dll.
    $env:PATH = "$rustup;$env:PATH"
}
if (-not $env:CARGO_TARGET_DIR) {
    # Build outside OneDrive/synced folders: sync clients lock and slow target/.
    $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'findex-build\target'
}

function Get-Python {
    foreach ($candidate in @(@('py', '-3.12'), @('python'))) {
        if (Get-Command $candidate[0] -ErrorAction SilentlyContinue) { return $candidate }
    }
    throw 'Python 3.12+ with psutil is required (py -3.12 -m pip install psutil).'
}

function Initialize-Results([string]$OutDir) {
    if (-not $OutDir) {
        $sha = (git -C $Repo rev-parse --short=12 HEAD).Trim()
        $OutDir = Join-Path $Repo ("validation\results\{0}-{1}" -f (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'), $sha)
    }
    New-Item -ItemType Directory -Force $OutDir | Out-Null
    $env:FINDEX_RESULTS_DIR = (Resolve-Path $OutDir).Path
    return $env:FINDEX_RESULTS_DIR
}

function Invoke-Harness([string]$Script, [string[]]$Arguments) {
    $python = Get-Python
    $exe = $python[0]
    $prefix = @($python | Select-Object -Skip 1)
    # Out-Host keeps harness stdout out of this function's return value.
    & $exe @prefix (Join-Path $Harness $Script) @Arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { Write-Warning "$Script exited with $LASTEXITCODE" }
    return $LASTEXITCODE
}

function Assert-ReleaseBuild {
    $exe = Join-Path $env:CARGO_TARGET_DIR 'release\findex.exe'
    if (-not (Test-Path $exe)) {
        cargo build --release --locked -p findex --manifest-path (Join-Path $Repo 'Cargo.toml')
        if ($LASTEXITCODE -ne 0) { throw 'release build failed' }
    }
}
