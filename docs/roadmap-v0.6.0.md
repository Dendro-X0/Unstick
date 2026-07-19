# Unstick — v0.6.0 roadmap (Control depth)

**Status:** **Shipped** (unsigned portable)  
**Theme:** Idle-under-stress Efficiency Mode (Soft deepen)  
**Design:** [idle-under-stress-design.md](../specs/backend/idle-under-stress-design.md)  
**Notes:** [RELEASE-v0.6.0.md](RELEASE-v0.6.0.md) · zip `Unstick-0.6.0-windows-x64.zip`  
**Evidence:** [hardware-control-l3-cliff-evidence.md](../specs/backend/hardware-control-l3-cliff-evidence.md) Run 2  
**Parent:** [roadmap-future.md](roadmap-future.md)

```
HANDOFF ATOMIC STEP: v0.6 I5 Done — shipped 0.6.0; next v0.7 UX/ops or Authenticode when cert
PAUSED / CANCELLED:    Suspend-as-default; timer-res; Idle without streak/TTL; zero-stutter claims
```

## Slices

| ID | Work | Status |
|----|------|--------|
| **I0** | Design spec | **Done** |
| **I1** | Control loop Idle gate + plan intensity 3 | **Done** — L1 gate/plan tests |
| **I2** | Config + runtime wire | **Done** — `idle_under_stress_*` in config.json; status intensity 0..=3 |
| **I3** | Guard UX + USER-GUIDE | **Done** — efficiency idle chips + hover; USER-GUIDE Idle section |
| **I4** | WD Green L3 re-soak (Run 2) | **Done** — i3 + efficiency_idle ~9s; released ~39s after stop; no hang FP |
| **I5** | Version bump 0.6.0 + RELEASE + zip | **Done** — `Unstick-0.6.0-windows-x64.zip` |

## Out

PI rewrite, MSI, Authenticode (parallel 0.5.1 T5), claiming zero launch stutter.
