# Package Unstick into dist/ (Windows).
# Usage: pwsh -File scripts/Package-Portable.ps1

param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$env:TMPDIR = Join-Path $Root ".tmp"
$env:TEMP = $env:TMPDIR
$env:TMP = $env:TMPDIR
New-Item -ItemType Directory -Force -Path $env:TMPDIR | Out-Null

if (-not $SkipBuild) {
    Write-Host "Building release binaries..."
    cargo build --release -p guardian-service -p guardian-tray -p guardian-ui
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$Dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

$Bins = @(
    "guardian-service.exe",
    "guardian-ui.exe",
    "guardian-tray.exe"
)
foreach ($b in $Bins) {
    $src = Join-Path $Root "target\release\$b"
    if (-not (Test-Path $src)) {
        throw "Missing $src - build failed?"
    }
    Copy-Item -Force $src (Join-Path $Dist $b)
}

$Docs = @(
    @{ Src = "docs\USER-GUIDE.md"; Dst = "USER-GUIDE.md" },
    @{ Src = "docs\packaging-and-soak.md"; Dst = "packaging-and-soak.md" },
    @{ Src = "README.md"; Dst = "README.txt" }
)
foreach ($d in $Docs) {
    $src = Join-Path $Root $d.Src
    if (Test-Path $src) {
        Copy-Item -Force $src (Join-Path $Dist $d.Dst)
    }
}

Copy-Item -Force (Join-Path $Root "scripts\Install-Autostart.ps1") $Dist -ErrorAction SilentlyContinue
Copy-Item -Force (Join-Path $Root "scripts\Uninstall-Autostart.ps1") $Dist -ErrorAction SilentlyContinue
$releaseNotes = @(
    "docs\RELEASE-v0.1.2.md",
    "docs\RELEASE-v0.1.1.md",
    "docs\RELEASE-v0.1.0.md"
) | ForEach-Object { Join-Path $Root $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($releaseNotes) {
    Copy-Item -Force $releaseNotes (Join-Path $Dist "RELEASE-NOTES.md")
}

# Versioned zip next to dist/ (workspace.package.version)
$Ver = "0.1.2"
$cargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $Ver = $Matches[1]
}
$ZipName = "Unstick-$Ver-windows-x64.zip"
$ZipPath = Join-Path $Root $ZipName
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
Compress-Archive -Path (Join-Path $Dist "*") -DestinationPath $ZipPath -CompressionLevel Optimal

Write-Host "Portable package ready: $Dist"
Write-Host "Zip: $ZipPath"
Get-ChildItem $Dist | Format-Table Name, Length -AutoSize
Get-Item $ZipPath | Format-Table Name, Length -AutoSize
