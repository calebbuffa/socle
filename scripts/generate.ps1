# Generate Rust types from i3s-spec
# Usage: .\scripts\generate.ps1

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir

Write-Host "Phase 1: Parsing i3s-spec markdown -> JSON IR" -ForegroundColor Cyan
python "$scriptDir\parse_spec.py"
if ($LASTEXITCODE -ne 0) { throw "parse_spec.py failed" }

Write-Host ""
Write-Host "Phase 2: Generating Rust code from JSON IR" -ForegroundColor Cyan
python "$scriptDir\generate_rust.py"
if ($LASTEXITCODE -ne 0) { throw "generate_rust.py failed" }

Write-Host ""
Write-Host "Phase 3: Verifying build" -ForegroundColor Cyan
Push-Location $projectRoot
try {
    cargo build -p i3s
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    Write-Host "Build succeeded!" -ForegroundColor Green
} finally {
    Pop-Location
}
