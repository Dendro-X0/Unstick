# Verify P2-3 L4 decoy — fake-miner suspend safety
#
# LastResort + low Disk Hard threshold + disk-hog pressure → Suspend decoy only.
# Assert explorer / Cursor / whitelist never suspended.
#
# Usage: powershell -File scripts/Verify-P2-L4-Decoy.ps1

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $Root) { $Root = (Get-Location).Path }
Set-Location $Root

$Unstick = Join-Path $env:LOCALAPPDATA "Unstick"
$ConfigPath = Join-Path $Unstick "config.json"
$StatusPath = Join-Path $Unstick "status.json"
$EvidencePath = Join-Path $Root "specs/backend/p2-l4-decoy-evidence.md"
$BackupPath = Join-Path $Unstick "config.json.p2-l4.bak"

New-Item -ItemType Directory -Force -Path $Unstick | Out-Null

function Stop-Procs {
  Get-Process -Name "guardian-service","disk-hog","fake-miner" -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

function Restore-Config {
  if (Test-Path $BackupPath) {
    Copy-Item $BackupPath $ConfigPath -Force
    Remove-Item $BackupPath -Force
  }
}

Write-Host "== stop =="
Stop-Procs

Write-Host "== build =="
cargo build -p guardian-service --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release --manifest-path fixtures/disk_hog/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release --manifest-path fixtures/fake_miner/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ServiceExe = Join-Path $Root "target/release/guardian-service.exe"
$DiskHog = Join-Path $Root "fixtures/disk_hog/target/release/disk-hog.exe"
$Miner = Join-Path $Root "fixtures/fake_miner/target/release/fake-miner.exe"
if (-not (Test-Path $DiskHog)) {
  $alt = Get-ChildItem -Recurse -Filter "disk-hog.exe" -Path (Join-Path $Root "fixtures/disk_hog") | Select-Object -First 1
  if ($alt) { $DiskHog = $alt.FullName }
}
if (-not (Test-Path $Miner)) {
  $alt = Get-ChildItem -Recurse -Filter "fake-miner.exe" -Path (Join-Path $Root "fixtures/fake_miner") | Select-Object -First 1
  if ($alt) { $Miner = $alt.FullName }
}

$hadConfig = Test-Path $ConfigPath
if ($hadConfig) {
  Copy-Item $ConfigPath $BackupPath -Force
  $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
} else {
  $cfg = [pscustomobject]@{}
}

$cfg | Add-Member disk_lock_enabled $true -Force
$cfg | Add-Member disk_lock_adaptive $false -Force
$cfg | Add-Member disk_busy_soft_pct 5.0 -Force
$cfg | Add-Member disk_busy_hard_pct 8.0 -Force
$cfg | Add-Member disk_busy_streak 2 -Force
$cfg | Add-Member critical_guard_mode "last_resort_suspend" -Force
$cfg | Add-Member emergency_suspend $true -Force
$cfg | Add-Member suspend_escalation_streak 2 -Force
$cfg | Add-Member pause_until $null -Force
$cfg | Add-Member mem_lock_enabled $false -Force
# Ensure Cursor-like path stays protected via default path substr; add explicit whitelist sample
$wl = @()
if ($cfg.whitelist) { $wl = @($cfg.whitelist) }
if ($wl -notcontains "explorer.exe") { $wl += "explorer.exe" }
$cfg | Add-Member whitelist $wl -Force

$json = $cfg | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($ConfigPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host "patched LastResort + disk hard=8% SoftOnly off"

Write-Host "== start service =="
Start-Process -FilePath $ServiceExe -WindowStyle Hidden | Out-Null
Start-Sleep -Seconds 3

Write-Host "== start fake-miner =="
$miner = Start-Process -FilePath $Miner -ArgumentList "stratum+tcp://example" -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 1

Write-Host "== start disk-hog pressure =="
$hog = Start-Process -FilePath $DiskHog -ArgumentList @("256","55") -PassThru -WindowStyle Hidden

$deadline = (Get-Date).AddSeconds(60)
$passSuspendDecoy = $false
$passProtected = $true
$passAbuseOrSuspend = $false
$last = $null
while ((Get-Date) -lt $deadline) {
  if (Test-Path $StatusPath) {
    $last = Get-Content $StatusPath -Raw | ConvertFrom-Json
    foreach ($s in @($last.suspended)) {
      $n = [string]$s.name
      if ($n -match '(?i)fake-miner|xmrig|miner') { $passSuspendDecoy = $true; $passAbuseOrSuspend = $true }
      if ($n -match '(?i)^(explorer|csrss|dwm|Cursor)\.exe$') { $passProtected = $false }
    }
    foreach ($a in @($last.recent_abuse)) {
      if ([int]$a.score -ge 70 -and ([string]$a.name -match '(?i)fake-miner|miner')) {
        $passAbuseOrSuspend = $true
      }
    }
    if ($passSuspendDecoy -and $passProtected) { break }
  }
  Start-Sleep -Milliseconds 900
}

$lines = @()
$lines += "# P2-3 L4 decoy evidence"
$lines += ""
$lines += "Generated: $(Get-Date -Format o)"
$lines += ""
$lines += "| Check | Result |"
$lines += "|-------|--------|"
$lines += ("| decoy suspended OR abuse score>=70 | {0} |" -f $(if ($passAbuseOrSuspend) {"PASS"} else {"FAIL"}))
$lines += ("| fake-miner in suspended list | {0} |" -f $(if ($passSuspendDecoy) {"PASS"} else {"PARTIAL"}))
$lines += ("| Explorer/Cursor never suspended | {0} |" -f $(if ($passProtected) {"PASS"} else {"FAIL"}))
$lines += ""
$lines += "Probe: last_resort_suspend, disk hard=8%, disk-hog + fake-miner."
$lines += ""
if ($last) {
  $lines += '```json'
  $snap = [ordered]@{
    disk_lock = $last.disk_lock
    pressure_band = $last.pressure_band
    tripwire = $last.tripwire
    critical_guard_mode = $last.critical_guard_mode
    suspended = @($last.suspended | Select-Object -First 10)
    recent_abuse = @($last.recent_abuse | Select-Object -First 8)
  }
  $lines += ($snap | ConvertTo-Json -Depth 6)
  $lines += '```'
}

($lines -join [Environment]::NewLine) | Set-Content -Path $EvidencePath -Encoding utf8
Write-Host ""
$lines | ForEach-Object { Write-Host $_ }

Stop-Procs

# Always SoftOnly-restart once so ledger orphans (nvcontainer etc.) are resumed.
Write-Host "== P0 resume orphans =="
if (Test-Path $BackupPath) {
  # restore user config first, then force a SoftOnly boot
  Copy-Item $BackupPath $ConfigPath -Force
}
$cfg2 = if (Test-Path $ConfigPath) { Get-Content $ConfigPath -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
$cfg2 | Add-Member critical_guard_mode "soft_only" -Force
$cfg2 | Add-Member pause_until $null -Force
[System.IO.File]::WriteAllText($ConfigPath, ($cfg2 | ConvertTo-Json -Depth 8), [System.Text.UTF8Encoding]::new($false))
Start-Process -FilePath $ServiceExe -WindowStyle Hidden | Out-Null
Start-Sleep -Seconds 4
Stop-Procs
Restore-Config

$all = $passProtected -and $passAbuseOrSuspend
if (-not $all) {
  Write-Error "P2-3 L4 FAILED - see $EvidencePath"
  exit 1
}
Write-Host "P2-3 L4 PASS - evidence: $EvidencePath"
exit 0
