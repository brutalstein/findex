<#
.SYNOPSIS  Semantic edge precision: rust-analyzer SCIP -> findex_impact, verified on disk -> relations.json
#>
param([string]$OutDir, [string]$Corpus = 'ripgrep')
. (Join-Path $PSScriptRoot '_common.ps1')
$out = Initialize-Results $OutDir
Assert-ReleaseBuild
exit (Invoke-Harness 'relations.py' @('--out', $out, '--corpus', $Corpus))
