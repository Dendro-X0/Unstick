# Install HKCU Run autostart for Unstick (no admin required).
# Usage:
#   pwsh -File scripts/Install-Autostart.ps1
#   pwsh -File scripts/Install-Autostart.ps1 -InstallDir "D:\Tools\Unstick" -Tray -StartNow

param(
    [string]$InstallDir = "",
    [switch]$Tray,
    [switch]$StartNow
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $cand = Join-Path $Root "dist"
    if (Test-Path (Join-Path $cand "guardian-service.exe")) {
        $InstallDir = $cand
    } else {
        $InstallDir = Join-Path $Root "target\release"
    }
}

$Service = Join-Path $InstallDir "guardian-service.exe"
$TrayExe = Join-Path $InstallDir "guardian-tray.exe"

if (-not (Test-Path $Service)) {
    throw "guardian-service.exe not found in $InstallDir. Run Package-Portable.ps1 first."
}

$RunKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
# Quoted path so spaces in "Experimental projects" work
$ServiceCmd = "`"$Service`""
Set-ItemProperty -Path $RunKey -Name "Unstick" -Value $ServiceCmd
Write-Host "Autostart: Unstick -> $Service"

if ($Tray) {
    if (-not (Test-Path $TrayExe)) {
        throw "guardian-tray.exe not found in $InstallDir"
    }
    $TrayCmd = "`"$TrayExe`" --tray"
    Set-ItemProperty -Path $RunKey -Name "UnstickTray" -Value $TrayCmd
    Write-Host "Autostart: UnstickTray -> $TrayExe --tray"
} else {
    Write-Host "Tray not enabled (pass -Tray to autostart tray). Open guardian-ui.exe when you want the UI."
}

if ($StartNow) {
    Start-Process -FilePath $Service
    Write-Host "Started guardian-service."
    if ($Tray -and (Test-Path $TrayExe)) {
        Start-Process -FilePath $TrayExe -ArgumentList "--tray"
        Write-Host "Started guardian-tray."
    }
}

Write-Host "Done. UI is on-demand: $InstallDir\guardian-ui.exe"
