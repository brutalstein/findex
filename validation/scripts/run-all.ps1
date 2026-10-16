<#
.SYNOPSIS  Full validation run into validation/results/<UTC>-<sha>/.
.DESCRIPTION  Offline by default: no Claude or other model session starts unless
              -EnableLiveModelTests is given (and the harness gate then also needs operator
              approval). -Offline additionally skips fetching corpora. -Steps runs a subset.
.EXAMPLE   .\validation\scripts\run-all.ps1 -Quick
.EXAMPLE   .\validation\scripts\run-all.ps1 -Steps claude-ab -EnableLiveModelTests -TotalBudgetUsd 1
#>
param([string]$OutDir, [switch]$Quick, [switch]$Extended, [switch]$Offline, [switch]$EnableLiveModelTests,
      [string[]]$Steps = @('unit', 'retrieval', 'performance', 'relations', 'security', 'integration', 'claude-ab'),
      [ValidateSet('Normal', 'BelowNormal', 'Idle')][string]$Priority = 'Normal',
      [double]$SessionBudgetUsd = 0.40, [double]$TotalBudgetUsd = 6.0)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
$failed = @()
$python = Get-Python
# -Steps may arrive as one comma-separated string through powershell -File.
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($EnableLiveModelTests -or @($Steps | Where-Object { $_ -notin 'integration', 'claude-ab' }).Count) {
    & $python[0] @($python | Select-Object -Skip 1) (Join-Path $Harness 'corpora.py') $(if ($Offline) { '--verify-only' })
    if ($LASTEXITCODE -ne 0) { throw 'corpora/baseline not ready (run without -Offline once to fetch)' }
}
& $python[0] @($python | Select-Object -Skip 1) (Join-Path $Harness 'report.py') --out $out --environment-only
$plan = @(
    @{ Name = 'unit'; Args = @{} },
    @{ Name = 'retrieval'; Args = @{ Quick = $Quick } },
    @{ Name = 'performance'; Args = @{ Quick = $Quick; Extended = $Extended; Priority = $Priority } },
    @{ Name = 'relations'; Args = @{} },
    @{ Name = 'security'; Args = @{ Quick = $Quick } },
    @{ Name = 'integration'; Args = @{ Offline = $Offline; EnableLiveModelTests = $EnableLiveModelTests } },
    @{ Name = 'claude-ab'; Args = @{ Offline = $Offline; Quick = $Quick; EnableLiveModelTests = $EnableLiveModelTests
                                     SessionBudgetUsd = $SessionBudgetUsd; TotalBudgetUsd = $TotalBudgetUsd } }
)
foreach ($step in $plan | Where-Object { $_.Name -in $Steps }) {
    $params = $step.Args; $params.OutDir = $out
    & (Join-Path $PSScriptRoot "run-$($step.Name).ps1") @params
    if ($LASTEXITCODE -ne 0) { $failed += $step.Name }
}
& (Join-Path $PSScriptRoot 'generate-report.ps1') -OutDir $out
Write-Host "Results: $out"
if ($failed) { Write-Warning "Steps with failures: $($failed -join ', ')"; exit 1 }
