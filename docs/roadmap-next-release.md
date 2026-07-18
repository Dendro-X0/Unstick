# Unstick — next release roadmap

**Shipped:** `v0.1.0` … `v0.2.0`, **`v0.3.0`** (EcoQoS / memory priority / self-overhead; unsigned until cert)  
**Next:** Authenticode public Latest; optional Darwin apply; MSI if needed  
**Notes:** [RELEASE-v0.3.0.md](RELEASE-v0.3.0.md) · **Design:** [ecoqos-efficiency-design.md](../specs/backend/ecoqos-efficiency-design.md)

```mermaid
flowchart LR
  v02[v0.2.0 MemLock_L4]
  v03[v0.3.0 Smoothness]
  sign[Signed_public]
  v02 --> v03 --> sign
```

---

## v0.3.0 launch definition

| # | Gate | Status |
|---|------|--------|
| S1 | Soft/Warn EcoQoS apply | **Done** |
| S2 | Soft prefers ProcessMemoryPriority LOW | **Done** |
| S3 | Hard WS trim Idle/Suspend only; L3/L4 PASS | **Done** |
| S4 | Normal-band lighter process sample | **Done** |
| S5 | DPC advisory UX + WPR/WPA copy | **Done** |
| S6 | USER-GUIDE + stutter research | **Done** |
| S7 | Portable zip; unsigned honesty | **Done** |

### Explicitly out of v0.3

- Standby / SysMain purge  
- Kernel DPC “fixes”  
- MSI/Store as primary channel  
- Darwin live apply without macOS host  

---

## After v0.3.0

- [ ] Obtain Authenticode cert → `Package-Portable.ps1 -Sign` → promote signed Latest  
- [ ] Darwin QoS/App Nap apply smoke on macOS  
- [ ] Optional MSI/MSIX  
- [ ] Further self-overhead if Measure-SelfOverhead stays above ~1% one-core idle  
