# Package Unstick into dist/ (Windows).
# Usage:
#   pwsh -File scripts/Package-Portable.ps1
#   pwsh -File scripts/Package-Portable.ps1 -Sign
#   pwsh -File scripts/Package-Portable.ps1 -SkipBuild -Sign
#
# Signing (-Sign): requires signtool + UNSTICK_SIGN_THUMBPRINT (or /a auto cert).
# If -Sign is set and signing fails while REQUIRE_SIGN=1, exit non-zero.

param(
    [switch]$SkipBuild,
    [switch]$Sign
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

function Invoke-SignBinaries {
    $signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if (-not $signtool) {
        $kit = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -First 1
        if ($kit) { $signtool = $kit.FullName } else { $signtool = $null }
    } else {
        $signtool = $signtool.Source
    }
    if (-not $signtool) {
        throw "signtool.exe not found (install Windows SDK) - required for -Sign"
    }
    $thumb = $env:UNSTICK_SIGN_THUMBPRINT
    foreach ($b in $Bins) {
        $path = Join-Path $Dist $b
        Write-Host "Signing $b ..."
        if ($thumb) {
            & $signtool sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com /sha1 $thumb $path
        } else {
            & $signtool sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com /a $path
        }
        if ($LASTEXITCODE -ne 0) { throw "signtool failed for $b (exit $LASTEXITCODE)" }
    }
    Write-Host "All binaries signed."
}

$signed = $false
if ($Sign) {
    try {
        Invoke-SignBinaries
        $signed = $true
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
        if ($env:REQUIRE_SIGN -eq "1") { exit 1 }
        Write-Host "Continuing with UNSIGNED package (REQUIRE_SIGN not set)." -ForegroundColor Yellow
    }
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
    "docs\RELEASE-v0.7.0.md",
    "docs\RELEASE-v0.6.0.md",
    "docs\RELEASE-v0.5.0.md",
    "docs\RELEASE-v0.4.0.md",
    "docs\RELEASE-v0.3.0.md",
    "docs\RELEASE-v0.1.2.md",
    "docs\RELEASE-v0.1.1.md",
    "docs\RELEASE-v0.1.0.md"
) | ForEach-Object { Join-Path $Root $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($releaseNotes) {
    Copy-Item -Force $releaseNotes (Join-Path $Dist "RELEASE-NOTES.md")
}

# Honesty banner for unsigned builds
$banner = Join-Path $Dist "SIGNING.txt"
if ($signed) {
    "Authenticode: signed (SHA256). Public channel OK." | Set-Content -Path $banner -Encoding utf8
} else {
    @"
Authenticode: UNSIGNED portable build.
Private beta / local use only. SmartScreen may warn.
Public Latest releases require: pwsh -File scripts/Package-Portable.ps1 -Sign
(with UNSTICK_SIGN_THUMBPRINT or an available code-signing cert).
"@ | Set-Content -Path $banner -Encoding utf8
}

# Versioned zip next to dist/ (workspace.package.version)
$Ver = "0.7.0"
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
Write-Host ("Signed: {0}" -f $signed)
Get-ChildItem $Dist | Format-Table Name, Length -AutoSize
Get-Item $ZipPath | Format-Table Name, Length -AutoSize
