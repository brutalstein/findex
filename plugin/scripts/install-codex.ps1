# Explicit local installation. No build, downloads, model calls or credential access.
param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [string]$Codex = 'codex'
)
$ErrorActionPreference = 'Stop'
if (-not [IO.Path]::IsPathRooted($Binary) -or
    -not (Test-Path -LiteralPath $Binary -PathType Leaf) -or
    [IO.Path]::GetFileName($Binary) -ine 'findex.exe') {
    throw 'Binary must be an existing absolute findex.exe path.'
}
if (-not [IO.Path]::IsPathRooted($env:LOCALAPPDATA)) {
    throw 'LOCALAPPDATA must be an absolute directory.'
}
$runtime = (Resolve-Path -LiteralPath $Binary).ProviderPath
$source = Split-Path -Parent $PSScriptRoot
$marketplace = Join-Path $env:LOCALAPPDATA 'Findex\codex-plugin-marketplace'
$installed = Join-Path $marketplace 'plugin'
New-Item -ItemType Directory -Force -Path $installed | Out-Null
$digest = (Get-FileHash -LiteralPath $runtime -Algorithm SHA256).Hash.ToLowerInvariant()
# Windows locks running executables. Keep each verified runtime immutable so an
# update never overwrites the binary used by an existing MCP session.
$runtimeDir = Join-Path $env:LOCALAPPDATA "Findex\codex-runtimes\$digest"
New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null
$boundRuntime = Join-Path $runtimeDir 'findex.exe'
if (-not (Test-Path -LiteralPath $boundRuntime)) {
    Copy-Item -LiteralPath $runtime -Destination $boundRuntime
}
if ((Get-FileHash -LiteralPath $boundRuntime -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) {
    throw 'Installed runtime hash mismatch; existing MCP configuration was preserved.'
}
Get-ChildItem -LiteralPath $source -Force |
    Where-Object { $_.Name -notin @('bin', '.mcp.codex.json') } |
    Copy-Item -Destination $installed -Recurse -Force
# Bind the trusted executable explicitly: Codex 0.164 alpha on Windows did not
# expand plugin-root placeholders in bundled MCP commands in the native test.
$config = @{mcpServers = @{findex = @{
    type = 'stdio'; command = $boundRuntime; args = @('mcp')
}}}
$config | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $installed '.mcp.codex.json') -Encoding UTF8
$catalogDir = Join-Path $marketplace '.agents\plugins'
New-Item -ItemType Directory -Force -Path $catalogDir | Out-Null
@{name = 'findex-local'; plugins = @(@{
    name = 'findex'; source = @{source = 'local'; path = './plugin'}
    policy = @{installation = 'AVAILABLE'; authentication = 'ON_INSTALL'}
    category = 'Developer Tools'
})} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $catalogDir 'marketplace.json') -Encoding UTF8
& $Codex plugin marketplace add $marketplace --json
if ($LASTEXITCODE -ne 0) { throw 'Codex marketplace registration failed.' }
& $Codex plugin add 'findex@findex-local' --json
if ($LASTEXITCODE -ne 0) { throw 'Codex plugin installation failed.' }
