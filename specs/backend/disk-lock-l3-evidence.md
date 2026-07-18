# Disk Lock L3 evidence (P2-2 probe)

Generated: 2026-07-17T19:55:12.4671581-12:00

| Check | Result |
|-------|--------|
| disk_busy observed >= 5% | PASS (busy=99.9) |
| status.disk_lock soft/hard | PASS (value=hard) |
| recent_throttles disk_lock:* | PASS |
| no Explorer/Cursor suspend | PASS |

Probe: disk_busy_soft_pct=5, adaptive=false, SoftOnly, pause_until=null.

```json
{
    "disk_lock":  "hard",
    "disk_lock_soft_pct":  50.0,
    "disk_busy_percent":  99.94104,
    "disk_queue_length":  1.8686318,
    "disk_latency_sec":  0.11118552,
    "disk_calibrated":  false,
    "pressure_band":  "emergency",
    "recent_throttles":  [
                             {
                                 "pid":  12692,
                                 "name":  "NVIDIA Overlay.exe",
                                 "level":  "idle",
                                 "reason":  "disk_lock:hard"
                             },
                             {
                                 "pid":  21224,
                                 "name":  "svchost.exe",
                                 "level":  "idle",
                                 "reason":  "disk_lock:hard"
                             },
                             {
                                 "pid":  4824,
                                 "name":  "svchost.exe",
                                 "level":  "idle",
                                 "reason":  "disk_lock:hard"
                             },
                             {
                                 "pid":  15152,
                                 "name":  "SearchHost.exe",
                                 "level":  "idle",
                                 "reason":  "disk_lock:hard"
                             }
                         ],
    "suspended":  [

                  ]
}
```
