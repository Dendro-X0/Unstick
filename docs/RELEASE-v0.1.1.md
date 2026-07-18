# Unstick v0.1.1 — release notes

**Channel:** public portable release (unsigned)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.1.1`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.1.1

Patch over [v0.1.0](https://github.com/Dendro-X0/Unstick/releases/tag/v0.1.0).

---

## Why 0.1.1

- Ship **P2-4 false-positive soak** sign-off into the tagged release (60m LastResort probe)
- Add `scripts/Verify-P2-4-FalsePositive.ps1` + evidence
- Sync release notes / proof checklists after CI green on `main`

## Unchanged from 0.1.0

- SoftOnly default, Disk Lock, Mem Lock, focus boost, Last-resort opt-in
- Portable install path (`guardian-service` + `guardian-ui`)

## Install

1. Download `Unstick-0.1.1-windows-x64.zip`
2. Run `guardian-service.exe`, then `guardian-ui.exe`
3. Optional: `pwsh -File Install-Autostart.ps1 -StartNow`

## Limits

Same as v0.1.0: user-mode only, unsigned binaries (SmartScreen may warn), no MSI yet.

## Next

- `v0.2`: Mem Lock L4, installer, code signing — [roadmap-next-release.md](roadmap-next-release.md)
