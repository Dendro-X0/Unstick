# Verify P2-4 false-positive soak
#
# Runs Guard (LastResort) under build + disk pressure and fails if
# Explorer / Cursor / whitelist / shell protected names are suspended
# or appear in mem/disk lock throttles.
#
# Usage:
#   powershell -File scripts/Verify-P2-4-FalsePositive.ps1
#   powershell -File scripts/Verify-P2-4-FalsePositive.ps1 -Minutes 120
#
# Default 60 minutes (coding-phase automated). Use 120 for full checklist length.

param(
    [int]$Minutes = 60
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $Root) { $Root = (Get-Location).Path }
Set-Location $Root

$Unstick = Join-Path $env:LOCALAPPDATA "Unstick"
$ConfigPath = Join-Path $Unstick "config.json"
$StatusPath = Join-Path $Unstick "status.json"
$EventsPath = Join-Path $Unstick "events.jsonl"
$EvidencePath = Join-Path $Root "specs/backend/p2-4-false-positive-evidence.md"
$BackupPath = Join-Path $Unstick "config.json.p2-4.bak"
$BadLog = Join-Path $Unstick "p2-4-bad-events.log"

New-Item -ItemType Directory -Force -Path $Unstick | Out-Null
if (Test-Path $BadLog) { Remove-Item $BadLog -Force }

$ForbiddenName = '(?i)^(explorer|csrss|dwm|winlogon|lsass|Cursor|Code|steam|steamwebhelper|GameOverlayUI|conhost|WindowsTerminal|powershell|pwsh|cmd)\.exe$'
$ForbiddenPathHint = '(?i)\\(cursor|steam|epic games|riot games)\\'

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

function Test-Forbidden($name, $path) {
  if ($name -and ($name -match $ForbiddenName)) { return $true }
  if ($path -and ($path -match $ForbiddenPathHint)) { return $true }
  return $false
}

Write-Host "== stop =="
Stop-Procs

Write-Host "== build service + disk-hog =="
cargo build -p guardian-service --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release --manifest-path fixtures/disk_hog/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ServiceExe = Join-Path $Root "target/release/guardian-service.exe"
$DiskHog = Join-Path $Root "fixtures/disk_hog/target/release/disk-hog.exe"
if (-not (Test-Path $DiskHog)) {
  $alt = Get-ChildItem -Recurse -Filter "disk-hog.exe" -Path (Join-Path $Root "fixtures/disk_hog") | Select-Object -First 1
  if ($alt) { $DiskHog = $alt.FullName }
}

$hadConfig = Test-Path $ConfigPath
if ($hadConfig) {
  Copy-Item $ConfigPath $BackupPath -Force
  $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
} else {
  $cfg = [pscustomobject]@{}
}

