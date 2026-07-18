# Unstick v0.1.0 — release notes

**Channel:** public portable release (unsigned binaries; code signing deferred)  
**Surface:** Portable `dist/` + optional HKCU autostart  
**Version:** `0.1.0` (Cargo workspace)  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.1.0

---

## Highlights

- Soft throttle under CPU / RAM / disk pressure (**Soft only** Critical Guard by default)
- **Disk Lock** — user soft/hard OS-drive Active Time %; VeryLow I/O + working-set trim
- **Mem Lock** — trim background working sets when available RAM is scarce (safe % sliders)
- Focus-aware boost; focused app tree never throttled or paused
- **Last-resort pause** (opt-in) with durable ledger + auto-resume after crash/kill
- Whitelist for games/apps; abuse / miner heuristics (not antivirus)
- Disk latency tripwire, paging vs mapped-fault split, DPC/ISR advisory, thermal Serious suppresses Suspend

## Install (portable)

1. Download `Unstick-0.1.0-windows-x64.zip` from the GitHub release (or build: `pwsh -File scripts/Package-Portable.ps1`)
2. Run `guardian-service.exe`, then `guardian-ui.exe`
3. Optional autostart: `pwsh -File Install-Autostart.ps1 -StartNow`
4. Read `USER-GUIDE.md` in the package

## Limits

- User-mode only — cannot hard-cap IOPS on Win10/11 desktop Job Objects
- Elevated targets may show Apply denied (run elevated or whitelist)
- DPC/ISR stutter is detect-only (cannot fix bad drivers)
- No MSI/Store yet (v0.2); unsigned binaries — Windows SmartScreen may warn

## Proof snapshot

| Gate | Status |
|------|--------|
| P0 / P1 safety + ops | Done |
| P2-1 automated verify | Local script available |
| P2-2 Disk Lock L3 | Probe PASS 2026-07-17 |
| P2-3 L4 decoy | PASS 2026-07-17 |
| P2-4 false-positive | **PASS 2026-07-17** (60m coding-phase probe; gaming hour optional) |
| Mem Lock L3 | PASS |

## Verify locally

```powershell
powershell -File scripts/Verify-P2-Automated.ps1
powershell -File scripts/Verify-DiskLock-L3.ps1
powershell -File scripts/Verify-P2-L4-Decoy.ps1
powershell -File scripts/Verify-P2-4-FalsePositive.ps1
powershell -File scripts/Verify-MemLock-L3.ps1
powershell -File scripts/Package-Portable.ps1
```

## Next

- `v0.2`: Mem Lock L4, installer, code signing — see [roadmap-next-release.md](roadmap-next-release.md)
