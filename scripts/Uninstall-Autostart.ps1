# Remove Unstick autostart and optionally stop processes / AppData.
# Usage:
#   pwsh -File scripts/Uninstall-Autostart.ps1
#   pwsh -File scripts/Uninstall-Autostart.ps1 -StopProcesses -RemoveData

param(
    [switch]$StopProcesses,
    [switch]$RemoveData
)

$ErrorActionPreference = "Stop"
$RunKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"

foreach ($name in @("Unstick", "UnstickTray", "OsFreezeGuard", "OsFreezeGuardTray")) {
    if (Get-ItemProperty -Path $RunKey -Name $name -ErrorAction SilentlyContinue) {
        Remove-ItemProperty -Path $RunKey -Name $name
        Write-Host "Removed Run key: $name"
    } else {
        Write-Host "Run key not present: $name"
    }
}

if ($StopProcesses) {
    foreach ($p in @("guardian-service", "guardian-tray", "guardian-ui")) {
        Get-Process -Name $p -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Write-Host "Stopped process (if running): $p"
    }
}

if ($RemoveData) {
    foreach ($folder in @("Unstick", "OsFreezeGuard")) {
        $data = Join-Path $env:LOCALAPPDATA $folder
        if (Test-Path $data) {
            Remove-Item -Recurse -Force $data
            Write-Host "Removed $data"
        }
    }
} else {
    Write-Host "Config/logs kept under %LOCALAPPDATA%\Unstick (pass -RemoveData to delete)."
}

Write-Host "Uninstall autostart complete. Delete the dist/ folder manually if you no longer need the binaries."
