# Unstick v0.5.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.5.0`  
**Roadmap:** [roadmap-v0.5.0.md](roadmap-v0.5.0.md)  
**Design:** [hardware-control-north-star.md](../specs/backend/hardware-control-north-star.md)

---

## Why 0.5.0

Packages the **hardware-control north-star**: freeze mitigation and load/thermal **relief** with real headroom, visible sensing vs capping, and a clean (no-console) runtime — not a game booster and not hardware-damage insurance.

### Highlights

- **Freeze-safe control** — operating band **80–88%** of the calibrated cliff (was ~97–99%); fast release when load eases; soft intensity ceiling 2; soft demotion **TTL** (~45s)
- **Stress headroom** — hard latency / Disk Hard / paging / **thermal-power** shifts the band lower
- **Guard UX** — tripwires show **monitoring** vs **soft capping**; Event log uses **capped** for disk/mem actions
- **Clean runtime** — service/UI/tray are Windows GUI-subsystem (no terminal pop-ups); logs in `%LOCALAPPDATA%\Unstick\`
- **Soak fixture** — `disk_hog` defaults 1024 MiB/180s; `cliff` preset 2048 MiB/300s
- **Timer-resolution lever** — investigated and **rejected** ([timer-resolution-reject.md](../specs/backend/timer-resolution-reject.md))

### Unchanged / honesty

- Windows x64 portable only  
- SoftOnly default; Suspend remains experimental-only  
- Overload = **relief**, not damage prevention  
- No standby purge; no claimed DPC or hardware-damage “fix”  

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.5.0-windows-x64.zip`, stop service/UI, extract over install, restart.

```powershell
# or local package
pwsh -File scripts/Package-Portable.ps1
```

## Proof

- L1: `cargo test -p guardian-core -p guardian-win -p guardian-detect`  
- Control / thermal: `control::` unit tests including stress + thermal flags  
- Manual: Guard monitoring vs capping under `disk_hog` / `cliff`  

## Next

- Authenticode-signed public Latest when cert exists  
- Optional Efficiency Mode Idle under stress streak (0.5.1+ design)  
- Long L3 soak setpoint / self-overhead tuning  