$wl = @()
if ($cfg.whitelist) { $wl = @($cfg.whitelist) }
foreach ($extra in @("steam.exe", "\Steam\", "\Epic Games\", "explorer.exe")) {
  if ($wl -notcontains $extra) { $wl += $extra }
}
$cfg | Add-Member whitelist $wl -Force
$cfg | Add-Member critical_guard_mode "last_resort_suspend" -Force
$cfg | Add-Member emergency_suspend $true -Force
$cfg | Add-Member suspend_escalation_streak 3 -Force
$cfg | Add-Member pause_until $null -Force
$cfg | Add-Member disk_lock_enabled $true -Force
$cfg | Add-Member disk_lock_adaptive $false -Force
$cfg | Add-Member disk_busy_soft_pct 55.0 -Force
$cfg | Add-Member disk_busy_hard_pct 85.0 -Force
$cfg | Add-Member disk_busy_streak 2 -Force
$cfg | Add-Member mem_lock_enabled $true -Force
$json = $cfg | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($ConfigPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host ("patched LastResort soak ${Minutes}m whitelist=$($wl -join ', ')")

Write-Host "== start guardian-service =="
Start-Process -FilePath $ServiceExe -WindowStyle Hidden | Out-Null
Start-Sleep -Seconds 3

$deadline = (Get-Date).AddMinutes($Minutes)
$nextBuild = Get-Date
$nextHog = (Get-Date).AddMinutes(2)
$violations = New-Object System.Collections.Generic.List[string]
$samples = 0
$sawPressure = $false
$sawAnySuspend = $false
$suspendNames = New-Object System.Collections.Generic.HashSet[string]

Write-Host ("== soak until {0:o} ==" -f $deadline)

while ((Get-Date) -lt $deadline) {
  # Coding load: rebuild core periodically
  if ((Get-Date) -ge $nextBuild) {
    Write-Host ("[{0:HH:mm:ss}] cargo test -p guardian-core --lib" -f (Get-Date))
    cargo test -p guardian-core --lib --quiet 2>&1 | Out-Null
    $nextBuild = (Get-Date).AddMinutes(8)
  }

  # Burst disk pressure (short) so LastResort can arm without 60m of thrash
  if ((Get-Date) -ge $nextHog) {
    Write-Host ("[{0:HH:mm:ss}] disk-hog pulse 96MiB/25s" -f (Get-Date))
    Start-Process -FilePath $DiskHog -ArgumentList @("96","25") -WindowStyle Hidden | Out-Null
    $nextHog = (Get-Date).AddMinutes(10)
  }

  if (Test-Path $StatusPath) {
    $samples++
    try {
      $st = Get-Content $StatusPath -Raw | ConvertFrom-Json
    } catch {
      Start-Sleep -Seconds 2
      continue
    }
    if ($st.pressure_band -ne "normal" -or $st.disk_lock -ne "off") { $sawPressure = $true }

    foreach ($s in @($st.suspended)) {
      $sawAnySuspend = $true
      [void]$suspendNames.Add([string]$s.name)
      $n = [string]$s.name
      if (Test-Forbidden $n $null) {
        $msg = "FORBIDDEN_SUSPEND name=$n pid=$($s.pid) reason=$($s.reason)"
        $violations.Add($msg)
        Add-Content -Path $BadLog -Value $msg
      }
    }
    foreach ($t in @($st.recent_throttles)) {
      $n = [string]$t.name
      if ((Test-Forbidden $n $null) -and ($t.level -eq "suspend" -or [string]$t.reason -like "*suspend*")) {
        $msg = "FORBIDDEN_THROTTLE_SUSPEND name=$n pid=$($t.pid) reason=$($t.reason)"
        $violations.Add($msg)
        Add-Content -Path $BadLog -Value $msg
      }
      # Cursor / explorer must never be Soft-throttled either (path/name protected)
      if ($n -match '(?i)^(explorer|Cursor)\.exe$') {
        $msg = "FORBIDDEN_THROTTLE name=$n level=$($t.level) reason=$($t.reason)"
        $violations.Add($msg)
        Add-Content -Path $BadLog -Value $msg
      }
    }
    foreach ($a in @($st.recent_abuse)) {
      if ([string]$a.name -match '(?i)^(cargo|rustc|Cursor)\.exe$' -and [int]$a.score -ge 70) {
        $msg = "FALSE_ABUSE name=$($a.name) score=$($a.score)"
        $violations.Add($msg)
        Add-Content -Path $BadLog -Value $msg
      }
    }
  }

  if ($violations.Count -gt 0) {
    Write-Host "FAIL early: $($violations[0])"
    break
  }
  Start-Sleep -Seconds 5
}

# UI-closed sticky check: stop UI if any (none), keep service, ensure no new sticky after SoftOnly resume path
Write-Host "== SoftOnly resume orphans (sticky check) =="
Stop-Procs
$cfg2 = Get-Content $ConfigPath -Raw | ConvertFrom-Json
$cfg2 | Add-Member critical_guard_mode "soft_only" -Force
$cfg2 | Add-Member pause_until $null -Force
[System.IO.File]::WriteAllText($ConfigPath, ($cfg2 | ConvertTo-Json -Depth 8), [System.Text.UTF8Encoding]::new($false))
Start-Process -FilePath $ServiceExe -WindowStyle Hidden | Out-Null
Start-Sleep -Seconds 5
$stickyOk = $true
if (Test-Path $StatusPath) {
  $st = Get-Content $StatusPath -Raw | ConvertFrom-Json
  if (@($st.suspended).Count -gt 0) {
    $stickyOk = $false
    $violations.Add("STICKY_SUSPEND_AFTER_RESTART count=$(@($st.suspended).Count)")
  }
  $recovered = $st.recovered_suspends
} else {
  $recovered = 0
}
Stop-Procs
Restore-Config

$pass = ($violations.Count -eq 0) -and $stickyOk -and ($samples -gt 10)
$lines = @()
$lines += "# P2-4 false-positive evidence"
$lines += ""
$lines += "Generated: $(Get-Date -Format o)"
$lines += "Duration requested: ${Minutes}m"
$lines += ""
$lines += "| Check | Result |"
$lines += "|-------|--------|"
$lines += ("| no Explorer/Cursor/whitelist/shell Suspend | {0} |" -f $(if ($violations.Count -eq 0) {"PASS"} else {"FAIL"}))
$lines += ("| no sticky suspend after SoftOnly restart | {0} (recovered={1}) |" -f $(if ($stickyOk) {"PASS"} else {"FAIL"}), $recovered)
$lines += ("| status samples collected | {0} (n={1}) |" -f $(if ($samples -gt 10) {"PASS"} else {"FAIL"}), $samples)
$lines += ("| pressure observed during soak | {0} |" -f $(if ($sawPressure) {"yes"} else {"no (idle host)"}))
$lines += ("| any Suspend seen (non-forbidden OK) | {0} |" -f $(if ($sawAnySuspend) {"yes: $($suspendNames -join ', ')" } else {"none"}))
$lines += ""
$lines += "Mode: last_resort_suspend + periodic cargo test + disk-hog pulses; Steam/Epic path whitelist."
$lines += ""
if ($violations.Count -gt 0) {
  $lines += "## Violations"
  $lines += ""
  foreach ($v in ($violations | Select-Object -Unique)) { $lines += "- $v" }
  $lines += ""
}
$lines += '```text'
$lines += ("samples={0} pressure={1} suspend_names={2}" -f $samples, $sawPressure, ($suspendNames -join ','))
$lines += '```'

($lines -join [Environment]::NewLine) | Set-Content -Path $EvidencePath -Encoding utf8
Write-Host ""
$lines | ForEach-Object { Write-Host $_ }

if (-not $pass) {
  Write-Error "P2-4 FAILED - see $EvidencePath"
  exit 1
}
Write-Host "P2-4 PASS - evidence: $EvidencePath"
exit 0
