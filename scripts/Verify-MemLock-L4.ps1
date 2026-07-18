# Verify Mem Lock L4 — false-positive (mapped I/O / IDE) no Hard latch
#
# Forces hard_raw via high mem_avail_hard_pct while paging evidence stays false
# (healthy available RAM + quiet pagefile). Soft may latch; Hard must not.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/Verify-MemLock-L4.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/Verify-MemLock-L4.ps1 -Minutes 5

param(
    [int]$Minutes = 5
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $Root) { $Root = (Get-Location).Path }
Set-Location $Root

$Unstick = Join-Path $env:LOCALAPPDATA "Unstick"
$ConfigPath = Join-Path $Unstick "config.json"
$StatusPath = Join-Path $Unstick "status.json"
$EvidenceDir = Join-Path $Root "specs/backend"
$EvidencePath = Join-Path $EvidenceDir "mem-lock-l4-evidence.md"
$ChecklistPath = Join-Path $EvidenceDir "mem-lock-l4-checklist.md"
$BackupPath = Join-Path $Unstick "config.json.memlock-l4.bak"
$BadLog = Join-Path $Unstick "memlock-l4-bad.log"

New-Item -ItemType Directory -Force -Path $Unstick | Out-Null
New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
if (Test-Path $BadLog) { Remove-Item $BadLog -Force }

$ForbiddenName = '(?i)^(explorer|csrss|dwm|winlogon|lsass|Cursor|Code)\.exe$'

function Stop-Procs {
  Get-Process -Name "guardian-service","mapped-io-hog","mem-hog" -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 1
}

function Restore-Config {
  if (Test-Path $BackupPath) {
    Copy-Item $BackupPath $ConfigPath -Force
    Remove-Item $BackupPath -Force
  }
}

function Write-Bad($msg) {
  Add-Content -Path $BadLog -Value $msg
  Write-Host "FAIL: $msg" -ForegroundColor Red
}

Write-Host "== L1 mem_hard_requires_paging =="
cargo test -p guardian-core mem_hard_requires_paging -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "== stop =="
Stop-Procs

Write-Host "== build service + mapped-io-hog =="
cargo build -p guardian-service --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release --manifest-path fixtures/mapped_io_hog/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ServiceExe = Join-Path $Root "target/release/guardian-service.exe"
$HogExe = Join-Path $Root "fixtures/mapped_io_hog/target/release/mapped-io-hog.exe"
if (-not (Test-Path $HogExe)) {
  $alt = Get-ChildItem -Recurse -Filter "mapped-io-hog.exe" -Path (Join-Path $Root "fixtures/mapped_io_hog") |
    Select-Object -First 1
  if ($alt) { $HogExe = $alt.FullName }
}
if (-not (Test-Path $HogExe)) { Write-Error "mapped-io-hog.exe not found"; exit 1 }

$hadConfig = Test-Path $ConfigPath
if ($hadConfig) {
  Copy-Item $ConfigPath $BackupPath -Force
  $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
} else {
  $cfg = [pscustomobject]@{}
}

$cfg | Add-Member -NotePropertyName mem_lock_enabled -NotePropertyValue $true -Force
# Soft may latch; Hard raw is intentionally easy, but paging gate must block Hard.
$cfg | Add-Member -NotePropertyName mem_avail_soft_pct -NotePropertyValue 99.0 -Force
$cfg | Add-Member -NotePropertyName mem_avail_hard_pct -NotePropertyValue 99.0 -Force
$cfg | Add-Member -NotePropertyName mem_lock_hard_requires_paging -NotePropertyValue $true -Force
$cfg | Add-Member -NotePropertyName critical_guard_mode -NotePropertyValue "soft_only" -Force
$cfg | Add-Member -NotePropertyName emergency_suspend -NotePropertyValue $true -Force
$cfg | Add-Member -NotePropertyName pause_until -NotePropertyValue $null -Force
$json = $cfg | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($ConfigPath, $json, [System.Text.UTF8Encoding]::new($false))

Write-Host "== start guardian-service =="
$svc = Start-Process -FilePath $ServiceExe -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 3

Write-Host "== start mapped-io-hog 256 MiB =="
$hog = Start-Process -FilePath $HogExe -ArgumentList "256" -PassThru -WindowStyle Hidden

$deadline = (Get-Date).AddMinutes($Minutes)
$hardSeen = $false
$samples = 0
$softSeen = $false
$cargoPulseEvery = 45
$lastCargo = (Get-Date).AddSeconds(-$cargoPulseEvery)

