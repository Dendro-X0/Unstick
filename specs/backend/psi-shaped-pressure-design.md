# PSI-shaped stall fractions — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P1 (stall fractions)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-core::pressure (model) ← sensors feed PressureInputs
PROOF BEFORE DONE:     L1 tests stalls + score; L3 status exposes stall_* fields; bands still behave
```

## Goal

Score pressure as **stall fractions** (Linux PSI-shaped: `some` / `full` for cpu · memory · io), not raw utilization alone. Windows this pass uses PDH proxies; Linux later maps `/proc/pressure/*` 1:1 into the same struct.

## Model

```rust
pub struct StallFractions {
    pub cpu_some: f32,      // 0..1 — runnable wait / CPU contention proxy
    pub memory_some: f32,   // memory scarcity
    pub memory_full: f32,   // thrash (paging + low avail)
    pub io_some: f32,       // disk busy / queue / latency
    pub io_full: f32,       // disk thrash (hard latency or sat+busy)
}
```

### Windows proxies (this pass)

| Field | Proxy |
|-------|--------|
| `cpu_some` | `max(cpu%/100, (dpc+irq)/100)` — DPC steals thread time ([WPT](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/cpu-analysis)) |
| `memory_some` | `max(used_ratio, commit/100)` |
| `memory_full` | if `paging_pressure_evidence`: `max(fault_pressure, 1-avail)` else `0` (mapped I/O discounted upstream) |
| `io_some` | `max(busy/100, queue/8, latency/15ms)` |
| `io_full` | `1` path: latency ≥ hard **or** (busy ≥ hard **and** saturation ≥ 0.85); else scaled peak of those |

### Score

```
some = 0.25*cpu_some + 0.30*memory_some + 0.35*io_some
full_boost = 0.20 * max(memory_full, io_full)
raw = clamp01(some + full_boost)
```

EMA + hysteresis + tripwires **unchanged**. Disk Lock streaks still use raw inputs.

### Status

| Field | Meaning |
|-------|---------|
| `stall_cpu` | cpu_some |
| `stall_memory` | memory_some |
| `stall_io` | io_some |
| `stall_memory_full` | memory_full |
| `stall_io_full` | io_full |

## Out of scope

- Reading `/proc/pressure` (Linux port)
- Changing Soft/Hard Disk Lock action ladder
- Thermal axis (P2)
