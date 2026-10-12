<#
.SYNOPSIS  Controlled Claude Code study, arms A-E (see claude_ab.py ARMS) -> tokens.json, comparisons.json
.DESCRIPTION  Spends real model usage on the operator's existing authenticated account.
              -SessionBudgetUsd caps each session (--max-budget-usd); the run stops scheduling
              sessions once the estimated spend reaches -TotalBudgetUsd (remaining ones are BLOCKED).
#>
param([string]$OutDir, [switch]$Offline, [switch]$Quick, [string]$Model = 'haiku', [int]$Reps = 3,
      [double]$SessionBudgetUsd = 0.40, [double]$TotalBudgetUsd = 6.0, [string]$Tasks = '', [string]$Arms = 'A,B,C,D')
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
if ($Offline) {
    '{"status":"BLOCKED","reason":"-Offline: real Claude sessions need network"}' | Set-Content -Encoding ascii (Join-Path $out 'tokens.json')
    exit 0
}
Assert-ReleaseBuild
if ($Quick) { $Reps = 1 }
$extra = @('--out', $out, '--model', $Model, '--reps', $Reps, '--session-budget', $SessionBudgetUsd, '--total-budget', $TotalBudgetUsd)
if ($Tasks) { $extra += @('--tasks', $Tasks) }
$extra += @('--arms', $Arms)
exit (Invoke-Harness 'claude_ab.py' $extra)
