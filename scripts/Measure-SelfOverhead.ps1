# Measure Unstick guardian-service / guardian-ui CPU over idle (and optional busy) windows.
# Usage:
#   powershell -File scripts/Measure-SelfOverhead.ps1
#   powershell -File scripts/Measure-SelfOverhead.ps1 -IdleSeconds 60 -BusySeconds 0 -Label before
# Requires guardian-service running (starts release build if -StartService).

param(
    [int]$IdleSeconds = 60,
    [int]$BusySeconds = 0,
    [string]$Label = "measure",
    [switch]$StartService,
    [switch]$StartUi
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

function Get-ProcCpu([string]$Name) {
    $procs = Get-Process -Name $Name -ErrorAction SilentlyContinue
    if (-not $procs) { return $null }
    $sum = 0.0
    foreach ($p in $procs) { $sum += [double]$p.CPU }
    return @{ Count = @($procs).Count; CpuSeconds = $sum }
}

function Sample-Window([string]$ProcName, [int]$Seconds, [string]$Phase) {
    $t0 = Get-Date
    $a = Get-ProcCpu $ProcName
    if (-not $a) {
        Write-Host "WARN: process '$ProcName' not found during $Phase"
        return $null
    }
    Start-Sleep -Seconds $Seconds
    $b = Get-ProcCpu $ProcName
    if (-not $b) {
        Write-Host "WARN: process '$ProcName' exited during $Phase"
        return $null
    }
    $elapsed = ((Get-Date) - $t0).TotalSeconds
    $deltaCpu = [Math]::Max(0.0, $b.CpuSeconds - $a.CpuSeconds)
    # % of one logical core ≈ 100 * cpu_seconds / wall_seconds
    $pctOneCore = if ($elapsed -gt 0) { 100.0 * $deltaCpu / $elapsed } else { 0.0 }
    [pscustomobject]@{
        Label     = $Label
        Phase     = $Phase
        Process   = $ProcName
        Instances = $b.Count
        WallSec   = [Math]::Round($elapsed, 2)
        DeltaCpuSec = [Math]::Round($deltaCpu, 4)
        PctOneCore  = [Math]::Round($pctOneCore, 3)
        Machine   = $env:COMPUTERNAME
        WhenUtc   = (Get-Date).ToUniversalTime().ToString("o")
    }
}

$svcExe = Join-Path $Root "target\release\guardian-service.exe"
$uiExe = Join-Path $Root "target\release\guardian-ui.exe"

if ($StartService) {
    if (-not (Test-Path $svcExe)) {
        Write-Host "==> cargo build --release -p guardian-service"
        cargo build --release -p guardian-service
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    $existing = Get-Process -Name "guardian-service" -ErrorAction SilentlyContinue
    if (-not $existing) {
        Write-Host "==> starting guardian-service"
        Start-Process -FilePath $svcExe -WorkingDirectory $Root
        Start-Sleep -Seconds 3
    }
}

if ($StartUi) {
    if (-not (Test-Path $uiExe)) {
        Write-Host "==> cargo build --release -p guardian-ui"
        cargo build --release -p guardian-ui
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    $existingUi = Get-Process -Name "guardian-ui" -ErrorAction SilentlyContinue
    if (-not $existingUi) {
        Write-Host "==> starting guardian-ui"
        Start-Process -FilePath $uiExe -WorkingDirectory $Root
        Start-Sleep -Seconds 2
    }
}

Write-Host "==> Measure-SelfOverhead label=$Label idle=${IdleSeconds}s busy=${BusySeconds}s"
$results = @()

if ($IdleSeconds -gt 0) {
    $r = Sample-Window "guardian-service" $IdleSeconds "idle"
    if ($r) { $results += $r; Write-Host ("service idle: {0}% of one core over {1}s" -f $r.PctOneCore, $r.WallSec) }
    $rui = Sample-Window "guardian-ui" $IdleSeconds "idle"
    if ($rui) { $results += $rui; Write-Host ("ui idle: {0}% of one core over {1}s" -f $rui.PctOneCore, $rui.WallSec) }
}

if ($BusySeconds -gt 0) {
    Write-Host "(busy window is wall-clock only; induce Warn band externally if needed)"
    $r = Sample-Window "guardian-service" $BusySeconds "busy"
    if ($r) { $results += $r; Write-Host ("service busy: {0}% of one core over {1}s" -f $r.PctOneCore, $r.WallSec) }
}

$outDir = Join-Path $Root "specs\backend"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$csv = Join-Path $outDir "self-overhead-measure-$Label-$stamp.csv"
$results | Export-Csv -NoTypeInformation -Path $csv
Write-Host "Wrote $csv"
$results | Format-Table -AutoSize
exit 0
