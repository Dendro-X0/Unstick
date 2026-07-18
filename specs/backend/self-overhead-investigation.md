# Self-overhead investigation (v0.1.2)

**Status:** baseline captured; design in `self-overhead-design.md`  
**Date:** 2026-07-17  
**Goal:** Lower Unstick’s own CPU/I/O so the Guard is cheaper on low-end PCs. Not an OS-effectiveness retune.

## Session header

```text
HANDOFF ATOMIC STEP: none — plan v0.1.2 self-overhead
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-win::sensors, guardian-service::runtime, guardian-ui
PROOF BEFORE DONE:     L1 cargo test; L2 Verify-P2-Automated; L3 Measure-SelfOverhead before/after
```

## Current tick cost (code map)

| Surface | Behavior | Path |
|---------|----------|------|
| Sample cadence | Idle 2000 ms / busy 500 ms | `guardian-core` config + `runtime.rs` sleep |
| Process sample | `refresh_cpu_all` + `refresh_memory` + **`refresh_processes(All, true)`** + disks | `crates/guardian-win/src/sensors.rs` `sample()` |
| Per-process strings | `name`, `exe()` path, **`cmd()` joined every process every tick** | same |
| Status file | `serde_json::to_string_pretty` + `fs::write(status.json)` **every tick** | `apps/guardian-service/src/runtime.rs` |
| IPC | In-memory `last_status` on `GetStatus` (good) | `runtime.rs` `handle_request` |
| UI poll | ~900 ms `GetStatus` | `apps/guardian-ui/src/app.rs` |
| UI paint | Fixed `request_repaint_after(33ms)` ≈ **30 Hz** | same |

### Detect cmdline consumers

`guardian-detect` uses `cmd_line` for:

1. Script hosts (`powershell.exe`, etc.) — encoded-command heuristics  
2. Miner-token haystack (`stratum+tcp`, …) combined with name + path  

Name/path alone catch many decoys; cmdline is required for encoded PowerShell and stratum-in-args without miner-like names. **Gating cmdline to script hosts + elevated CPU/disk + top offenders preserves detect.**

Path is still needed every tick for whitelist / allow_paths / suspicious_path — **keep `exe()` path** in v0.1.2 (cmdline is the primary subtraction).

## Hypotheses (ranked)

1. **H1 — `proc.cmd()` for all PIDs** dominates string/syscall work inside `sample()`.  
2. **H2 — pretty `status.json` every 0.5–2s** adds avoidable disk + serialize cost (esp. busy band).  
3. **H3 — UI 30 Hz paint** dominates `guardian-ui` CPU while metrics update ~1 Hz.  
4. **H4 — Sample interval** is not the limiter at idle (2s already); do not slow defaults unless L3 proves otherwise.

## Baseline method

```powershell
powershell -File scripts/Measure-SelfOverhead.ps1 -StartService -IdleSeconds 60 -BusySeconds 0 -Label before
```

Metric: `% of one logical core` ≈ `100 * ΔCPU_seconds / wall_seconds` from `Get-Process .CPU`.

### Baseline results

| Label | Phase | Process | WallSec | PctOneCore | Machine | Notes |
|-------|-------|---------|---------|------------|---------|-------|
| before | idle | guardian-service | 60.06 | **4.319** | XTZJ-20221014TG | UI closed; release build pre-change |
| before | idle | guardian-ui | — | n/a | | not started |

CSV: `specs/backend/self-overhead-measure-before-20260717-222431.csv`

Method note: Windows `Get-Process .CPU` is cumulative process CPU time; `% of one core` = `100 * ΔCPU / wall`.

## Success bar (from plan)

- Service idle (UI closed): sustained **≤ ~0.5% of one logical core** (document if hardware differs).  
- UI: no fixed 30 Hz; ~1s settled + short lerp bursts.  
- No SoftOnly / P2-1 regression.

## Out of scope

Policy weights, Disk/Mem Lock thresholds, MSI/signing, sample interval default changes.
