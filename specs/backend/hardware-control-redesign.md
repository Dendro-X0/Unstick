# Hardware-control redesign (Option 2) — design

```
HANDOFF ATOMIC STEP: none — greenfield Option 2 (user chose redesign over abandon)
PAUSED / CANCELLED:    Suspend-as-primary remediation (retire from default product)
CANONICAL OWNER:       guardian-core::control (new) ← sensors → envelope → actuator
PROOF BEFORE DONE:     named per phase below; no “smarter” claim without L3 soak
```

## Decision

**Chosen:** Redesign Unstick as a **Windows hardware-control exploration** — read local disk/RAM state, derive a **safe operational envelope**, and **closed-loop cap** utilization near the top of that envelope (~**97–99%** of the freeze cliff), instead of crude Soft/Hard bands and forced Suspend.

**Rejected for this path:** Abandon; further polish of threshold Soft/Hard + NtSuspend as the center of the product.

## Product job (locked)

| In | Out |
|----|-----|
| Windows x64 portable only | Other OS / installers |
| Protect **OS drive (SSD/HDD)** and **RAM** from freeze-inducing overload | General “PC optimizer” suite |
| Soft actuators: EcoQoS, I/O priority, memory priority, optional WS trim | Default **NtSuspend** / force-terminate without guaranteed resume |
| Freeze / thrash **mitigation** | Claimed prevention of SSD wear or thermal **hardware damage** |

Honesty: user-mode can reduce hitching; it cannot replace firmware SMART/thermal limits. Docs must say **freeze avoidance**, not “hardware damage prevention.”

## Why the current approach fails

1. **Open-loop thresholds** — Soft/Hard fire after crossing fixed % lines; no setpoint that *holds* just under collapse.
2. **Crude simplification** — band jumps (BelowNormal → Idle → Suspend) instead of continuous intensity.
3. **Suspend harm** — `NtSuspendProcess` can leave apps frozen with no reliable auto-recovery ([stuck-suspend-investigation.md](stuck-suspend-investigation.md)).
4. **Weak value on modern cached SSDs** — Active Time % alone is a poor “collapse” proxy without latency/queue and machine-specific headroom.

## Control model

```mermaid
flowchart LR
  sense[Sense_disk_RAM_latency_queue_paging]
  calib[Calibrate_envelope_idle_and_load]
  err[Error_vs_97_99_setpoint]
  act[Actuate_EcoQoS_IoPrio_MemPrio]
  sense --> calib --> err --> act
  act --> sense
```

### 1. Sense (hardware status)

Reuse and prioritize existing sensors; treat utilization as **secondary** to **service-time / thrash**:

| Axis | Primary signals (already or near-shipped) | Collapse proxy |
|------|-------------------------------------------|----------------|
| Disk | `disk_latency_sec`, queue length, Active Time / busy | Latency + queue saturation (see [disk-latency-tripwire-design.md](disk-latency-tripwire-design.md)) |
| RAM | available %, commit %, hard-fault / pages-sec, paging evidence | `memory_full` / paging (see [psi-shaped-pressure-design.md](psi-shaped-pressure-design.md)) |

**Not** the sole control input: raw CPU %, fixed Soft/Hard % alone.

### 2. Calibrate (local safe thresholds)

Per machine / boot (or weekly re-learn):

| Phase | Behavior |
|-------|----------|
| Idle baseline | Collect p50/p95 latency, queue, free RAM when Guard is ARMED and band quiet |
| Stress glimpse | Optional short natural-load observation (no synthetic hog unless L3 probe) |
| Envelope | `disk_ceiling`, `mem_ceiling` = values just **below** observed freeze-risk region |

Defaults until calibrated: conservative envelopes from current Soft latency / Soft avail % — then **tighten toward 97–99% of calibrated ceiling**, not toward 100% of Active Time.

### 3. Setpoint (97–99%)

Define normalized load `u ∈ [0,1]` per axis (disk, memory) relative to that axis’s envelope:

```text
u_disk = f(latency, queue, busy) / disk_ceiling_norm
u_mem  = g(commit, avail, paging) / mem_ceiling_norm
```

Target:

```text
u_set ∈ [0.80, 0.88]   # freeze-safe headroom (was 0.97–0.99; see freeze-safe-dynamic-control-design.md)
```

Controller (v1 exploration): **PI-like** or **bang-bang with hysteresis** on error `e = u - u_set`:

- `e < −deadband` → release actuators (restore Normal / clear EcoQoS)
- `|e| ≤ deadband` → hold
- `e > +deadband` → increase soft intensity on **background offenders only** (focus tree untouched)

No step Soft/Hard labels required in the control loop; UI may show “holding / releasing / capping” instead.

### 4. Actuate (refined, not Suspend)

Ordered intensity (continuous ladder, single tick may apply one step up/down):

1. EcoQoS on + BelowNormal (Efficiency Mode style) — [ecoqos-efficiency-design.md](ecoqos-efficiency-design.md)
2. VeryLow I/O priority (disk axis dominant)
3. `ProcessMemoryPriority` LOW (memory axis dominant)
4. Soft `EmptyWorkingSet` only if paging evidence (keep Mem Lock L4 gate)
5. **Never default:** `NtSuspendProcess` — Last-resort Suspend removed from default mode; code path may remain behind explicit experimental flag or be deleted in a later slice

Auto-recovery invariant: every demotion has a **paired restore** on release; ledger must not drop failed restores ([stuck-suspend-investigation.md](stuck-suspend-investigation.md) lessons apply to EcoQoS/mem-prio restore too).

## Architecture delta

