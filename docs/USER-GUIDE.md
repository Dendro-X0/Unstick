# Unstick — User Guide (v0.1)

## What it does

Keeps your Windows desktop responsive on low-end hardware by:

- Soft-throttling heavy background work under CPU / RAM / disk pressure
- **Disk Lock** when the OS drive Active Time hits your safe thresholds
- **Critical Guard** — temporarily pausing top resource hogs (then auto-resuming)
- Optional abuse / miner-style heuristics (not antivirus)

## Quick start

1. Unzip or use the `dist` folder with:
   - `guardian-service.exe`
   - `guardian-ui.exe`
   - `guardian-tray.exe` (optional)
2. Start **service** first, then the **UI**:
   - Double-click `guardian-service.exe`
   - Double-click `guardian-ui.exe`
3. Leave the Guard tab **ARMED**. Pause 15 minutes from the big button when you need full unrestricted load.

### Autostart (recommended)

In PowerShell from the folder that contains the scripts (or from `dist` after packaging):

```powershell
pwsh -File Install-Autostart.ps1 -StartNow
# optional tray icon:
pwsh -File Install-Autostart.ps1 -Tray -StartNow
```

Remove:

```powershell
pwsh -File Uninstall-Autostart.ps1 -StopProcesses
# also wipe config/logs:
pwsh -File Uninstall-Autostart.ps1 -StopProcesses -RemoveData
```

The UI is **on demand** — it does not need to stay open for protection to work once the service is running.

## Safe disk usage

On the **Guard** tab:

- **Soft %** (default 85) — limit offender I/O (VeryLow priority + working-set trim)
- **Hard %** (default 95) — temporarily pause top disk processes
- Click **Apply thresholds**

Presets: `85 / 95` or earlier intervention `70 / 90`.

## Whitelist

Open the **WHITELIST** tab (or Monitor → Whitelist next to a process).

Add game or app names (`steam.exe`) or path fragments (`\Epic Games\`). Whitelisted programs are **never** soft-throttled, Disk-Locked, or paused.

## Critical Guard

Checkbox on Guard: when ON, emergency / Disk Lock hard may **pause** processes. They resume when pressure drops, after ~45 seconds max, when you Pause Guard, or after a service restart (crash recovery).

## Risks (read once)

- Pausing is intentional under extreme load; whitelist anything you never want frozen (games launchers, DAWs, etc.).
- Elevated (admin) apps may ignore Guard unless you run elevated or whitelist them.
- This is **user-mode** software — it cannot hard-cap disk IOPS like a kernel driver.
- Heuristics are **not** antivirus.

## Files

| Path | Purpose |
|------|---------|
| `%LOCALAPPDATA%\Unstick\config.json` | Settings, whitelist, disk thresholds |
| `%LOCALAPPDATA%\Unstick\guardian.log` | Rotating service log |
| `%LOCALAPPDATA%\Unstick\events.jsonl` | Throttle / suspend events |
| `%LOCALAPPDATA%\Unstick\suspend_ledger.json` | Crash-recovery list (cleared after resume) |

## Troubleshooting

| Symptom | Try |
|---------|-----|
| UI says offline | Start `guardian-service.exe` first |
| Disk gauge ≠ Task Manager | Wait a few samples for PDH; restart service |
| Game stuttered / paused | Whitelist the game; or Pause Guard 15m |
| “elevated process” warning | Whitelist that app or run service as admin |

Version: see **v0.1.0** in the UI header and status.
