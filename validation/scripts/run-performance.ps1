<#
.SYNOPSIS  Cold/no-op indexing, MCP latency, idle CPU, watcher workload, concurrency -> performance/memory/cpu.json
.PARAMETER Priority  Process priority class for this PowerShell host and its children (resource limit).
#>
param([string]$OutDir, [switch]$Quick, [switch]$Extended, [int]$Reps = 3,
      [string]$Corpora = 'ripgrep,tokio,vscode', [ValidateSet('Normal', 'BelowNormal', 'Idle')][string]$Priority = 'Normal')
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
Assert-ReleaseBuild
(Get-Process -Id $PID).PriorityClass = $Priority
if ($Quick) { $Corpora = 'ripgrep'; $Reps = 1 }
if ($Extended) { $Reps = [Math]::Max($Reps, 5) }
$extra = @('--out', $out, '--corpora', $Corpora, '--reps', $Reps); if ($Quick) { $extra += '--quick' }
exit (Invoke-Harness 'perf.py' $extra)
