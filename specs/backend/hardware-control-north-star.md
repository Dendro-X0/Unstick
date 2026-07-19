# Hardware-control north-star (post-v0.4.0)

```
HANDOFF ATOMIC STEP: none — product direction after v0.4.0 soak feedback
PAUSED / CANCELLED:    Suspend-as-primary; overclocking; standby purge; kernel DPC “fixes”; other-OS
CANONICAL OWNER:       guardian-core control/envelope + guardian-ui Guard + docs
PROOF BEFORE DONE:     Spec + roadmap; Guard sensing/capping UX; thermal stress L1
```

## Mission

Unstick is a **Windows user-mode hardware-control Guard** focused on:

1. **Targeted fluidity** — soft-demote background offenders (documented OS levers) so focus work stays responsive.
2. **Freeze / crash mitigation** — keep OS-disk latency and paging off the freeze cliff; restore when load eases.
3. **Overload relief** — under thermal/power constraint, demand more headroom and ease background heat/load; **advise**, do not claim hardware-damage prevention.

Default claim line: **freeze mitigation + load/thermal relief** — not damage prevention, not game FPS boost, not overclocking.

## Claim language

| Objective | Honest meaning | Not claimed |
|-----------|----------------|-------------|
| Targeted performance | EcoQoS / priority / I/O / mem-prio on **background** offenders | Game booster, overclock, “Smart Booster” |
| Prevent freezes/crashes | Closed-loop on `u_disk` / `u_mem` + Soft Disk/Mem Lock | Zero hitch under Extreme; cure driver DPC |
| Hardware overload | **Relief**: stress band shift + soft demotion + thermal advisory | Prevent damage, SMART, firmware, GPU VRM |

## Architecture spine

```
Sense (disk, RAM, thermal/power)
  → Envelope ceilings + u_disk / u_mem
  → Bang-bang soft control (freeze-safe band, stress headroom, fast release, soft TTL)
  → Actuators (EcoQoS, I/O, mem-prio, paging-gated WS)
  → Guard UX (sensing vs actively capping)
```

Owners: `envelope.rs`, `control.rs`, `throttle.rs`, `advisory.rs`, `apps/guardian-ui`.

## Lever matrix (OS / public sources)

| Lever | Source | Verdict |
|-------|--------|---------|
| EcoQoS / Efficiency Mode | MS Learn `SetProcessInformation` + EcoQoS blog | **Keep / deepen** |
| Memory priority | MS ProcessMemoryPriority | **Keep** |
| I/O priority VeryLow | NtSetInformationProcess | **Keep** (disk offenders) |
| EmptyWorkingSet / hard WS shrink | MS + community | **Gated only** (paging evidence) — never “RAM cleaner” |
| Focus-tree + whitelist | Product | **Keep** |
| Thermal/power proxies | `classify_thermal_power` | **Wire into control stress** |
| Timer-resolution / multimedia | MS `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION` / winmm | **Reject** — see [timer-resolution-reject.md](timer-resolution-reject.md) |
| Standby list purge | Optimizer folklore | **Reject** |
| Overclock / GPU clocks | N/A | **Reject** |
| Kernel DPC fix | Drivers | **Advisory only** (WPR) |

## Delivery (v0.5.x)

| Phase | Work | Status |
|-------|------|--------|
| P0 | This north-star + roadmap / USER-GUIDE claim sync | **Done** |
| P1 | Guard UX: sensing vs actively capping | **Done** |
| P2 | Thermal/power → control stress headroom + L1 | **Done** |
| Ship | See [docs/roadmap-v0.5.0.md](../../docs/roadmap-v0.5.0.md) S1–S4 | **Done** (unsigned 0.5.0 Latest intent) |
| P3 | Efficiency Mode Idle under stress streak | **Design:** [idle-under-stress-design.md](idle-under-stress-design.md) → v0.6 I1+ |
| P3 | Stronger soak fixtures; timer-resolution investigate-or-reject | **S1 Done; S2 Reject documented** |

## Anti-goals

- Overclocking, GPU boost, game-booster suites
- Claiming hardware damage prevention
- Standby purge / fake RAM %
- Cross-OS installers
- Suspend as default product path

## Related

- [hardware-control-redesign.md](hardware-control-redesign.md) (D0–D5)
- [freeze-safe-dynamic-control-design.md](freeze-safe-dynamic-control-design.md)
- [ecoqos-efficiency-design.md](ecoqos-efficiency-design.md)
- [docs/roadmap-next-release.md](../../docs/roadmap-next-release.md)
