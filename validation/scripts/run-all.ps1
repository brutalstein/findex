<#
.SYNOPSIS  Full validation run into validation/results/<UTC>-<sha>/.
.EXAMPLE   .\validation\scripts\run-all.ps1 -Quick -Offline
.EXAMPLE   .\validation\scripts\run-all.ps1 -Extended -TotalBudgetUsd 6
#>
param([string]$OutDir, [switch]$Quick, [switch]$Extended, [switch]$Offline,
      [ValidateSet('Normal', 'BelowNormal', 'Idle')][string]$Priority = 'Normal',
      [double]$SessionBudgetUsd = 0.40, [double]$TotalBudgetUsd = 6.0)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
$failed = @()
$python = Get-Python
& $python[0] @($python | Select-Object -Skip 1) (Join-Path $Harness 'corpora.py') $(if ($Offline) { '--verify-only' })
if ($LASTEXITCODE -ne 0) { throw 'corpora/baseline not ready (run without -Offline once to fetch)' }
$steps = @(
    @{ Name = 'unit'; Args = @{} },
    @{ Name = 'retrieval'; Args = @{ Quick = $Quick } },
    @{ Name = 'performance'; Args = @{ Quick = $Quick; Extended = $Extended; Priority = $Priority } },
    @{ Name = 'relations'; Args = @{} },
    @{ Name = 'security'; Args = @{ Quick = $Quick } },
    @{ Name = 'integration'; Args = @{ Offline = $Offline } },
    @{ Name = 'claude-ab'; Args = @{ Offline = $Offline; Quick = $Quick; SessionBudgetUsd = $SessionBudgetUsd; TotalBudgetUsd = $TotalBudgetUsd } }
)
foreach ($step in $steps) {
    $params = $step.Args; $params.OutDir = $out
    & (Join-Path $PSScriptRoot "run-$($step.Name).ps1") @params
    if ($LASTEXITCODE -ne 0) { $failed += $step.Name }
}
& (Join-Path $PSScriptRoot 'generate-report.ps1') -OutDir $out
Write-Host "Results: $out"
if ($failed) { Write-Warning "Steps with failures: $($failed -join ', ')"; exit 1 }
