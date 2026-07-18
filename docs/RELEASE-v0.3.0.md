# Unstick v0.3.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.3.0`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.3.0

---

## Why 0.3.0

Operational **smoothness**: align Soft remediation with Microsoft Efficiency Mode / EcoQoS and memory-priority guidance, cut Guard self-overhead further, and deepen honest DPC advisories—without fake “DPC cures” or standby purge.

### Highlights

- **EcoQoS / Efficiency Mode** — Soft/Warn apply `ProcessPowerThrottling` (EcoQoS) via `SetProcessInformation`, not only BelowNormal/Idle class
- **ProcessMemoryPriority LOW** on Mem Lock Soft before Hard working-set shrink
- **Hard WS shrink** only on Idle/Suspend Mem Lock ladder (Soft keeps L3 `EmptyWorkingSet` for proof)
- **Normal-band sampling** — full process enum every other tick when idle (CPU/mem/PDH every tick)
- **DPC advisory UX** — clearer “driver / WPR / WPA” copy in UI + USER-GUIDE
- Design: `specs/backend/ecoqos-efficiency-design.md`

### Unchanged

- SoftOnly default; no standby/SysMain purge; no claimed DPC fix
- Mem Lock L4 paging gate; IDE/Defender protection; tray HARD toasts; event log

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.3.0-windows-x64.zip`, stop service, extract over install, restart.

## Limits

- User-mode only; no MSI/Store yet  
- **Unsigned** binaries — SmartScreen may warn; public “production” channel waits on code signing  
- Darwin QoS apply remains stubbed off macOS

## Proof

- Mem Lock L3/L4: `specs/backend/mem-lock-l3-evidence.md`, `mem-lock-l4-evidence.md`  
- EcoQoS design: `specs/backend/ecoqos-efficiency-design.md`  
- Self-overhead: `scripts/Measure-SelfOverhead.ps1` → ~1.18% one-core idle (`specs/backend/self-overhead-measure-v0.3.0-*.csv`)  
- Automated: `cargo test` / Mem Lock verify scripts

## Next

- Authenticode-signed public Latest  
- Optional live Darwin QoS/App Nap apply on a macOS host  
- MSI/MSIX if Store path is chosen later  
