# P2-3 L4 decoy evidence

Generated: 2026-07-17T19:56:05.1702865-12:00

| Check | Result |
|-------|--------|
| decoy suspended OR abuse score>=70 | PASS |
| fake-miner in suspended list | PASS |
| Explorer/Cursor never suspended | PASS |

Probe: last_resort_suspend, disk hard=8%, disk-hog + fake-miner.

```json
{
    "disk_lock":  "hard",
    "pressure_band":  "emergency",
    "tripwire":  "disk_busy_hard",
    "critical_guard_mode":  "last_resort_suspend",
    "suspended":  [
                      {
                          "pid":  29720,
                          "name":  "nvcontainer.exe",
                          "reason":  "disk_lock:hard",
                          "suspended_secs":  0
                      },
                      {
                          "pid":  22784,
                          "name":  "fake-miner.exe",
                          "reason":  "disk_lock:hard",
                          "suspended_secs":  0
                      },
                      {
                          "pid":  30380,
                          "name":  "TextInputHost.exe",
                          "reason":  "disk_lock:hard",
                          "suspended_secs":  0
                      },
                      {
                          "pid":  1608,
                          "name":  "AIAssistant-313.exe",
                          "reason":  "disk_lock:hard",
                          "suspended_secs":  0
                      },
                      {
                          "pid":  15856,
                          "name":  "disk-hog.exe",
                          "reason":  "disk_lock:hard",
                          "suspended_secs":  0
                      }
                  ],
    "recent_abuse":  [

                     ]
}
```
