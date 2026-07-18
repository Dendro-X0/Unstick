# Unstick v0.2.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.2.0`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.2.0

---

## Why 0.2.0

Mem Lock quality bar + packaging maturity + Guard UX polish after the v0.1.x portable beta.

### Highlights

- **Mem Lock L4** false-positive proof — Hard never latches on mapped-I/O / IDE-like load when paging evidence is absent (`Verify-MemLock-L4.ps1`)
- **IDE / Defender protection** — Cursor/Code and MsMpEng-class processes stay off the apply list (fixes Soft-trim miss after path gating)
- **Tray toasts** on Disk Lock / Mem Lock **HARD**
- **Event log** on Monitor — last actions from session / `events.jsonl`
- **Portable update story** + `Package-Portable.ps1 -Sign` scaffolding + `SIGNING.txt` honesty banner
- Softer elevated Access Denied copy (expected for AV leftovers)

### Unchanged

- SoftOnly default, Disk Lock, focus boost, Last-resort opt-in
- Sample intervals from v0.1.2 self-overhead work

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.2.0-windows-x64.zip`, stop service, extract over install, restart.

## Limits

- User-mode only; no MSI/Store yet  
- **Unsigned** binaries — SmartScreen may warn; public “production” channel waits on code signing (V2-3)  
- Darwin QoS apply remains stubbed off macOS

## Proof

- Mem Lock L4: `specs/backend/mem-lock-l4-evidence.md`  
- Packaging design: `specs/backend/v0.2-packaging-design.md`  
- Automated: `Verify-P2-Automated.ps1` / `cargo test`

## Next

- Authenticode-signed public Latest  
- Optional live Darwin QoS/App Nap apply on a macOS host  
- MSI/MSIX if Store path is chosen later  
