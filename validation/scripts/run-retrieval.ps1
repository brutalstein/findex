<#
.SYNOPSIS  Golden-set retrieval benchmark: git grep vs baseline Findex vs current Findex -> retrieval.json
#>
param([string]$OutDir, [switch]$Quick, [ValidateSet('all', 'dev', 'heldout')][string]$Split = 'all')
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
Assert-ReleaseBuild
$extra = @('--out', $out, '--split', $Split); if ($Quick) { $extra += @('--corpora', 'ripgrep,requests') }
exit (Invoke-Harness 'retrieval.py' $extra)
