# Local mirror of P2-1 CI gates (Windows).
# Usage: powershell -File scripts/Verify-P2-Automated.ps1

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$env:TMPDIR = Join-Path $Root ".tmp"
$env:TEMP = $env:TMPDIR
$env:TMP = $env:TMPDIR
New-Item -ItemType Directory -Force -Path $env:TMPDIR | Out-Null

Write-Host "==> cargo test -p guardian-core -p guardian-detect"
cargo test -p guardian-core -p guardian-detect
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "==> cargo build --release (service, ui, tray)"
cargo build --release -p guardian-service -p guardian-ui -p guardian-tray -p unstick-updater
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "==> fake-miner fixture"
cargo build --release --manifest-path fixtures/fake_miner/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "==> Package-Portable.ps1 -SkipBuild"
& "$Root\scripts\Package-Portable.ps1" -SkipBuild
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "P2-1 automated gates PASSED."
Write-Host "Complete manual soaks in docs/p2-proof-checklist.md (P2-2 .. P2-4)."
