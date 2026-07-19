# Unstick — v0.5.1 roadmap (Trust & proof)

**Status:** In progress  
**Theme:** Evidence and trust for unsigned Latest — not new actuators  
**Parent:** [roadmap-future.md](roadmap-future.md) · **Shipped base:** [RELEASE-v0.5.0.md](RELEASE-v0.5.0.md)

```
HANDOFF ATOMIC STEP: v0.5.1 — trust & proof (soak + overhead + signing path)
PAUSED / CANCELLED:    New Soft actuators (Idle streak); MSI; Suspend-as-default
CANONICAL OWNER:       docs/soak + Measure-SelfOverhead + Package-Portable -Sign
PROOF BEFORE DONE:     L3 cliff evidence file; overhead CSV+notes; signed zip OR documented cert blocker
```

## Goal

Make **v0.5.0** safer to recommend: prove soft capping helps under sustained OS-volume pressure, bound Guard self-overhead, and unlock signed Latest when a cert exists.

## Ship gates

| ID | Work | Done when |
|----|------|-----------|
| **T1** | Band roadmap + future ladder linked | **Done** |
| **T2** | L3 cliff soak evidence template + at least one filled run | **Done** — Run 1 WD Green (soft capping i2; launch stutter under ~948 ms latency) |
| **T3** | Repo description/topics set on GitHub | **Done** |
| **T4** | Self-overhead remeasure on 0.5.0 binaries | **Done** — idle ~1.5% one-core (see CSV) |
| **T5** | Authenticode: sign zip **or** document blocker | **Blocker doc** — [signing-blocker.md](signing-blocker.md) |

## Out of 0.5.1

- Efficiency Mode Idle under stress (→ 0.6 design)  
- MSI/MSIX  
- Setpoint changes without soak data  

## Suggested order

1. T1 (docs) → T3 (repo polish)  
2. T4 (overhead, needs running service)  
3. T2 (manual cliff soak — human + Guard LIVE)  
4. T5 (cert-dependent)  
