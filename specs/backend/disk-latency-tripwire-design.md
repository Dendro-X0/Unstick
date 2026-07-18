# Disk latency tripwire — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P0 item (1)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-win::sensors (PDH) → guardian-core::pressure / disk_calibrate
PROOF BEFORE DONE:     L1 cargo test -p guardian-core -p guardian-win; L3 status.json shows disk_latency_sec
```

## Goal

Sense **Avg. Disk sec/Transfer** on the system volume’s PhysicalDisk and use it alongside busy%/queue so Disk Lock and pressure reflect **service time**, not only utilization. Official MS guidance: pair hard faults / disk load with transfer latency; queue alone is weaker.

## Contracts

### Sample / status

| Field | Type | Meaning |
|-------|------|---------|
| `disk_latency_sec` | f32 | PDH `PhysicalDisk(...)\Avg. Disk sec/Transfer` (seconds per IRP) |

Fallback when PDH unavailable: `0.0` (do not invent latency from estimate).

### Config defaults

| Field | Default | Role |
|-------|---------|------|
| `disk_latency_soft_sec` | `0.015` (15 ms) | Soft Disk Lock / pressure contribution |
| `disk_latency_hard_sec` | `0.040` (40 ms) | Hard Disk Lock / emergency tripwire path |
| Reuse `disk_busy_streak` | 2 | Consecutive samples required |

### Pressure / Disk Lock

- `disk_pressure`: `max(busy, queue_norm, latency_norm)` where latency_norm = clamp(latency / soft_latency)
- Soft hit: existing OR `latency >= soft` for streak
- Hard hit: existing OR `latency >= hard` for streak  
- Tripwire name when hard from latency: `disk_latency_hard` (distinct from `disk_busy_hard`)

### UI this pass

Status field only (Monitor/gauges can show later). No new Controls slider unless needed.

## Proof

- L1: unit — soft latency alone can enter Disk Soft after streak; hard latency forces Emergency tripwire; busy-only path unchanged
- L3: running service status.json includes `disk_latency_sec` (typically &lt;0.005 idle SSD)
