# Guardian Design Spec (Windows v1 + Critical Guard)

## Purpose

User-mode Windows background guardian that:

1. Prevents interactive freezes on low-end hardware by soft-throttling under CPU / disk / RAM pressure.
2. **Disk Lock:** PDH system-disk Active Time tripwires → VeryLow I/O + working-set trim; hard path suspends disk offenders (Job Object MaxIops is unavailable on Win10/11 desktop).
3. At **critical** thresholds, auto-suspends high-resource non-protected processes via `NtSuspendProcess`, then auto-resumes when pressure falls.
4. Surfaces behavioral abuse and cryptomining-like activity (heuristics, not antivirus).

## Non-goals

- Kernel / filter drivers
- Overclocking or power-plan boosters
- File quarantine / signature AV
- Indiscriminate process kill
- Standby-list purge by default (future opt-in)
- macOS / Linux

## Components

| Crate / app | Responsibility |
|-------------|----------------|
| `guardian-core` | Sample loop, pressure scoring, hysteresis, policy engine |
| `guardian-win` | Sensors (PDH / performance info), priority / I/O, job objects, NtSuspend |
| `guardian-detect` | Abuse / miner heuristics, allowlists |
| `guardian-service` | Always-on sampler + actor; IPC server |
| `guardian-ui` | Polished client (Critical Guard toggle, suspended chip) |
| `guardian-tray` | Tray + CLI companion |

## IPC

- Transport: named pipe `\\.\pipe\unstick`
- Codec: newline-delimited JSON
- Messages: `GetStatus`, `Pause { minutes }`, `Resume`, `TrustPid { pid }`, `AddAllowPath { path }`, `Events`, `SetCriticalGuard { enabled }`

## Sampling

| Condition | Interval |
|-----------|----------|
| Idle (pressure `normal`) | 2000 ms |
| `warn` / `throttle` / `emergency` | 500 ms |

Sensors per tick:

- System CPU % (0–100)
- Memory: available bytes, commit %, hard-fault / pages-per-sec rate
- Disk: system PhysicalDisk `% Idle Time` → busy = 100−idle (PDH; matches Task Manager Active Time); Avg. Disk Queue Length; MB/s estimate fallback
- Top processes: pid, name, path, parent pid, cpu %, rss, disk read/write rate

## Pressure score

Inputs normalized to 0.0–1.0:

| Signal | Weight |
|--------|--------|
| CPU utilization | 0.25 |
| Memory pressure (1 − available/total, plus commit) | 0.30 |
| Disk queue / busy | 0.35 |
| Hard-fault rate (scaled) | 0.10 |

```text
score = w_cpu*cpu + w_mem*mem + w_disk*disk + w_fault*fault
```

EMA smoothing (α = 0.35) on score to avoid flicker.

### Hysteresis bands

| Band | Enter | Exit |
|------|-------|------|
| `normal` | (default) | — |
| `warn` | score ≥ 0.55 | score &lt; 0.50 |
| `throttle` | score ≥ 0.70 | score &lt; 0.62 |
| `emergency` | score ≥ 0.85 **or** hard tripwire | score &lt; 0.72 and no tripwire |

### Hard tripwires (force emergency)

Any one forces `emergency` even if EMA score lags:

1. Available RAM &lt; 5% of total **and** hard-fault rate ≥ 500/s
2. Disk queue length ≥ 8 for ≥ 2 consecutive samples
3. Commit charge ≥ 95%
4. System-disk busy / queue / saturation vs **calibrated** hard thresholds for ≥ `disk_busy_streak` samples → tripwire `disk_busy_hard`

### Disk Lock (graduated, hardware-adaptive)

See [disk-lock-design.md](disk-lock-design.md). Soft/hard busy% and queue are learned from this machine's peak throughput and healthy baseline (`disk_lock_adaptive`, default ON). Priors `disk_busy_soft_pct` / `disk_busy_hard_pct` apply only before calibration or when adaptive is off.

