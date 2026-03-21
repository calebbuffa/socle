# Compute SHA512 hashes for orkester release tarballs.
# Usage: .\scripts\vcpkg-hashes.ps1 0.1.1

param(
    [Parameter(Mandatory)][string]$Version
)

$Repo = "calebbuffa/socle"
$Base = "https://github.com/$Repo/releases/download/orkester/v$Version"
$Targets = @(
    "x86_64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

$tmp = Join-Path $env:TEMP "orkester-hashes-$Version"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

foreach ($target in $Targets) {
    $file = "orkester-$target.tar.gz"
    $outPath = Join-Path $tmp $file
    Invoke-WebRequest -Uri "$Base/$file" -OutFile $outPath -UseBasicParsing
    $hash = (Get-FileHash -Algorithm SHA512 $outPath).Hash.ToLower()
    Write-Host ("{0,-35} {1}" -f $target, $hash)
}

Remove-Item -Recurse -Force $tmp
