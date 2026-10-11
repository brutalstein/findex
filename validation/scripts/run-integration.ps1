<#
.SYNOPSIS  Real Claude Code <-> Findex integration (MCP config, --plugin-dir, launcher) -> compatibility.json
.DESCRIPTION  Uses the already-authenticated official `claude` CLI. Records BLOCKED when it is
              missing or not logged in, or when -Offline is given. Credentials are never read.
#>
param([string]$OutDir, [switch]$Offline, [string]$Model = 'haiku', [double]$SessionBudgetUsd = 0.30)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
if ($Offline) {
    '{"status":"BLOCKED","reason":"-Offline: real Claude sessions need network"}' | Set-Content -Encoding ascii (Join-Path $out 'compatibility.json')
    exit 0
}
Assert-ReleaseBuild
exit (Invoke-Harness 'claude_integration.py' @('--out', $out, '--model', $Model, '--budget-usd', $SessionBudgetUsd))
