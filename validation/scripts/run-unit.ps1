<#
.SYNOPSIS  fmt, clippy, full Rust test suite, release build, cargo audit -> tests.json
#>
param([string]$OutDir, [switch]$SkipClippy)
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
$extra = @('--out', $out); if ($SkipClippy) { $extra += '--skip-clippy' }
exit (Invoke-Harness 'unit.py' $extra)
