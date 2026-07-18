# Unstick v0.4.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.4.0`  
**GitHub:** https://github.com/Dendro-X0/Unstick/releases/tag/v0.4.0

---

## Why 0.4.0

**Hardware control** exploration (Option 2): closed-loop disk/RAM capping near the freeze cliff (~97–99%), Soft-only product path, Suspend experimental-only — not a general PC optimizer and not other-OS packages.

Design: [`specs/backend/hardware-control-redesign.md`](../specs/backend/hardware-control-redesign.md)

### Highlights

- **D1** — Soft-only product path; `experimental_suspend` required for Last-resort NtSuspend; soft EcoQoS/mem-prio restore when PIDs leave the plan
- **D2** — Per-machine **envelope** from idle baselines; `status.envelope` + `envelope_profile.json`
- **D3** — Disk closed-loop (`u_disk` bang-bang → EcoQoS → VeryLow I/O → Idle); never Suspend
- **D4** — Memory closed-loop (`u_mem`); WS trim **paging-gated** (Mem Lock L4 FP safe)
- **D5** — Guard UI: Hardware control readout; Soft/Hard % under **Advanced thresholds**; **Hardware Guard** framing

### Unchanged / honesty

- Windows x64 portable only  
- SoftOnly default; no standby purge; no claimed DPC or hardware-damage “fix”  
- Soft Disk/Mem Lock tripwires remain as an advanced safety net  

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.4.0-windows-x64.zip`, stop service, extract over install, restart.

## Limits

- **Windows only** — no macOS/Linux installers  
- **Protection / hardware-control utility, not a suite**  
- User-mode only; no Windows MSI/Store yet  
- **Unsigned** binaries — SmartScreen may warn  

## Proof

- L1: `cargo test -p guardian-core -p guardian-win -p guardian-detect`  
- Envelope / control: `status.envelope`, `disk_control_*`, `mem_control_*`  
- Design phases D0–D5 marked Done in redesign + roadmap  

## Next

- Authenticode-signed public Latest when cert exists  
- Optional further tuning of setpoint deadbands / self-overhead  
