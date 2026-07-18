# Verify Mem Lock L3 — runtime soak probe
#
# Proof (AC5 + WS trim):
# 1. Temporarily raise soft available-% so Mem Lock Soft latches without thrashing the box
# 2. Start guardian-service + mem-hog (large RSS)
# 3. Assert status.mem_lock == soft and recent_throttles include mem_lock:soft for mem-hog
# 4. Assert mem-hog Working Set drops after apply
#
# Usage (from repo root):
#   powershell -File scripts/Verify-MemLock-L3.ps1
#   # or: bash scripts/verify-memlock-l3.sh

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $Root) { $Root = (Get-Location).Path }
Set-Location $Root

$Unstick = Join-Path $env:LOCALAPPDATA "Unstick"
$ConfigPath = Join-Path $Unstick "config.json"
$StatusPath = Join-Path $Unstick "status.json"
$EvidenceDir = Join-Path $Root "specs/backend"
$EvidencePath = Join-Path $EvidenceDir "mem-lock-l3-evidence.md"
$BackupPath = Join-Path $Unstick "config.json.memlock-l3.bak"

New-Item -ItemType Directory -Force -Path $Unstick | Out-Null

function Stop-Guardian {
  Get-Process -Name "guardian-service" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Get-Process -Name "mem-hog" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

Write-Host "== stop any running service/hog =="
Stop-Guardian

Write-Host "== build guardian-service + mem-hog =="
cargo build -p guardian-service --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release --manifest-path fixtures/mem_hog/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ServiceExe = Join-Path $Root "target/release/guardian-service.exe"
$HogExe = Join-Path $Root "fixtures/mem_hog/target/release/mem-hog.exe"
if (-not (Test-Path $HogExe)) {
  # cargo may place binary under fixtures/mem_hog/target when isolated workspace
  $alt = Get-ChildItem -Recurse -Filter "mem-hog.exe" -Path (Join-Path $Root "fixtures/mem_hog") -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($alt) { $HogExe = $alt.FullName }
}
if (-not (Test-Path $HogExe)) {
  Write-Error "mem-hog.exe not found"
}

# Backup + patch config for Soft latch (avail soft 99% ⇒ Soft almost always on this host)
$hadConfig = Test-Path $ConfigPath
if ($hadConfig) {
  Copy-Item $ConfigPath $BackupPath -Force
  $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
} else {
  $cfg = [pscustomobject]@{}
}
$cfg | Add-Member -NotePropertyName mem_lock_enabled -NotePropertyValue $true -Force
$cfg | Add-Member -NotePropertyName mem_avail_soft_pct -NotePropertyValue 40.0 -Force
$cfg | Add-Member -NotePropertyName mem_avail_hard_pct -NotePropertyValue 2.0 -Force
$cfg | Add-Member -NotePropertyName mem_lock_hard_requires_paging -NotePropertyValue $true -Force
$cfg | Add-Member -NotePropertyName critical_guard_mode -NotePropertyValue "soft_only" -Force
$cfg | Add-Member -NotePropertyName emergency_suspend -NotePropertyValue $true -Force
# Must be armed — a leftover pause_until skips all apply.
$cfg | Add-Member -NotePropertyName pause_until -NotePropertyValue $null -Force
# UTF-8 without BOM — serde_json rejects a BOM and would fall back to defaults.
$json = $cfg | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($ConfigPath, $json, [System.Text.UTF8Encoding]::new($false))

# Sanity: re-read thresholds we expect the service to load
$verify = Get-Content $ConfigPath -Raw | ConvertFrom-Json
if ([double]$verify.mem_avail_soft_pct -lt 30) {
  Write-Error "config patch failed (mem_avail_soft_pct=$($verify.mem_avail_soft_pct))"
  exit 1
}
if ($null -ne $verify.pause_until -and "$($verify.pause_until)" -ne "") {
  Write-Error "pause_until must be null for L3 apply (got $($verify.pause_until))"
  exit 1
}
Write-Host ("patched config mem soft={0} hard={1} pause_until=null" -f $verify.mem_avail_soft_pct, $verify.mem_avail_hard_pct)

function Restore-Config {
  if (Test-Path $BackupPath) {
    Copy-Item $BackupPath $ConfigPath -Force
    Remove-Item $BackupPath -Force
  } elseif (-not $hadConfig -and (Test-Path $ConfigPath)) {
    # leave defaults; optional cleanup of probe-only keys skipped
  }
}

# Backup already created above; stop was done before build.
Write-Host "== start guardian-service =="
$svc = Start-Process -FilePath $ServiceExe -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 3

Write-Host "== start mem-hog 384 MiB =="
$hog = Start-Process -FilePath $HogExe -ArgumentList "384" -PassThru -WindowStyle Hidden
$wsBefore = 0L
for ($i = 0; $i -lt 30; $i++) {
  Start-Sleep -Milliseconds 400
  try {
    $wsBefore = [int64](Get-Process -Id $hog.Id -ErrorAction Stop).WorkingSet64
  } catch {
    $wsBefore = 0L
  }
  if ($wsBefore -gt 200MB) { break }
}
Write-Host ("mem-hog pid={0} WS_before={1:N0} bytes" -f $hog.Id, $wsBefore)
if ($wsBefore -lt 100MB) {
  Write-Error "mem-hog did not commit a large working set (WS=$wsBefore)"
  Stop-Guardian
  Restore-Config
  exit 1
}

$deadline = (Get-Date).AddSeconds(25)
$passSoft = $false
$passThrottle = $false
$lastStatus = $null
while ((Get-Date) -lt $deadline) {
  if (Test-Path $StatusPath) {
    $lastStatus = Get-Content $StatusPath -Raw | ConvertFrom-Json
    if ($lastStatus.mem_lock -eq "soft" -or $lastStatus.mem_lock -eq "hard") {
      $passSoft = $true
    }
    $th = @($lastStatus.recent_throttles)
    foreach ($t in $th) {
      if ($t.pid -eq $hog.Id -and ($t.reason -like "mem_lock:*")) {
        $passThrottle = $true
      }
    }
    if ($passSoft -and $passThrottle) { break }
  }
  Start-Sleep -Milliseconds 800
}

Start-Sleep -Seconds 2
$wsAfter = $null
try { $wsAfter = (Get-Process -Id $hog.Id -ErrorAction Stop).WorkingSet64 } catch { $wsAfter = $null }
$wsDrop = $false
if ($null -ne $wsAfter -and $wsBefore -gt 0) {
  # EmptyWorkingSet should shrink; allow 10% drop as minimum signal
  $wsDrop = ($wsAfter -lt ($wsBefore * 0.9))
}

$protectedOk = $true
if ($lastStatus -and $lastStatus.recent_throttles) {
  foreach ($t in @($lastStatus.recent_throttles)) {
    $n = [string]$t.name
    if ($n -match '(?i)^(explorer|csrss|dwm|Cursor)\.exe$' -and ($t.reason -like "mem_lock:*")) {
      $protectedOk = $false
    }
  }
}

$lines = @()
$lines += "# Mem Lock L3 evidence"
$lines += ""
$lines += "Generated: $(Get-Date -Format o)"
$lines += ""
$lines += "| Check | Result |"
$lines += "|-------|--------|"
$lines += ("| status.mem_lock soft/hard | {0} (value={1}) |" -f $(if ($passSoft) { "PASS" } else { "FAIL" }), $(if ($lastStatus) { $lastStatus.mem_lock } else { "n/a" }))
$lines += ("| recent_throttles mem_lock for mem-hog | {0} |" -f $(if ($passThrottle) { "PASS" } else { "FAIL" }))
$lines += ("| Working Set drop on mem-hog | {0} (before={1:N0} after={2}) |" -f $(if ($wsDrop) { "PASS" } else { "FAIL" }), $wsBefore, $(if ($null -ne $wsAfter) { "{0:N0}" -f $wsAfter } else { "n/a" }))
$lines += ("| no mem_lock on explorer/csrss/dwm/Cursor | {0} |" -f $(if ($protectedOk) { "PASS" } else { "FAIL" }))
$lines += ""
$lines += "Probe config: mem_avail_soft_pct=40 (clamp max), pause_until=null, SoftOnly mode."
$lines += ""
if ($lastStatus) {
  $lines += '```json'
  $snap = [ordered]@{
    mem_lock = $lastStatus.mem_lock
    mem_lock_soft_pct = $lastStatus.mem_lock_soft_pct
    mem_lock_hard_pct = $lastStatus.mem_lock_hard_pct
    stall_memory = $lastStatus.stall_memory
    pressure_band = $lastStatus.pressure_band
    recent_throttles = @($lastStatus.recent_throttles | Select-Object -First 8)
  }
  $lines += ($snap | ConvertTo-Json -Depth 6)
  $lines += '```'
}

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
($lines -join [Environment]::NewLine) | Set-Content -Path $EvidencePath -Encoding utf8

Write-Host ""
Write-Host "--- L3 results ---"
$lines | ForEach-Object { Write-Host $_ }

Stop-Guardian
Restore-Config

$all = $passSoft -and $passThrottle -and $wsDrop -and $protectedOk
if (-not $all) {
  Write-Error "Mem Lock L3 FAILED - see $EvidencePath"
  exit 1
}
Write-Host "Mem Lock L3 PASS - evidence: $EvidencePath"
exit 0
