# Unstick — User Guide (v0.4 hardware-control)

## Scope

Unstick is a **Windows** portable **hardware-control** utility for the **OS drive (SSD/HDD)** and **RAM**. It holds utilization just under the freeze cliff with soft actuators. It is not a full performance-optimization suite.

## What it does

Under disk / memory pressure it:

- **Disk Lock** — soft-throttles background I/O when OS-drive Active Time crosses your thresholds (protects SSD/HDD responsiveness)
- **Disk control** — closed-loop soft capping when disk load approaches the calibrated envelope (~97–99% of the freeze cliff); EcoQoS → VeryLow I/O → Idle; never Suspend
- **Mem control** — same closed-loop on RAM pressure; memory-priority first; working-set trim only when paging evidence is present (avoids IDE false positives)
- **Mem Lock** — lowers **memory priority** (and Soft working-set trim) so cold pages yield first; Hard only with paging evidence
- Soft-throttles competing background work with **EcoQoS / Efficiency Mode** as a means to keep the machine from hitching when disk/RAM are contended — not a general CPU “optimizer”
- **Hardware Guard** — Soft only by default; Suspend is experimental opt-in only
- Optional light abuse / miner-style heuristics (not antivirus)

## What it does **not** do

- Act as a comprehensive performance suite or “make my PC faster” tool
- Clear standby / SysMain cache (“RAM cleaner” style) — that hurts more than it helps
- Fix high **DPC/ISR** stutter from bad drivers — Unstick only warns; use WPR/WPA (see below)
- Replace antivirus or Task Manager
- Run on macOS/Linux — **Windows x64 portable only** (no other-OS installers)

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

## Hardware control (primary)

On the **Guard** tab → **Controls**:

- Shows live **envelope** (learning idle → calibrated), **u_disk** / **u_mem**, and control mode (`released` / `holding` / `capping`) with intensity.
- Closed-loop soft capping targets ~**97–99%** of the freeze cliff for OS disk and RAM.
- Chips on the hero: **Disk cap** / **RAM cap** when actively controlling.

Leave **Hardware Guard** checked (ARMED). Soft only is the product path.

### Advanced thresholds (optional)

Under **Advanced thresholds ▸** (collapsed by default):

- **Disk Soft/Hard %** — legacy Active Time tripwires (safety net alongside closed-loop)
- **RAM Soft/Hard available %** — Mem Lock tripwires; Hard still requires paging evidence

Presets remain available (`85/95`, `70/90`, `15/8`, `20/10`). These do **not** clear standby cache.

### Event log

On the **Monitor** tab, the **Event log** shows recent throttle / suspend / resume / info lines from this session (and `events.jsonl` after a service restart).

### DPC / ISR warnings

If Guard shows **DPC/ISR · elevated/high**, stutter is likely from a **driver**, not an app Unstick can throttle. Capture a trace:

```powershell
wpr -start GeneralProfile -filemode
# reproduce the hitch for ~30–60s
wpr -stop $env:TEMP\unstick-dpc.etl
```

Open the `.etl` in **Windows Performance Analyzer** → add **DPC/ISR** by module; update or roll back the offending `.sys` (GPU, network, audio, chipset).

## Limits / honesty

Unstick is a **user-mode** Guard. It cannot:

- Cure kernel DPC/ISR latency (drivers) — it only advises and points you at WPR/WPA
- Guarantee zero hitching under Extreme memory pressure
- Replace Task Manager, Resource Monitor, or antivirus
- Prevent hardware damage (firmware/SMART/thermal still own that)

Soft remediation uses **EcoQoS** and **memory priority** (Microsoft Efficiency Mode style). **NtSuspend is not part of the product path** unless you explicitly opt in (see Hardware Guard).

## Whitelist

Open the **WHITELIST** tab (or Monitor → Whitelist next to a process).

Add game or app names (`steam.exe`) or path fragments (`\Epic Games\`). Whitelisted programs are **never** soft-throttled, Disk-Locked, or paused.

## Hardware Guard

Checkbox on Guard: master enable for closed-loop disk/RAM control + Soft Disk/Mem Lock.

| Mode | Default | Behavior |
|------|---------|----------|
| **Soft only** | **Yes (product path)** | Closed-loop soft control — **never** NtSuspend |
| **Last-resort pause** | Hidden unless opted in | Experimental: may NtSuspend top offenders after sustained Emergency |

**Opt-in Suspend (not recommended):** set `"experimental_suspend": true` in `%LOCALAPPDATA%\Unstick\config.json`, restart the service, then Last-resort appears in the UI. Suspend can leave apps stuck; Soft restore is preferred.

The focused app (and its child processes) is never throttled or paused. When Guard is LIVE you see a **Focus · app.exe** chip.

**Never Soft-throttled by default:** Explorer, Cursor/VS Code, common browsers (Chrome/Edge/Firefox/Brave), and interactive shells (Windows Terminal, PowerShell, cmd).

## Risks (read once)

- Soft demotions (priority / EcoQoS / mem-priority) are restored when pressure drops or a process leaves the plan.
- Elevated (admin) apps may ignore Guard unless you run elevated or whitelist them.
- This is **user-mode** software — it cannot hard-cap disk IOPS like a kernel driver.
- Heuristics are **not** antivirus.

## Files

| Path | Purpose |
|------|---------|
| `%LOCALAPPDATA%\Unstick\config.json` | Settings, whitelist, disk thresholds |
| `%LOCALAPPDATA%\Unstick\envelope_profile.json` | Idle-calibrated hardware envelope (D2) |
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

Version: see the version chip in the UI header and `status.version`.
