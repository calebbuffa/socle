param(
    [string]$SchemaGenDir = "",
    [string]$TilesSchemaDir = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

# Get script directory and socle root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SocleRoot = Split-Path -Parent $ScriptDir

# Set defaults relative to socle root if not provided
if (-not $SchemaGenDir) { $SchemaGenDir = Join-Path $SocleRoot "crates/schema-gen" }
if (-not $TilesSchemaDir) { $TilesSchemaDir = Join-Path $SocleRoot "extern/3d-tiles/specification/schema" }
if (-not $OutputDir) { $OutputDir = Join-Path $SocleRoot "crates/tiles3d/src" }

# Resolve to absolute paths
$SchemaGenDir = (Resolve-Path $SchemaGenDir).Path
$TilesSchemaDir = (Resolve-Path $TilesSchemaDir).Path
$OutputDir = (Resolve-Path $OutputDir).Path

# Verify required files exist
$SchemaFile = Join-Path $TilesSchemaDir "tileset.schema.json"
$ConfigFile = Join-Path $SchemaGenDir "configs/3dtiles.json"

if (-not (Test-Path $SchemaFile)) {
    Write-Error "Schema file not found: $SchemaFile"
}

if (-not (Test-Path $ConfigFile)) {
    Write-Error "Config file not found: $ConfigFile"
}

Write-Host "Regenerating 3D Tiles types..." -ForegroundColor Cyan
Write-Host "  Schema: $SchemaFile"
Write-Host "  Config: $ConfigFile"
Write-Host "  Output: $OutputDir"
Write-Host ""

# Run schema-gen
Push-Location $SchemaGenDir
try {
    $arguments = @(
        "--schema", $SchemaFile,
        "--config", $ConfigFile,
        "--output", $OutputDir,
        "--schema-dir", $TilesSchemaDir,
        "--module-doc", "Generated 3D Tiles types."
    )

    Write-Verbose "Running: cargo run -- $($arguments -join ' ')"

    & cargo run -- @arguments

    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "3D Tiles types regenerated successfully!" -ForegroundColor Green
        Write-Host "  Output: $(Join-Path $OutputDir 'generated.rs')"
    }
    else {
        Write-Error "Schema generation failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
