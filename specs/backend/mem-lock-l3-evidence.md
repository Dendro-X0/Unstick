# Mem Lock L3 evidence

Generated: 2026-07-18T00:04:47.9748768-12:00

| Check | Result |
|-------|--------|
| status.mem_lock soft/hard | PASS (value=soft) |
| recent_throttles mem_lock for mem-hog | PASS |
| Working Set drop on mem-hog | PASS (before=406,401,024 after=12,288) |
| no mem_lock on explorer/csrss/dwm/Cursor | PASS |

Probe config: mem_avail_soft_pct=40 (clamp max), pause_until=null, SoftOnly mode.

```json
{
    "mem_lock":  "soft",
    "mem_lock_soft_pct":  40.0,
    "mem_lock_hard_pct":  2.0,
    "stall_memory":  0.92505,
    "pressure_band":  "normal",
    "recent_throttles":  [
                             {
                                 "pid":  24516,
                                 "name":  "mem-hog.exe",
                                 "level":  "below_normal",
                                 "reason":  "mem_lock:soft"
                             }
                         ]
}
```
