# Unstick v0.7.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.7.0`  
**Roadmap:** [roadmap-v0.7.0.md](roadmap-v0.7.0.md)  
**Designs:** [session-actions-summary-design.md](../specs/backend/session-actions-summary-design.md) · [guard-profiles-design.md](../specs/backend/guard-profiles-design.md) · [prove-control-export-design.md](../specs/backend/prove-control-export-design.md)

---

## Why 0.7.0

**Operator clarity** after 0.6.0 Efficiency Idle: see what Soft did this session, pick Dev/Gaming/Quiet skins, read pressure/capping from the tray, and optionally export config or run a short prove soak — without dashboard sprawl or boost claims.

### Highlights

- **Session actions** — Monitor / Controls: `This session · capped N · idle M · restored K` (actuators ran/restored — **not** freezes prevented)
- **Soft restore events** — leave-plan / Soft TTL emit `soft_restore:*` Resume lines in the Event log
- **Profiles** — Dev / Gaming / Quiet Soft policy skins (`SetProfile`); additive whitelist merge; SoftOnly only
- **Tray badge** — tooltip + icon tone for monitoring / cap / Efficiency Idle; CLI `ctrl=`
- **Tools** — Export/Import config JSON under AppData; opt-in **Prove Soft (90s)** via sibling `disk-hog.exe`
- **Maximized layout** — nav tabs, footer gauges, and status chips no longer stretch into empty slabs

### Unchanged / honesty

- Windows x64 portable only  
- SoftOnly default; Suspend remains experimental-only  
- Overload = **relief**, not damage prevention  
- Launch stutter on slow OS volumes may remain under high disk latency  
- `disk-hog` is a soak fixture — not shipped in the zip by default  

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.7.0-windows-x64.zip`, stop service/UI, extract over install, restart.

```powershell
# or local package
powershell -File scripts/Package-Portable.ps1
```

## Proof

- L1: `cargo test -p guardian-core` (session counters, profiles, export/import sanitize)  
- L1: `cargo test -p guardian-tray` (control summary / icon tone)  
- L2: `cargo check -p guardian-service -p guardian-ui -p guardian-tray`  
- Manual: Monitor session line after Soft; tray icon under pressure; Export config path toast  

## Next

- Authenticode-signed public Latest when cert exists ([signing-blocker.md](signing-blocker.md))  
- v1.0 gate: signed zip + soak on ≥2 machine classes + hang-free Soft path ([roadmap-future.md](roadmap-future.md))  
