# EcoQoS + Efficiency Mode — design (v0.3)

```
HANDOFF ATOMIC STEP: none — plan v0.3 smoothness
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-core::policy → guardian-win::throttle
PROOF BEFORE DONE:     L1 policy flags; L2 build; Mem Lock L3/L4 re-run; Measure-SelfOverhead
```

## Goal

Align Unstick Soft remediation with Microsoft **Efficiency Mode** / **EcoQoS** and **ProcessMemoryPriority**, so background offenders yield CPU/thermal headroom without aggressive pagefile dumps. Hard WS shrink stays last-resort under Mem Lock Hard + paging evidence.

## Sources

- [Introducing EcoQoS](https://devblogs.microsoft.com/performance-diagnostics/introducing-ecoqos/)
- [Task Manager Efficiency Mode](https://devblogs.microsoft.com/performance-diagnostics/reduce-process-interference-with-task-manager-efficiency-mode/) — Idle priority + EcoQoS
- [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation) — `ProcessPowerThrottling`, `ProcessMemoryPriority`

## Contracts

### `PlannedAction` (additions)

| Field | Meaning |
|-------|---------|
| `apply_ecoqos` | Call `SetProcessInformation(ProcessPowerThrottling)` with `EXECUTION_SPEED` on |
| `apply_mem_priority_low` | Call `SetProcessInformation(ProcessMemoryPriority)` with `MEMORY_PRIORITY_LOW` |

Existing: `level`, `apply_disk_lock`, `apply_mem_lock`, `apply_job_cap`.

### Soft ladder (v0.3)

| Trigger | Priority class | EcoQoS | Mem priority | EmptyWorkingSet | WS shrink |
|---------|----------------|--------|--------------|-----------------|-----------|
| Warn (no disk/mem) | BelowNormal | yes | no | no | no |
| Throttle / pressure Soft | Utility→BelowNormal or Background→Idle | yes | no | no | no |
| Disk Lock Soft/Hard | as today + EcoQoS | yes | no | yes (disk) | no |
| Mem Lock Soft | soft_level + EcoQoS | yes | **yes** | yes (keep L3) | no |
| Mem Lock Hard | Idle + EcoQoS | yes | yes | yes | **yes** (~60% max) |
| Suspend (LastResort) | Idle + Suspend | yes | yes | as mem/disk | as hard |

Focus tree / protected / whitelist: never receive these flags.

### Restore (`restore_all` / resume)

- Priority → Normal  
- EcoQoS → off (`StateMask = 0` with ControlMask EXECUTION_SPEED)  
- Memory priority → leave system default (or MEMORY_PRIORITY_NORMAL if API allows)  
- I/O priority restore as today  

### SoftOnly

Unchanged: never Suspend from Mem/Disk Hard. EcoQoS + Idle are SoftOnly-safe.

### Forbidden (unchanged)

- Standby / SysMain purge  
- Kernel DPC “fixes”  
- EmptyWorkingSet on focus tree  

## Self-overhead (Normal band)

On Normal with Disk/Mem Off: refresh CPU/memory/PDH every tick; **full process enumeration every other tick**, reuse cached `ProcessSample` list on light ticks. Busy bands (Warn+) always full refresh.

## DPC advisory copy

High/Warn messages must state: Unstick cannot fix driver DPCs; capture with `wpr -start GeneralProfile -filemode` / WPA DPC/ISR graph. No remediation API.

## Acceptance

- S1 — Soft/Warn offenders get `apply_ecoqos`  
- S2 — Mem Soft sets `apply_mem_priority_low` before Hard shrink  
- S3 — Hard WS shrink only when `apply_mem_lock` and Hard mode (Idle/Suspend level path)  
- S4 — Measure-SelfOverhead improves vs 0.2 baseline on Normal idle  
- SoftOnly + Mem L3/L4 still PASS  

## Proof

| Layer | Command |
|-------|---------|
| L1 | `cargo test -p guardian-core -p guardian-win` |
| L2 | `Verify-P2-Automated.ps1` |
| L3 | `Verify-MemLock-L3.ps1`, `Verify-MemLock-L4.ps1 -Minutes 3`, `Measure-SelfOverhead.ps1` |
