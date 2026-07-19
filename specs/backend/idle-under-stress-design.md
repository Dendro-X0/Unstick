# Idle-under-stress Efficiency Mode — design (v0.6)

```
HANDOFF ATOMIC STEP: v0.6.0 — design Idle-under-stress (no impl until gate checklist)
PAUSED / CANCELLED:    Suspend-as-default; timer-resolution; standby purge; Idle without streak/TTL
CANONICAL OWNER:       guardian-core::control + policy + guardian-win::throttle
PROOF BEFORE DONE:     L1 intensity-3 mapping; L3 WD Green cliff re-soak (hang FP + launch stutter note)
```

## Motivation (from v0.5 L3 cliff)

On `XTZJ-20221014TG` (WD Green OS+pagefile, ~948 ms avg response, Active Time 100%):

- Guard correctly reached **`disk_busy_hard — soft capping · disk i2`** (max Soft intensity today).
- Idle desktop did **not** hard-lock or crash.
- **New process / screenshot** still caused **temporary freezes** — Soft i2 (EcoQoS + BelowNormal + VeryLow I/O) cannot create latency headroom when the OS volume queue is already ~1 s deep.

Evidence: [hardware-control-l3-cliff-evidence.md](hardware-control-l3-cliff-evidence.md) Run 1.

v0.6 deepens Soft actuators **one step** toward Task Manager **Efficiency Mode** (Idle priority class + EcoQoS), only under **sustained** cliff stress, still **TTL-restored**. Honesty unchanged: mitigate freezes / ease background load — **not** zero hitch on a saturated Green SSD.

## Sources

