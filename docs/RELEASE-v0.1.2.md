# Unstick v0.1.2 — release notes

**Channel:** public portable release (unsigned)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.1.2`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.1.2

Patch over [v0.1.1](https://github.com/Dendro-X0/Unstick/releases/tag/v0.1.1).

---

## Why 0.1.2

Lower **Unstick self-overhead** on low-end PCs (service sampling + status I/O + UI paint), without changing SoftOnly defaults or Disk/Mem Lock policy.

### Changes

- **Gated process command-line collection** — skip expensive `cmd()` for quiet processes; still collect for script hosts, hot CPU/disk, and miner-ish names (`specs/backend/self-overhead-design.md`)
- **Compact + throttled `status.json`** — compact JSON; ≤1 Hz file write on Normal band; IPC `GetStatus` still fresh every tick
- **Adaptive UI repaint** — ~1 Hz when gauges settled; ~15 Hz only while lerping or interacting (was fixed ~30 Hz)

### Proof

- L1: `cargo test` (incl. cmdline gate unit tests)
- L2: `Verify-P2-Automated.ps1`
- L3: `Measure-SelfOverhead.ps1` before/after — [self-overhead-l3-evidence.md](../specs/backend/self-overhead-l3-evidence.md)

## Unchanged

- SoftOnly default, Disk Lock, Mem Lock, focus boost, Last-resort opt-in
- Sample intervals (`sample_idle_ms` / `sample_busy_ms`)
- Portable install path

## Install

1. Download `Unstick-0.1.2-windows-x64.zip`
2. Run `guardian-service.exe`, then `guardian-ui.exe`
3. Optional: `pwsh -File Install-Autostart.ps1 -StartNow`

## Limits

Same as v0.1.x: user-mode only, unsigned binaries (SmartScreen may warn), no MSI yet.

## Next

- `v0.2`: Mem Lock L4, installer, code signing — [roadmap-next-release.md](roadmap-next-release.md)