| Today | Option 2 |
|-------|----------|
| `pressure` bands → Soft/Hard Disk/Mem Lock | `envelope` + `u` + setpoint error → intensity |
| `policy::plan` Suspend on Emergency | Soft-only control by default |
| Fixed `mem_avail_soft_pct` / disk busy % as primary | Calibrated ceilings; fixed % as bootstrap only |
| “Protection utility” messaging | Same scope + **hardware-control** exploration honesty |

Canonical owner sketch:

- `guardian-core`: `Envelope`, `Utilization`, `ControlLoop` (new module or evolve `pressure`)
- `guardian-win`: sensors unchanged + restore fidelity on actuators
- `guardian-service`: wire sample → control → apply
- UI: show envelope / `u` / setpoint; deprecate Soft/Hard % as primary mental model (advanced override OK)

## Explicit non-goals (this redesign)

- Kernel minifilter / true IOPS QoS job caps if OS unsupported
- Standby purge
- Claiming SSD longevity or thermal damage prevention
- macOS / Linux product
- Game booster / suite features

## Phased delivery

| Phase | Slice | Proof |
|-------|-------|-------|
| **D0** | This design + roadmap pointer; **no behavior change** | Peer read | **Done** |
| **D1** | SoftOnly-only default product path; disable Suspend in default config; harden restore ledger for EcoQoS/mem-prio | L1 + soak: no stuck Suspend orphans | **Done** — `experimental_suspend` (default false); soft `restore_not_in_plan` |
| **D2** | Envelope calibration (idle baseline → ceilings in status/config) | L3 status shows calibrated fields | **Done** — `EnvelopeCalibrator` + `status.envelope` + `envelope_profile.json` |
| **D3** | Closed-loop setpoint on disk axis (latency/queue → intensity) | L3: under disk hog, `u_disk` stays near setpoint without Suspend | **Done** — `DiskControlLoop` bang-bang → EcoQoS / VeryLow I/O / Idle |
| **D4** | Closed-loop memory axis (paging-gated) | L3/L4: reuse Mem Lock FP matrix — no Hard latch on IDE mapped I/O | **Done** — `MemControlLoop`; WS trim only if `paging_pressure_evidence` |
| **D5** | UI + USER-GUIDE: hardware-control framing; Soft/Hard % as advanced | Manual UI | **Done** — Guard Hardware control panel; Advanced thresholds ▸ |

Suggested first version tag after D1–D3 land: **`v0.4.0` (Hardware control)** — exploratory, still unsigned honesty until Authenticode.

## D1 notes

- Config: `experimental_suspend` (default `false`). Load forces `critical_guard_mode=soft_only` unless the flag is set.
- Policy / IPC: Suspend plans only when `suspend_allowed()`; UI hides Last-resort unless flag is on.
- Throttle: `restore_not_in_plan` clears EcoQoS / mem-priority / priority when a PID leaves the action plan; suspend ledger remove only after successful resume.

## D2 notes

- Module: `guardian-core::envelope` — idle window (quiet + ARMED + Normal) → p50/p95 latency/queue/avail.
- Until primed: ceilings = Soft lines (conservative). After ≥30 idle samples: lift toward ~98% of Hard; persist `%LOCALAPPDATA%\Unstick\envelope_profile.json`.
- Status: `envelope` (`calibrated`, ceilings, `u_disk` / `u_mem`, `u_set_lo/hi` **0.80–0.88** freeze-safe band; see `freeze-safe-dynamic-control-design.md`).

## D3 notes

- Module: `guardian-core::control` — bang-bang on `u_disk` vs `u_set_lo`/`u_set_hi` with 2-tick **escalation** hold, 0-tick release, stress band shift, soft intensity cap 2.
- Intensity 1→EcoQoS+BelowNormal, 2→+VeryLow I/O (default max); never Suspend. Config: `disk_control_enabled` (default true).
- Soft demotion TTL (`max_soft_demote_secs`, default 45) + fast release when `u` eases.
- Status: `disk_control_intensity`, `disk_control_mode` (`released`/`holding`/`capping`). Soft Disk Lock bands still run as a parallel safety net.

## D4 notes

- Same bang-bang loop on `u_mem`; intensity 1→EcoQoS+mem-prio, 2→+WS trim **only when** `paging_pressure_evidence` (mapped-I/O / IDE FP safe).
- Config: `mem_control_enabled` (default true). Status: `mem_control_intensity`, `mem_control_mode`. Never Suspend.

## D5 notes

- Guard Controls primary: envelope + `u_disk`/`u_mem` + control mode/intensity.
- Soft/Hard % sliders live under **Advanced thresholds ▸**.
- Hero chips: Disk/RAM cap·hold; checkbox label **Hardware Guard**.

## Risks

| Risk | Mitigation |
|------|------------|
| Sitting near cliff utilization still freezes | Freeze-safe 0.80–0.88 band + stress headroom shift |
| Calibration never sees real cliff | Keep conservative bootstrap; optional guided “learn” soak |
| Continuous EcoQoS thrash | Escalation hold + soft TTL recovery windows |
| Users expect “damage prevention” | Docs claim freeze mitigation only |

## First atomic step after approval of this doc

**D1:** Product default = SoftOnly control path; Suspend not applied unless experimental flag; document in USER-GUIDE + roadmap. No closed-loop yet.

## Related

- [guardian-design.md](guardian-design.md) — superseded in spirit for Suspend/bands; keep as historical v1
- [stuck-suspend-investigation.md](stuck-suspend-investigation.md)
- [disk-latency-tripwire-design.md](disk-latency-tripwire-design.md)
- [psi-shaped-pressure-design.md](psi-shaped-pressure-design.md)
- [ecoqos-efficiency-design.md](ecoqos-efficiency-design.md)
