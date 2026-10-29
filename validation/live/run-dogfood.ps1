# findex Live Dogfooding Test Script

# Bu script, findex'i kendi kodbayı üzerinde test eder (dogfooding).
# Çıktı kanıt dosyası olarak kaydedilir.

# Kullanım: powershell -ExecutionPolicy Bypass -File validation/live/run-dogfood.ps1

param([string]$OutDir = "validation/live/results")

$ErrorActionPreference = "Stop"
$date = Get-Date -Format "yyyyMMddTHHmmssZ"
$runDir = Join-Path $OutDir "$date-dogfood"
New-Item -ItemType Directory -Path $runDir -Force | Out-Null

Write-Host "=== findex Live Dogfooding Test ===" -ForegroundColor Cyan
Write-Host "Run directory: $runDir"

# 1. Build release
Write-Host "`n[1/6] Building release binary..." -ForegroundColor Yellow
cmd /c "cargo build --release -p findex 2>&1" | Tee-Object -FilePath (Join-Path $runDir "build.log")
if ($LASTEXITCODE -ne 0) { Write-Host "BUILD FAILED" -ForegroundColor Red; exit 1 }
Write-Host "Build OK" -ForegroundColor Green

$exe = "target/release/findex.exe"
if (-not (Test-Path $exe)) { Write-Host "Binary not found: $exe" -ForegroundColor Red; exit 1 }

# 2. Index self
Write-Host "`n[2/6] Indexing self (findex project)..." -ForegroundColor Yellow
& $exe index | Tee-Object -FilePath (Join-Path $runDir "index.log")
if ($LASTEXITCODE -ne 0) { Write-Host "INDEX FAILED" -ForegroundColor Red; exit 1 }
Write-Host "Index OK" -ForegroundColor Green

# 3. Rank test
Write-Host "`n[3/6] Running rank: 'poll_proceed'..." -ForegroundColor Yellow
& $exe rank poll_proceed | Tee-Object -FilePath (Join-Path $runDir "rank-poll.log")
Write-Host "Rank OK" -ForegroundColor Green

# 4. Rank test 2
Write-Host "`n[4/6] Running rank: 'findex_rank'..." -ForegroundColor Yellow
& $exe rank findex_rank | Tee-Object -FilePath (Join-Path $runDir "rank-findex-rank.log")
Write-Host "Rank OK" -ForegroundColor Green

# 5. Search test
Write-Host "`n[5/6] Running search: Tree-sitter..." -ForegroundColor Yellow
& $exe search "Tree-sitter" | Tee-Object -FilePath (Join-Path $runDir "search-tree-sitter.log")
Write-Host "Search OK" -ForegroundColor Green

# 6. Doctor
Write-Host "`n[6/6] Running doctor..." -ForegroundColor Yellow
& $exe doctor | Tee-Object -FilePath (Join-Path $runDir "doctor.log")
Write-Host "Doctor OK" -ForegroundColor Green

# Summary
Write-Host "`n=== All live dogfood tests completed ===" -ForegroundColor Cyan
Write-Host "Results in: $runDir"
Get-ChildItem $runDir | Format-Table Name, Length
