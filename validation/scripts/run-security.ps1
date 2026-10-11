<#
.SYNOPSIS  Black-box security/reliability checks of the release binary -> security.json
#>
param([string]$OutDir, [switch]$Quick)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
Assert-ReleaseBuild
$extra = @('--out', $out); if ($Quick) { $extra += '--quick' }
exit (Invoke-Harness 'security.py' $extra)
