# P2-4 false-positive evidence

Generated: 2026-07-17T21:21:32.3942218-12:00
Duration requested: 60m

| Check | Result |
|-------|--------|
| no Explorer/Cursor/whitelist/shell Suspend | PASS |
| no sticky suspend after SoftOnly restart | PASS (recovered=0) |
| status samples collected | PASS (n=718) |
| pressure observed during soak | yes |
| any Suspend seen (non-forbidden OK) | yes: NVIDIA Overlay.exe, nvcontainer.exe, disk-hog.exe |

Mode: last_resort_suspend + periodic cargo test + disk-hog pulses; Steam/Epic path whitelist.

```text
samples=718 pressure=True suspend_names=NVIDIA Overlay.exe,nvcontainer.exe,disk-hog.exe
```
