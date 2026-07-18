# Focus-aware progressive scheduling

## Goal

Boost the user’s focused app and progressively ease background work by pressure band. Prefer soft throttle over `NtSuspend`. Users choose Critical Guard mode:

| Mode | Default | Behavior |
|------|---------|----------|
| `soft_only` | **Yes** | Priority / I/O / WS trim / job caps only — never Suspend |
| `last_resort_suspend` | No | Same ladder; Suspend only after Emergency/Disk Hard soft streak ≥ N |

## Focus

- Each sample: `GetForegroundWindow` → focus PID
- Protect focus PID + descendants (via `parent_pid` walk in sample)
- Status: `focus_pid`, `focus_name`, `focus_profile` (`dev` | `play` | `other`) — profile is UI label only

## Progressive ladder

| Band / Disk | Focused tree | Background |
|-------------|--------------|------------|
| Warn | AboveNormal boost | none |
| Throttle / Disk Soft | boost | BelowNormal (+ disk soft tools) |
| Emergency / Disk Hard | boost; never Suspend | Idle + disk tools; Suspend only if last_resort **and** streak ≥ 3 |

## IPC / config

- `critical_guard_mode`: `soft_only` | `last_resort_suspend` (default soft_only)
- `emergency_suspend` remains master enable for Critical Guard
- `SetCriticalGuardMode { mode }`

## Proof

- L1: unit tests soft_only never Suspend; focus never Suspend; last_resort needs streak
- L3: smoke focus chip updates when switching windows
