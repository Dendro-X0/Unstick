# Disk Lock Design Spec

## Plan alignment

- **Handoff atomic step:** none — greenfield feature from Disk Lock Guard plan
- **PAUSED/CANCELLED check:** clear
- **In scope:** PDH system-disk sensing, **adaptive** soft/hard Disk Lock, config, UI chip
- **Out of scope:** kernel minifilter, Job Object MaxIops (unsupported on Win10 1607+ desktop), moving pagefile

## Meta

- **Feature:** Disk Lock — precise OS-drive freeze prevention
- **Why:** DRAM-less SATA boot SSDs hit 100% Active Time at low MB/s; fixed 85/95% thresholds do not match NVMe vs slow SATA; thresholds must track **this machine's** storage

## Contracts

### Config (`GuardianConfig`)

| Field | Default | Meaning |
|-------|---------|---------|
| `disk_lock_enabled` | true | Master switch |
| `disk_lock_adaptive` | true | Learn **queue** sensitivity from local disk |
| `disk_busy_soft_pct` | 85 | **User safe soft** — Disk Lock limits I/O at this Active Time % |
| `disk_busy_hard_pct` | 95 | **User safe hard** — pause/suspend top disk offenders |
| `disk_busy_streak` | 2 | Consecutive samples required |

Busy% triggers are always the user/config values. Adaptive mode only tunes queue thresholds. Latency soft/hard (15ms / 40ms) are fixed config defaults — see `disk-latency-tripwire-design.md`.

IPC: `SetDiskSafeThresholds { soft_pct, hard_pct }` — Guard UI sliders + Apply.

### Status

| Field | Meaning |
|-------|---------|
| `disk_lock` | `off` / `soft` / `hard` |
| `disk_lock_soft_pct` / `disk_lock_hard_pct` | Live thresholds |
| `disk_calibrated` | Enough samples to trust adaptive bands |
| `disk_saturation` | 0–1 hardware saturation index |
| `disk_peak_io_bps` | Learned peak useful throughput |

### PlannedAction

- `apply_disk_lock: bool` — IoPriority VeryLow (0) + EmptyWorkingSet
- Reasons: `disk_lock:soft`, `disk_lock:hard`

## Ownership & data flow

```
PDH → WinSensor (busy, queue, agg io_bps)
  → DiskCalibrator.observe (learn peaks + healthy baseline)
  → DiskLockThresholds (dynamic soft/hard busy + queue)
  → score_pressure_tracked → PolicyEngine → ThrottleExecutor
  → StatusSnapshot (live thresholds + saturation)
```

| Step | Owner | Invariant |
|------|-------|-----------|
| Sense | `guardian-win::sensors` | Prefer PDH Active Time; fallback estimate |
| Calibrate | `guardian-core::disk_calibrate` | Soft/hard derived from this host's peaks/baseline |
| Trip | `guardian-core::pressure` | Soft does not force emergency; hard does |
| Plan | `guardian-core::policy` | Whitelist/protected never Disk-Locked |
| Apply | `guardian-win::throttle` | No SetIoRateControlInformationJobObject |

## Adaptive model

1. **Peak IO** — max useful bytes/sec while busy ∈ [20, 80]
2. **Peak busy / queue** — observed ceiling with slow decay
3. **Healthy EMA** — busy/queue when saturation &lt; 0.45
4. **Saturation index** — max(busy/100, queue/peak_queue, stall proxy) where stall = high busy + low throughput vs peak (DRAM-less signature)
5. **Soft busy** ≈ healthy + 0.55 × (peak − healthy), clamped [55, 92]
6. **Hard busy** ≈ healthy + 0.88 × span, ≥ soft+5, ≤ 99
7. **Soft/hard queue** from healthy queue EMA × multipliers vs peak queue

Until `MIN_SAMPLES` (40), blend prior config % with early estimate.

## Behavior

### Soft / Hard

Unchanged action ladder (VeryLow I/O + EmptyWorkingSet; hard adds NtSuspend). Enter via **calibrated** busy%, queue, or saturation ≥ 0.72 / 0.92 for `streak` samples.

## Acceptance criteria

- [ ] AC1 — PDH busy tracks Task Manager Active Time within ~15% when available
- [ ] AC2 — Adaptive soft/hard differ across slow vs fast learned profiles
- [ ] AC3 — Whitelist never receives Disk Lock / Suspend
- [ ] AC4 — Low MB/s + high busy still produces actions
- [ ] AC5 — Status exposes live soft/hard % and calibrated flag

## Proof plan

| Criterion | Layer | Command |
|-----------|-------|---------|
| AC2–AC4 | L1 | `cargo test -p guardian-core` |
| Live | L3 | Soak: thresholds move with this SSD; chip shows live % |

## Implementation slices

1. Spec + adaptive calibrator
2. Wire service + status + UI
3. Tests + dist
