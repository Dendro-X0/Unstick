# DPC / ISR advisory — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P1 (DPC/ISR advisory)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-win::sensors (PDH) → guardian-core::pressure (classify only) → runtime (status + Info)
PROOF BEFORE DONE:     L1 unit tests for thresholds; L3 status shows dpc/interrupt %; never Suspend/Disk Lock from DPC alone
```

## Goal

Detect elevated Deferred Procedure Call / Interrupt time — a documented cause of audio/UI stutter ([CPU Analysis](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/cpu-analysis)) — and **advise only**. User-mode Unstick cannot shorten driver DPCs.

## Sensing (this pass)

Continuous signal via PDH (same counters PerfGuide/WPA summarize from traces):

| Counter | Field |
|---------|--------|
| `\Processor(_Total)\% DPC Time` | `dpc_time_percent` |
| `\Processor(_Total)\% Interrupt Time` | `interrupt_time_percent` |

Full ETW module attribution (ndis.sys, etc.) stays **out of scope** — point users at WPA / LatencyMon in the advisory text.

## Thresholds (MS PerfGuide)

Investigate when `% DPC Time` **or** `% Interrupt Time` **> 20%** ([privileged-mode guide](https://learn.microsoft.com/en-us/archive/blogs/perfguide/user-mode-versus-privileged-mode-processor-usage)).

| Level | Condition (streak ≥ 3 samples) | Action |
|-------|--------------------------------|--------|
| None | both &lt; 10% | clear advisory |
| Warn | either ≥ 10% or sum ≥ 15% | status advisory string |
| High | either ≥ 20% | status + rate-limited `GuardianEvent::Info` (≤1 / 5 min) |

## Hard rules

- **Do not** raise pressure band / Disk Lock / Suspend from DPC alone
- **Do not** throttle processes based on DPC
- Advisory copy must say Unstick cannot fix driver/hardware ISR latency

## Status

| Field | Type |
|-------|------|
| `dpc_time_percent` | f32 |
| `interrupt_time_percent` | f32 |
| `dpc_advisory` | `Option<String>` |

## UI

Guard chip when advisory present: amber `DPC/ISR · high` (optional short); full text in Controls or toast via Info event.

## Proof

- L1: classify_none / warn / high; scoring path unchanged when only DPC high
- L3: status.json has DPC fields after service restart
