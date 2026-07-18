# Verify Disk Lock L3 — runtime soak probe (P2-2 automated slice)
#
# 1. Disable adaptive disk; set soft busy% very low so Soft latches under disk-hog load
# 2. SoftOnly (no Suspend) + pause_until=null
# 3. Assert status.disk_lock soft/hard, disk_lock:* throttles, no Explorer/Cursor suspend
#
# Usage: powershell -File scripts/Verify-DiskLock-L3.ps1

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $Root) { $Root = (Get-Location).Path }
Set-Location $Root

$Unstick = Join-Path $env:LOCALAPPDATA "Unstick"
$ConfigPath = Join-Path $Unstick "config.json"
$StatusPath = Join-Path $Unstick "status.json"
$EvidencePath = Join-Path $Root "specs/backend/disk-lock-l3-evidence.md"
$BackupPath = Join-Path $Unstick "config.json.disklock-l3.bak"

New-Item -ItemType Directory -Force -Path $Unstick | Out-Null

function Stop-Procs {
  Get-Process -Name "guardian-service","disk-hog" -ErrorAction SilentlyContinue |
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

$ServiceExe = Join-Path $Root "target/release/guardian-service.exe"
$HogExe = Join-Path $Root "fixtures/disk_hog/target/release/disk-hog.exe"
if (-not (Test-Path $HogExe)) {
  $alt = Get-ChildItem -Recurse -Filter "disk-hog.exe" -Path (Join-Path $Root "fixtures/disk_hog") |
    Select-Object -First 1
  if ($alt) { $HogExe = $alt.FullName }
}
if (-not (Test-Path $HogExe)) { Write-Error "disk-hog.exe not found" }

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
$cfg | Add-Member disk_busy_hard_pct 99.0 -Force
$cfg | Add-Member disk_busy_streak 2 -Force
$cfg | Add-Member critical_guard_mode "soft_only" -Force
$cfg | Add-Member emergency_suspend $true -Force
$cfg | Add-Member pause_until $null -Force
$cfg | Add-Member mem_lock_enabled $false -Force
$json = $cfg | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($ConfigPath, $json, [System.Text.UTF8Encoding]::new($false))

$verify = Get-Content $ConfigPath -Raw | ConvertFrom-Json
if ([double]$verify.disk_busy_soft_pct -gt 20) {
  Write-Error "soft pct patch failed"
  exit 1
}
Write-Host ("patched disk soft={0}% adaptive=false SoftOnly" -f $verify.disk_busy_soft_pct)

Write-Host "== start service =="
Start-Process -FilePath $ServiceExe -WindowStyle Hidden | Out-Null
Start-Sleep -Seconds 3

Write-Host "== start disk-hog 192 MiB x 50s =="
$hog = Start-Process -FilePath $HogExe -ArgumentList @("192","50") -PassThru -WindowStyle Hidden

$deadline = (Get-Date).AddSeconds(55)
$passLock = $false
$passThrottle = $false
$passProtected = $true
$passBusy = $false
$last = $null
while ((Get-Date) -lt $deadline) {
  if (Test-Path $StatusPath) {
    $last = Get-Content $StatusPath -Raw | ConvertFrom-Json
    if ($last.disk_busy_percent -ge 5.0) { $passBusy = $true }
    if ($last.disk_lock -eq "soft" -or $last.disk_lock -eq "hard") { $passLock = $true }
    foreach ($t in @($last.recent_throttles)) {
      if ([string]$t.reason -like "disk_lock:*") { $passThrottle = $true }
      $n = [string]$t.name
      if ($n -match '(?i)^(explorer|csrss|dwm|Cursor)\.exe$' -and $t.level -eq "suspend") {
        $passProtected = $false
      }
    }
    foreach ($s in @($last.suspended)) {
      $n = [string]$s.name
      if ($n -match '(?i)^(explorer|csrss|dwm|Cursor)\.exe$') { $passProtected = $false }
    }
    if ($passLock -and $passThrottle -and $passBusy) { break }
  }
  Start-Sleep -Milliseconds 800
}

$lines = @()
$lines += "# Disk Lock L3 evidence (P2-2 probe)"
$lines += ""
$lines += "Generated: $(Get-Date -Format o)"
$lines += ""
$lines += "| Check | Result |"
$lines += "|-------|--------|"
$lines += ("| disk_busy observed >= 5% | {0} (busy={1}) |" -f $(if ($passBusy) {"PASS"} else {"FAIL"}), $(if ($last) {"{0:N1}" -f $last.disk_busy_percent} else {"n/a"}))
$lines += ("| status.disk_lock soft/hard | {0} (value={1}) |" -f $(if ($passLock) {"PASS"} else {"FAIL"}), $(if ($last) {$last.disk_lock} else {"n/a"}))
$lines += ("| recent_throttles disk_lock:* | {0} |" -f $(if ($passThrottle) {"PASS"} else {"FAIL"}))
$lines += ("| no Explorer/Cursor suspend | {0} |" -f $(if ($passProtected) {"PASS"} else {"FAIL"}))
$lines += ""
$lines += "Probe: disk_busy_soft_pct=5, adaptive=false, SoftOnly, pause_until=null."
$lines += ""
if ($last) {
  $lines += '```json'
  $snap = [ordered]@{
    disk_lock = $last.disk_lock
    disk_lock_soft_pct = $last.disk_lock_soft_pct
    disk_busy_percent = $last.disk_busy_percent
    disk_queue_length = $last.disk_queue_length
    disk_latency_sec = $last.disk_latency_sec
    disk_calibrated = $last.disk_calibrated
    pressure_band = $last.pressure_band
    recent_throttles = @($last.recent_throttles | Select-Object -First 8)
    suspended = @($last.suspended | Select-Object -First 8)
  }
  $lines += ($snap | ConvertTo-Json -Depth 6)
  $lines += '```'
}

New-Item -ItemType Directory -Force -Path (Split-Path $EvidencePath) | Out-Null
($lines -join [Environment]::NewLine) | Set-Content -Path $EvidencePath -Encoding utf8
Write-Host ""
$lines | ForEach-Object { Write-Host $_ }

Stop-Procs
Restore-Config

$all = $passBusy -and $passLock -and $passThrottle -and $passProtected
if (-not $all) {
  Write-Error "Disk Lock L3 FAILED - see $EvidencePath"
  exit 1
}
Write-Host "Disk Lock L3 PASS - evidence: $EvidencePath"
exit 0
