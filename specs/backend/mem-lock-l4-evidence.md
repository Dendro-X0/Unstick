# Mem Lock L4 evidence

Generated: 2026-07-18T00:10:23.0644556-12:00

| Check | Result |
|-------|--------|
| L4-4 unit `mem_hard_requires_paging` | PASS (ran at probe start) |
| Samples | 146 over 5m |
| Soft seen (optional) | yes |
| Hard latch / mem_lock:hard throttle | PASS (never) |

Probe config: mem_avail_soft/hard_pct=99, mem_lock_hard_requires_paging=true, SoftOnly.
Workload: mapped-io-hog 256 MiB re-touch + cargo check pulses.

## Bad events

```
(none)
```

Overall: PASS