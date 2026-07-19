# No console pop-ups at runtime — design

```
HANDOFF ATOMIC STEP: none — UX: operate cleanly without terminal windows
PAUSED / CANCELLED:    Suspend-as-primary (unchanged)
CANONICAL OWNER:       apps/guardian-service, guardian-ui, guardian-tray mains + Start-Process
PROOF BEFORE DONE:     L1 cargo check; L3 launch service+UI — no extra conhost windows
```

## Problem

`guardian-service` (and likely `guardian-ui`) link as Windows **console** subsystem. `Start-Process` / autostart / `pnpm dev` therefore open a black terminal with tracing stdout.

## Contract

| Binary | Subsystem | Logging |
|--------|-----------|---------|
| `guardian-service` | `windows` | File only (`%LOCALAPPDATA%\Unstick\guardian.log*`); optional stdout if `UNSTICK_CONSOLE=1` |
| `guardian-ui` | `windows` | No console |
| `guardian-tray` | `windows` | `--cli` / `status` → `AllocConsole` then fmt; tray mode silent |

Dev runner: `Start-Process -WindowStyle Hidden` for service as belt-and-suspenders.

## Out of scope

- Changing log levels / log rotation policy beyond removing default stdout attach
- Converting to a Windows Service SCM install

## Proof

- L1: `cargo check -p guardian-service -p guardian-ui -p guardian-tray`
- L3: launch binaries — Task Manager shows process without console window; logs still in AppData
