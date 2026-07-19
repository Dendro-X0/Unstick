# Freeze-safe dynamic soft control — design

```
HANDOFF ATOMIC STEP: none — user pre-test refinement (no indefinite hang; release when load eases)
PAUSED / CANCELLED:    Suspend-as-primary; parking at cliff utilization
CANONICAL OWNER:       guardian-core::control + envelope setpoints + guardian-win soft TTL
PROOF BEFORE DONE:     L1 control/envelope tests; cargo check service
```

## Problem

1. Soft demotions (EcoQoS / Idle / VeryLow I/O) can linger if intensity stays high or restore fails → feels like a hang.
2. Holding utilization at ~97–99% of the freeze cliff can still lock the desktop — headroom is too thin.
3. Cap alone is not enough: offenders must **throttle under pressure** and **fully resume** when hardware load eases.

## Contracts

### Safer operating band

| Const | Old | New | Why |
|-------|-----|-----|-----|
| `U_SET_LO` | 0.97 | **0.80** | Start releasing with real headroom |
| `U_SET_HI` | 0.99 | **0.88** | Cap below cliff; leave OS latency budget |

### Stress headroom

When `disk_latency >= hard` **or** `DiskLock Hard` **or** `memory_full`-like paging evidence while controlling, temporarily shift the band down by **0.12** (demand lower `u`) so the loop does not sit in a freeze-prone plateau.

### Release bias

- Escalation: keep 2-tick hold (anti-chatter).
- Release: **0-tick hold**; if `u < 0.70 × u_lo`, force intensity **0** (full soft release this axis).

### Soft intensity ceiling (default)

Default max intensity **2** (EcoQoS + VeryLow I/O / mem-prio+optional WS). Intensity **3** (Idle class) remains available only under stress headroom path or when `u` stays above `u_hi` after intensity 2 for a streak — optional; **v1 of this slice: hard-cap max at 2** to avoid Idle feeling like a hang.

### Soft demotion TTL

Every applied soft demotion carries `since`. After **`max_soft_demote_secs` (default 45)**, force `restore_pid` even if still in the plan. Next tick may re-apply if pressure remains — creates recovery windows; prevents indefinite demotion.

### Still never default Suspend

`experimental_suspend` unchanged.

## Out of scope this slice

- Full PI controller
- Kernel QoS
- Claiming zero hitch under Extreme pressure

## Proof

- L1: setpoint constants; release-to-zero when well below; stress band shift; soft TTL restore listing
- Manual: local soak — throttle under load, processes return to Normal when idle
