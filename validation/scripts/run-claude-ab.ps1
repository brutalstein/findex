<#
.SYNOPSIS  Controlled Claude Code study (see claude_ab.py ARMS) -> tokens.json, comparisons.json
.DESCRIPTION  Off by default: without -EnableLiveModelTests no Claude session starts and
              tokens.json records NOT_RUN. With it, the harness still requires operator approval
              (FINDEX_LIVE_MODEL_APPROVAL), a claude.ai subscription sign-in, no API-billing
              variables, not CI, and at most -MaxSessions new sessions. Spends real subscription
              usage on the operator's existing account; -SessionBudgetUsd/-TotalBudgetUsd are
              client-side estimates, not billing guarantees.
#>
param([string]$OutDir, [switch]$Offline, [switch]$Quick, [switch]$EnableLiveModelTests, [int]$MaxSessions = 12,
      [switch]$Resume, [string]$Model = 'haiku', [int]$Reps = 3,
      [double]$SessionBudgetUsd = 0.40, [double]$TotalBudgetUsd = 6.0, [string]$Tasks = '', [string]$Arms = 'A,B,C,D')
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
$live = $EnableLiveModelTests -and -not $Offline
if ($live) { Assert-ReleaseBuild }
if ($Quick) { $Reps = 1 }
$extra = @('--out', $out, '--model', $Model, '--reps', $Reps, '--session-budget', $SessionBudgetUsd,
           '--total-budget', $TotalBudgetUsd, '--max-sessions', $MaxSessions, '--arms', $Arms)
if ($Tasks) { $extra += @('--tasks', $Tasks) }
if ($Resume) { $extra += '--resume' }
if ($live) { $extra += '--enable-live-model-tests' }
exit (Invoke-Harness 'claude_ab.py' $extra)
