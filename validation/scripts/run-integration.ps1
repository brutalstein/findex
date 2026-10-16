<#
.SYNOPSIS  Claude Code <-> Findex integration (MCP config, --plugin-dir, launcher) -> compatibility.json
.DESCRIPTION  Offline checks (CLI, plugin packaging, doctor) always run. Real Claude sessions run
              only with -EnableLiveModelTests and the harness live-model gate (operator approval,
              subscription sign-in, not CI); otherwise they are recorded NOT_RUN or BLOCKED.
              Credentials are never read.
#>
param([string]$OutDir, [switch]$Offline, [switch]$EnableLiveModelTests, [string]$Model = 'haiku',
      [double]$SessionBudgetUsd = 0.30)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
Assert-ReleaseBuild
$extra = @('--out', $out, '--model', $Model, '--budget-usd', $SessionBudgetUsd)
if ($EnableLiveModelTests -and -not $Offline) { $extra += '--enable-live-model-tests' }
exit (Invoke-Harness 'claude_integration.py' $extra)
