<#
.SYNOPSIS  environment.json, manifest.json, regressions.json, failures.json, summary.md, checksums.sha256
#>
param([Parameter(Mandatory = $true)][string]$OutDir)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
exit (Invoke-Harness 'report.py' @('--out', $out))
