# Findex plugin MCP bootstrap. Local, fail-closed, no downloads or credential access.
# Codex and Claude Code start this from the user's current project directory.
$ErrorActionPreference = 'Stop'
$pluginRoot = Split-Path -Parent $PSScriptRoot

function Select-FindexBinary {
    # An explicitly configured absolute path wins (for plugin development).
    $override = [Environment]::GetEnvironmentVariable('FINDEX_BIN')
    if (-not [string]::IsNullOrWhiteSpace($override)) {
        if (-not [IO.Path]::IsPathRooted($override) -or -not (Test-Path -LiteralPath $override -PathType Leaf)) {
            throw 'FINDEX_BIN must point to an existing absolute findex.exe path.'
        }
        if ([IO.Path]::GetFileName($override) -ine 'findex.exe') {
            throw 'FINDEX_BIN must name a findex.exe binary.'
        }
        return (Resolve-Path -LiteralPath $override).ProviderPath
    }

    # Marketplace-installed release bundles can carry their native engine.
    $bundled = Join-Path $pluginRoot 'bin\findex.exe'
    if (Test-Path -LiteralPath $bundled -PathType Leaf) {
        return (Resolve-Path -LiteralPath $bundled).ProviderPath
    }

    # Existing Findex CLI release ZIP keeps plugin/ adjacent to findex.exe.
    $adjacent = Join-Path (Split-Path -Parent $pluginRoot) 'findex.exe'
    if (Test-Path -LiteralPath $adjacent -PathType Leaf) {
        return (Resolve-Path -LiteralPath $adjacent).ProviderPath
    }

    # Resolve from PATH, not from the untrusted repository working directory.
    $command = Get-Command -Name 'findex.exe' -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        $resolved = (Resolve-Path -LiteralPath $command.Source).ProviderPath
        $cwd = (Resolve-Path -LiteralPath (Get-Location).Path).ProviderPath.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        # A repository may place a forged findex.exe on PATH via "." or a
        # project-local bin directory. Never run that implicitly.
        if ($resolved.StartsWith($cwd, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'PATH resolved Findex inside the untrusted working repository; use a verified release outside this repository or explicit FINDEX_BIN.'
        }
        return $resolved
    }
    throw 'Findex runtime unavailable. Install a verified Windows Findex release, add findex.exe to PATH, or configure FINDEX_BIN with its absolute path. No download attempted.'
}

try {
    $binary = Select-FindexBinary
    # No stdout messages: MCP stdio must contain protocol frames only.
    & $binary mcp
    exit $LASTEXITCODE
} catch {
    [Console]::Error.WriteLine('findex-plugin: ' + $_.Exception.Message)
    exit 127
}
