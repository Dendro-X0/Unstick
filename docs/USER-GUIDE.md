# Unstick — User Guide (v0.2)

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

## Updating (portable)

1. Download the latest `Unstick-*-windows-x64.zip` from the GitHub Release marked **Latest**.
2. Close the UI and stop `guardian-service` (Task Manager, or run `Uninstall-Autostart.ps1` without `-RemoveData`).
3. Extract the zip **over** your existing install folder (replace the `.exe` files).
4. Keep `%LOCALAPPDATA%\Unstick\` — that is your config, logs, and status (do not delete it to update).
5. Start `guardian-service.exe`, then `guardian-ui.exe` (or `Install-Autostart.ps1 -StartNow`).
6. Confirm the version chip in the UI matches the release tag.

There is no in-app auto-updater yet. Public builds should be Authenticode-signed when a signing cert is available; unsigned zips are for private beta only.

## Safe disk usage

On the **Guard** tab:

- **Soft %** (default 85) — limit offender I/O (VeryLow priority + working-set trim)
- **Hard %** (default 95) — temporarily pause top disk processes
- Click **Apply thresholds**

Presets: `85 / 95` or earlier intervention `70 / 90`.

## Safe available RAM (Mem Lock)

On the **Guard** tab under Controls:

- **Soft %** (default 15 available) — trim background working sets when free RAM drops below this
- **Hard %** (default 8 available) — deeper trim; pause only in **Last-resort** mode after a streak (and only with real paging pressure)
- Click **Apply RAM thresholds**

Presets: `15 / 8` or earlier `20 / 10`. Mem Lock does **not** clear the standby cache.

Mem Lock **Hard** only latches when available/commit thresholds are met **and** paging evidence is present (quiet IDE/git mapped I/O should not Hard-latch). SoftOnly never Suspends from Mem Lock.

### Event log

On the **Monitor** tab, the **Event log** shows recent throttle / suspend / resume / info lines from this session (and `events.jsonl` after a service restart).

## Whitelist

Open the **WHITELIST** tab (or Monitor → Whitelist next to a process).

Add game or app names (`steam.exe`) or path fragments (`\Epic Games\`). Whitelisted programs are **never** soft-throttled, Disk-Locked, or paused.

## Critical Guard (safe pause)

Checkbox on Guard: master enable for Critical Guard. Choose a mode in Controls:

| Mode | Default | Behavior |
|------|---------|----------|
| **Soft only** | Yes | Lowers background priority / I/O under pressure — never pauses processes |
| **Last-resort pause** | No | Same soft ladder first; only after sustained Emergency / Disk Lock Hard may pause top offenders |

The focused app (and its child processes) is never throttled or paused. When Guard is LIVE you see a **Focus · app.exe** chip.

**Never paused by default:** Explorer, Cursor/VS Code, common browsers (Chrome/Edge/Firefox/Brave), and interactive shells (Windows Terminal, PowerShell, cmd).

Paused processes (last-resort mode only) resume when pressure drops, after ~45 seconds max (**and will not be immediately re-paused**), when you Pause Guard, or after a service restart (crash recovery). Whitelist anything else you never want paused.

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
