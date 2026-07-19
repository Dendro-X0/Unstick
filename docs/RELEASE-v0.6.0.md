# Unstick v0.6.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.6.0`  
**Roadmap:** [roadmap-v0.6.0.md](roadmap-v0.6.0.md)  
**Design:** [idle-under-stress-design.md](../specs/backend/idle-under-stress-design.md)  
**Evidence:** [hardware-control-l3-cliff-evidence.md](../specs/backend/hardware-control-l3-cliff-evidence.md) Run 2

---

## Why 0.6.0

**Control depth** after 0.5.0: when Soft intensity 2 is still sitting on a sustained OS-disk/RAM cliff, Unstick may deepen to **Efficiency Idle** (Windows Idle + EcoQoS) — Soft-only, streak-gated, TTL-restored. Not Suspend. Not a zero-stutter guarantee on slow OS volumes.

### Highlights

- **Efficiency Idle (intensity 3)** — after sustained cliff at i2 (`idle_escalate_streak`, default 4 ticks); reasons `disk_control:efficiency_idle` / `mem_control:efficiency_idle`
- **Config** — `idle_under_stress_enabled` (default true), `idle_escalate_streak`, `idle_release_streak` in `%LOCALAPPDATA%\Unstick\config.json`
- **Soft restore** — Idle demotions still auto-restore via Soft TTL (~45s) and release when utilization eases
- **Guard UX** — tripwire **efficiency idle · disk i3**; chips **Disk idle** / **RAM idle**; hover explains Efficiency Mode
- **L3 proof** — WD Green cliff Run 2: i3 ~9s; released ~39s after hog stop; no hang FP; launch stutter honesty unchanged

### Unchanged / honesty

- Windows x64 portable only  
- SoftOnly default; Suspend remains experimental-only  
- Overload = **relief**, not damage prevention  
- On saturated slow OS SSDs (~0.7–1 s response), **new-process / screenshot hitch may remain**  
- No standby purge; no timer-resolution lever; no claimed DPC or hardware-damage “fix”

## Install / update

See USER-GUIDE **Updating (portable)**. Download `Unstick-0.6.0-windows-x64.zip`, stop service/UI, extract over install, restart.

```powershell
# or local package
pwsh -File scripts/Package-Portable.ps1
```

## Proof

- L1: `cargo test -p guardian-core` (Idle gate / plan / config)  
- L3: [cliff evidence Run 2](../specs/backend/hardware-control-l3-cliff-evidence.md) — WD Green Efficiency Idle + restore  
- Manual: Guard **efficiency idle** under `disk_hog cliff`

## Next

- Authenticode-signed public Latest when cert exists ([signing-blocker.md](signing-blocker.md))  
- v0.7 UX/ops — **shipped** [RELEASE-v0.7.0.md](RELEASE-v0.7.0.md)  