| Mode | Enter | Actions |
|------|-------|---------|
| Soft | calibrated soft busy/queue/sat × streak | Idle + IoPriority VeryLow + EmptyWorkingSet; disk-ranked offenders |
| Hard | calibrated hard × streak **or** emergency | Soft + NtSuspend top disk offenders |

Whitelist / protected set never Disk-Locked. Do **not** use `SetIoRateControlInformationJobObject` (unsupported on modern desktop Windows).

## Critical Guard (action ladder)

| Band | Actions |
|------|---------|
| `normal` | Restore soft-throttled; resume suspended when ledger allows |
| `warn` | Log top consumers; boost foreground |
| `throttle` | Soft: `BELOW_NORMAL` + BACKGROUND I/O + job CPU cap for build/MCP |
| `emergency` | Soft + **NtSuspendProcess** on top offenders (if Critical Guard enabled) |

### Suspend ledger

- Track pid → suspended_at, reason, name
- Resume when band exits emergency (score &lt; 0.72 and no tripwire), **or** after `max_suspend_secs` (default 45)
- Cap: `max_suspend_pids` (default 6)
- Never suspend protected set / foreground / guardian / IDEs

### Offender selection

- Default rank: (cpu% × 0.6 + normalized_io × 0.4)
- Under Disk Lock: rank by disk bytes/sec then CPU; include top-N even if each &lt; 1 MB/s
- Under throttle: prefer build/MCP workers for job caps
- Under emergency / Disk Lock hard: any non-protected high resource process
- Cap actions per tick (default 8 soft; suspend capped separately)

### Protected set (never throttle / suspend)

- Built-in OS / shell / guardian / IDE paths
- **User whitelist** (`config.whitelist`): exe names (`steam.exe`) or path substrings (`\steam\`) — never soft-throttle, suspend, or terminate
- Session `trusted_pids`
- Foreground process (interactive)

IPC: `AddWhitelist { entry }`, `RemoveWhitelist { entry }`; status includes `whitelist: string[]`.


### Build / MCP job matching

Path / name patterns: `cargo`, `rustc`, `cl`, `link`, `msbuild`, `node`, `npm`, `pnpm`, `yarn`, `python`, `pip`, `dotnet`; cmd containing `mcp`.

Job limits under `throttle+`: CPU rate 70% (configurable).

## Abuse / miner heuristics

Unchanged from v1 — score 0–100; alert ≥ 70; throttle-on-suspect ≥ 80; never auto-delete.

## Config

Default path: `%LOCALAPPDATA%\Unstick\config.json`

```json
{
  "pause_until": null,
  "emergency_suspend": true,
  "max_suspend_pids": 6,
  "max_suspend_secs": 45,
  "disk_lock_enabled": true,
  "disk_lock_adaptive": true,
  "disk_busy_soft_pct": 85,
  "disk_busy_hard_pct": 95,
  "disk_busy_streak": 2,
  "allow_paths": [],
  "whitelist": [],
  "protected_extra": [],
  "job_cpu_rate_percent": 70,
  "sample_idle_ms": 2000,
  "sample_busy_ms": 500,
  "trusted_pids": []
}
```

Event log: `%LOCALAPPDATA%\Unstick\events.jsonl`

Events include `Suspend` / `Resume` with tripwire reason.

## Proof plan

| Layer | Command / method |
|-------|------------------|
| L1 | `cargo test -p guardian-core -p guardian-detect` (hysteresis, tripwires, protected never Suspend) |
| L2 | Mock sensor → emergency → Suspend actions; score drop → Resume |
| L3 | Soak: `cargo build` + disk thrash; desktop interactive; suspended set resumes |
| L4 | Fake high-CPU decoy suspended at tripwire; Explorer/Cursor never suspended |

## Failure modes

Without a kernel driver we cannot:

- Enforce hard IOPS/bandwidth caps via Job Objects on Win10 1607+ desktop (API unsupported) — Disk Lock uses VeryLow I/O + working-set trim + suspend instead
- Prevent kernel / driver stalls
- Control protected / elevated processes without admin rights
- Guarantee freeze prevention under full disk + exhausted pagefile