- [EcoQoS](https://devblogs.microsoft.com/performance-diagnostics/introducing-ecoqos/)
- [Task Manager Efficiency Mode](https://devblogs.microsoft.com/performance-diagnostics/reduce-process-interference-with-task-manager-efficiency-mode/) — Idle + EcoQoS
- Existing Soft path: [ecoqos-efficiency-design.md](ecoqos-efficiency-design.md)
- Freeze-safe control: [freeze-safe-dynamic-control-design.md](freeze-safe-dynamic-control-design.md)
- North-star: [hardware-control-north-star.md](hardware-control-north-star.md)

## Contracts

### Intensity ladder (disk / mem closed-loop)

| Intensity | Priority | EcoQoS | Disk I/O | Mem | When |
|-----------|----------|--------|----------|-----|------|
| 0 | — | — | — | — | Released |
| 1 | BelowNormal | yes | no | mem-prio on mem axis | First escalate |
| 2 | BelowNormal | yes | VeryLow (disk) / WS if paging (mem) | as today | Default Soft max (**v0.5**) |
| **3** | **Idle** | **yes** | VeryLow (disk) | mem-prio; WS only if paging | **v0.6 — only if Idle gate open** |

`MAX_INTENSITY` becomes **3**, but intensity 3 is **gated** (see below). Without the gate, behavior stays v0.5 (cap at 2).

### Idle gate (must all hold)

Open Idle (allow intensity → 3) only when:

1. **Hardware Guard** armed and Soft control enabled for that axis.  
2. Closed-loop already at intensity **2** for ≥ **`idle_escalate_streak`** consecutive ticks (default **4**, ~2–8 s depending on sample interval).  
3. **Cliff signal** this tick (any one):
   - `disk_latency_sec >= disk_latency_hard_sec`, or  
   - `DiskLockMode::Hard`, or  
   - `u_disk > u_set_hi` (after stress shift) **and** disk Active Time / busy ≥ hard busy%, or  
   - mem axis: `MemLockMode::Hard` **or** (`paging_pressure_evidence` && `u_mem > u_set_hi`).  
4. **Thermal Serious** does **not** block Idle (Idle is Soft relief); Suspend remains suppressed under Serious as today.

Close Idle gate (force step-down toward ≤2, then normal release) when:

- Cliff signal clears for ≥ **`idle_release_streak`** ticks (default **2**), **or**  
- Soft demotion **TTL** fires (`max_soft_demote_secs`), **or**  
- Guard paused / Soft control disabled / intensity released by bang-bang full-release rule.

### Config (defaults)

| Key | Default | Meaning |
|-----|---------|---------|
| `idle_under_stress_enabled` | **true** | Master allow intensity 3 |
| `idle_escalate_streak` | 4 | Ticks at intensity 2 + cliff before Idle |
| `idle_release_streak` | 2 | Ticks without cliff before leaving Idle |

Serde defaults keep old configs valid (feature on by default once shipped; can disable for FP soaks).

### Policy / plan reasons

| Intensity | `reason` |
|-----------|----------|
| 3 disk | `disk_control:efficiency_idle` |
| 3 mem | `mem_control:efficiency_idle` (WS still paging-gated) |

Never Suspend. Focus tree / protected / whitelist never receive Idle.

### Throttle / restore

- Idle = `IDLE_PRIORITY_CLASS` + EcoQoS on (Task Manager Efficiency Mode parity).  
- Soft TTL and `restore_not_in_plan` unchanged — **Idle cannot linger indefinitely**.  
- On TTL restore: full Normal + EcoQoS off + I/O restore; next tick may re-escalate only if gate still open (recovery window).

### UI

- Tripwire / cap chip: `soft capping · disk i3` (or `efficiency idle`) when intensity 3.  
- Hover: “Efficiency Mode (Idle+EcoQoS) under sustained disk/RAM cliff — auto-restores.”  
- USER-GUIDE: Idle-under-stress is Soft-only deepen; still not a zero-stutter guarantee on WD Green–class volumes.

## Explicitly out of v0.6

| Item | Why |
|------|-----|
| NtSuspend as product path | Hang risk; experimental only |
| `IGNORE_TIMER_RESOLUTION` / `timeBeginPeriod` | Rejected (S2) |
| Standby purge / RAM cleaner | Anti-goal |
| Claiming launch stutter eliminated | L3 cliff proved Soft cannot beat ~1 s OS queue alone |
| PI controller rewrite | Optional later; bang-bang + gate first |

## Implementation slices (after design approval)

| Slice | Work | Proof |
|-------|------|-------|
| **I0** | This design + roadmap-v0.6.0 stub | Spec review |
| **I1** | `MAX_INTENSITY=3` + Idle gate in `DiskControlLoop` / plan_* | **Done** (L1) |
| **I2** | Config keys + runtime wire + status intensity 0..=3 | **Done** |
| **I3** | Guard chip copy + USER-GUIDE | **Done** |
| **I4** | Re-soak WD Green cliff: i3 appears; no Explorer hang; TTL restore; note launch stutter honesty | **Done** (L3 Run 2) |
| **I5** | Version 0.6.0 + RELEASE + zip | **Done** |

## Acceptance (ship v0.6.0)

1. With gate off or streak unmet → intensity never exceeds 2 (v0.5 behavior).  
2. With sustained Hard latency / Disk Hard + i2 held → intensity reaches 3; reasons `*_efficiency_idle`.  
3. Soft TTL restores Idle PIDs within `max_soft_demote_secs`.  
4. Focus / whitelist / shells never Idle’d.  
5. L3 Run 2: i3 observed on WD Green soak; **no** prolonged hang; docs state launch stutter may remain under ~1 s response.  
6. Self-overhead idle not grossly worse than v0.5 baseline (~1.5% one-core).

## Claim language

> Under sustained OS-disk or paging cliffs, Unstick may apply Task Manager–style Efficiency Mode (Idle + EcoQoS) to background offenders, then restore automatically. This deepens soft relief; it does not guarantee smooth app launches while the boot drive average response stays near one second.

## Related

- [hardware-control-l3-cliff-evidence.md](hardware-control-l3-cliff-evidence.md)  
- [docs/roadmap-future.md](../../docs/roadmap-future.md) § v0.6.0  
- [freeze-safe-dynamic-control-design.md](freeze-safe-dynamic-control-design.md)  
