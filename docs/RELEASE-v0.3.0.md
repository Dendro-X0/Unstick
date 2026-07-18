# Unstick v0.3.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.3.0`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.3.0

---

## Why 0.3.0

Better **disk/RAM protection** under pressure: Soft remediation follows Microsoft Efficiency Mode / EcoQoS and memory-priority guidance, Guard self-overhead drops further, DPC advisories stay honest — without fake “DPC cures,” standby purge, or “PC optimizer” scope creep.

### Highlights

- **EcoQoS / Efficiency Mode** — Soft/Warn apply `ProcessPowerThrottling` so background work yields when Disk/Mem Lock need headroom
- **ProcessMemoryPriority LOW** on Mem Lock Soft before Hard working-set shrink
- **Hard WS shrink** only on Idle/Suspend Mem Lock ladder (Soft keeps L3 `EmptyWorkingSet` for proof)
- **Normal-band sampling** — full process enum every other tick when idle (CPU/mem/PDH every tick)
- **DPC advisory UX** — clearer “driver / WPR / WPA” copy in UI + USER-GUIDE
- Design: `specs/backend/ecoqos-efficiency-design.md`

### Unchanged

- SoftOnly default; Disk Lock + Mem Lock as the core job; no standby/SysMain purge; no claimed DPC fix
- Mem Lock L4 paging gate; IDE/Defender protection; tray HARD toasts; event log

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.3.0-windows-x64.zip`, stop service, extract over install, restart.

## Limits

- **Windows only** — portable Guard for Windows x64; no macOS/Linux installers or packages  
- **Protection utility, not a suite** — Disk Lock (SSD/HDD) + Mem Lock (RAM); not a general performance optimizer  
- User-mode only; no Windows MSI/Store yet  
- **Unsigned** binaries — SmartScreen may warn; public “production” channel waits on code signing  

## Proof

- Mem Lock L3/L4: `specs/backend/mem-lock-l3-evidence.md`, `mem-lock-l4-evidence.md`  
- EcoQoS design: `specs/backend/ecoqos-efficiency-design.md`  
- Self-overhead: `scripts/Measure-SelfOverhead.ps1` → ~1.18% one-core idle (`specs/backend/self-overhead-measure-v0.3.0-*.csv`)  
- Automated: `cargo test` / Mem Lock verify scripts

## Next

- Authenticode-signed public Latest (still Windows portable protection utility)  
- Optional Windows MSI/MSIX if a Store path is chosen later  