Write-Host ("== soak {0} minutes (Hard must never latch) ==" -f $Minutes)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Seconds 2
  if (-not (Test-Path $StatusPath)) { continue }
  try {
    $st = Get-Content $StatusPath -Raw | ConvertFrom-Json
  } catch {
    continue
  }
  $samples++
  $ml = "$($st.mem_lock)".ToLowerInvariant()
  if ($ml -eq "soft") { $softSeen = $true }
  if ($ml -eq "hard") {
    $hardSeen = $true
    Write-Bad ("mem_lock=hard at sample {0} avail_hint stall_memory={1}" -f $samples, $st.stall_memory)
  }
  if ($st.recent_throttles) {
    foreach ($t in @($st.recent_throttles)) {
      $reason = "$($t.reason)"
      $name = "$($t.name)"
      if ($reason -match 'mem_lock:hard' -or $reason -match 'mem_lock:.*:suspend') {
        Write-Bad ("throttle {0} pid={1} reason={2}" -f $name, $t.pid, $reason)
        $hardSeen = $true
      }
      if ($name -match $ForbiddenName -and ($reason -match 'mem_lock:hard' -or $reason -match 'mem_lock:.*:suspend')) {
        Write-Bad ("forbidden name mem_lock hard/suspend: {0} {1}" -f $name, $reason)
        $hardSeen = $true
      }
      if ($name -match $ForbiddenName -and $reason -match 'mem_lock:soft') {
        Write-Bad ("forbidden name mem_lock soft (protection miss): {0} {1}" -f $name, $reason)
        $hardSeen = $true
      }
    }
  }

  if (((Get-Date) - $lastCargo).TotalSeconds -ge $cargoPulseEvery) {
    $lastCargo = Get-Date
    Write-Host "== cargo check pulse (IDE-like) =="
    cargo check -p guardian-core --quiet 2>$null
  }
}

Write-Host "== stop =="
Stop-Procs
Restore-Config

$pass = -not $hardSeen -and $samples -ge 10
$when = (Get-Date).ToString("o")

$evidence = @"
# Mem Lock L4 evidence

Generated: $when

| Check | Result |
|-------|--------|
| L4-4 unit ``mem_hard_requires_paging`` | PASS (ran at probe start) |
| Samples | $samples over ${Minutes}m |
| Soft seen (optional) | $(if ($softSeen) { 'yes' } else { 'no' }) |
| Hard latch / mem_lock:hard throttle | $(if ($hardSeen) { 'FAIL' } else { 'PASS (never)' }) |
| Forbidden names mem_lock throttle | $(if (Test-Path $BadLog) { 'see bad log' } else { 'PASS' }) |

Probe config: mem_avail_soft/hard_pct=99, mem_lock_hard_requires_paging=true, SoftOnly.
Workload: mapped-io-hog 256 MiB re-touch + cargo check pulses.

``````
bad_log_exists=$(Test-Path $BadLog)
``````
"@

# Fix the evidence markdown properly
$badNote = if (Test-Path $BadLog) { Get-Content $BadLog -Raw } else { "(none)" }
$evidence = @"
# Mem Lock L4 evidence

Generated: $when

| Check | Result |
|-------|--------|
| L4-4 unit ``mem_hard_requires_paging`` | PASS (ran at probe start) |
| Samples | $samples over ${Minutes}m |
| Soft seen (optional) | $(if ($softSeen) { 'yes' } else { 'no' }) |
| Hard latch / mem_lock:hard throttle | $(if ($hardSeen) { 'FAIL' } else { 'PASS (never)' }) |

Probe config: mem_avail_soft/hard_pct=99, mem_lock_hard_requires_paging=true, SoftOnly.
Workload: mapped-io-hog 256 MiB re-touch + cargo check pulses.

## Bad events

``````
$badNote
``````

Overall: $(if ($pass) { 'PASS' } else { 'FAIL' })
"@
[System.IO.File]::WriteAllText($EvidencePath, $evidence, [System.Text.UTF8Encoding]::new($false))

# Update checklist sign-off table dates via simple replace
if (Test-Path $ChecklistPath) {
  $cl = Get-Content $ChecklistPath -Raw
  $day = (Get-Date).ToString("yyyy-MM-dd")
  $res = if ($pass) { "PASS" } else { "FAIL" }
  $cl = $cl -replace '\| L4-1\.\.L4-3 automated probe \| _\(fill\)_ \| _\(fill\)_ \|', "| L4-1..L4-3 automated probe | $day | **$res** |"
  $cl = $cl -replace '\| L4-4 unit \| _\(fill\)_ \| _\(fill\)_ \|', "| L4-4 unit | $day | **PASS** |"
  [System.IO.File]::WriteAllText($ChecklistPath, $cl, [System.Text.UTF8Encoding]::new($false))
}

Write-Host ("Wrote $EvidencePath (pass={0})" -f $pass)
if (-not $pass) { exit 1 }
Write-Host "Mem Lock L4 PASS."
exit 0
