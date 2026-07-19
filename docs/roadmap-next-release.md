# Unstick — next release roadmap

**Shipped:** `v0.1.0` … `v0.4.0`, **`v0.5.0`** (hardware-control north-star; **unsigned** portable — intended Latest until Authenticode)  
**Package:** `Unstick-0.5.0-windows-x64.zip` (local / private beta) · [RELEASE-v0.5.0.md](RELEASE-v0.5.0.md)  
**Product scope:** Windows-only OS-disk / RAM / thermal-power **hardware control** — freeze mitigation + load/thermal **relief**, not a general performance suite.  
**Design:** [hardware-control-north-star.md](../specs/backend/hardware-control-north-star.md) · [hardware-control-redesign.md](../specs/backend/hardware-control-redesign.md)

```mermaid
flowchart LR
  v03[v0.3.0 Soft_EcoQoS]
  v04[v0.4.0 Hardware_control]
  v05[v0.5.0 North_star]
  sign[Signed_public]
  v03 --> v04 --> v05 --> sign
```

---

## v0.5.0 — Hardware-control north-star (**shipped**)

**Detail:** [roadmap-v0.5.0.md](roadmap-v0.5.0.md) · **Notes:** [RELEASE-v0.5.0.md](RELEASE-v0.5.0.md)

| Phase | Work | Status |
|-------|------|--------|
| P0 | North-star design + claim language (overload = relief) | **Done** |
| P1 | Guard UX: sensing vs actively capping | **Done** |
| P2 | Thermal/power → control stress headroom | **Done** |
| Freeze-safe + soft TTL + no-console | Band 80–88%; demotion TTL; GUI subsystem | **Done** |
| S1 | Stronger `disk_hog` / cliff soak | **Done** |
| S2 | Timer-resolution investigate → reject | **Done** |
| S3 | `0.5.0` bump + RELEASE + zip | **Done** |
| S4 | Roadmap closeout / unsigned Latest intent | **Done** |
| P3 actuator | Efficiency Mode Idle under stress streak | **Deferred → 0.5.1+** |

**Channel:** unsigned portable is the **current Latest** for private beta / self-use. Public SmartScreen-clean Latest waits on Authenticode.

---

## After v0.5.0 (next)

- [x] Git tag `v0.5.0` + GitHub release asset (unsigned zip)  
- [ ] Authenticode cert → signed Latest  
- [ ] Optional Idle-under-stress Efficiency Mode (dedicated design)  
- [ ] Optional Windows MSI/MSIX  
- [ ] Long L3 soak setpoint / self-overhead tuning  

### Explicitly out

- Standby purge; kernel DPC “fixes”; other-OS installers  
- Claiming hardware-damage prevention (overload = **relief** only)  
- Overclocking / GPU boost / general PC-optimizer suite  

---

## v0.4.0

See [RELEASE-v0.4.0.md](RELEASE-v0.4.0.md). D0–D5 **Done**.

## v0.3.0

See [RELEASE-v0.3.0.md](RELEASE-v0.3.0.md).
