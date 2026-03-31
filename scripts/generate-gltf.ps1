param(
    [string]$SchemaGenDir = "",
    [string]$GltfSchemaDir = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

# Get script directory and socle root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SocleRoot = Split-Path -Parent $ScriptDir

# Set defaults relative to socle root if not provided
if (-not $SchemaGenDir) { $SchemaGenDir = Join-Path $SocleRoot "crates/schema-gen" }
if (-not $GltfSchemaDir) { $GltfSchemaDir = Join-Path $SocleRoot "extern/glTF/specification/2.0/schema" }
if (-not $OutputDir) { $OutputDir = Join-Path $SocleRoot "crates/moderu/src" }

# Resolve to absolute paths
$SchemaGenDir = (Resolve-Path $SchemaGenDir).Path
$GltfSchemaDir = (Resolve-Path $GltfSchemaDir).Path
$OutputDir = (Resolve-Path $OutputDir).Path

Write-Verbose "Schema-gen directory: $SchemaGenDir"
Write-Verbose "glTF schema directory: $GltfSchemaDir"
Write-Verbose "Output directory: $OutputDir"

# Verify required files exist
$SchemaFile = Join-Path $GltfSchemaDir "glTF.schema.json"
$ConfigFile = Join-Path $SchemaGenDir "configs/gltf.json"

if (-not (Test-Path $SchemaFile)) {
    Write-Error "Schema file not found: $SchemaFile"
}

if (-not (Test-Path $ConfigFile)) {
    Write-Error "Config file not found: $ConfigFile"
}

Write-Host "Regenerating glTF types..." -ForegroundColor Cyan
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
        "--schema-dir", $GltfSchemaDir
    )

    Write-Verbose "Running: cargo run -- $($arguments -join ' ')"

    & cargo run -- @arguments

    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "glTF types regenerated successfully!" -ForegroundColor Green
        Write-Host "  Output: $(Join-Path $OutputDir 'generated.rs')"
    }
    else {
        Write-Error "Schema generation failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
