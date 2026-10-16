# Findex plugin MCP bootstrap. Local, fail-closed, no downloads or credential access.
# Claude Code starts this from the user's current project directory.
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
        return $command.Source
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
