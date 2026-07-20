# Unstick — v0.8.0 roadmap (in-app updates)

**Status:** **Shipped** (unsigned portable)  
**Theme:** Discover Latest + user-initiated portable install without leaving the app  
**Parent:** [roadmap-future.md](roadmap-future.md) · **Notes:** [RELEASE-v0.8.0.md](RELEASE-v0.8.0.md) · zip `Unstick-0.8.0-windows-x64.zip`  
**Design:** [in-app-update-design.md](../specs/backend/in-app-update-design.md)  
**Living index:** [roadmap-next-release.md](roadmap-next-release.md)

```
HANDOFF ATOMIC STEP: v0.8 R5 Done — shipped 0.8.0; next Authenticode or v1.0 gates
PAUSED / CANCELLED:    Silent auto-replace; MSI/MSIX; requiring Authenticode to apply; non-GitHub mirrors
CANONICAL OWNER:       release artifacts
PROOF BEFORE DONE:     met — L1 update tests; portable zip + SHA256SUMS; GitHub Latest
```

## Goal

Operators can **see** a newer GitHub Latest and **Install** it in-app (download → SHA256 → stop → extract over install dir → restart), keeping AppData config. Manual zip path remains the fallback.

## Ship gates

| ID | Work | Done when |
|----|------|-----------|
| **R0** | Design + roadmap pointers | **Done** — [in-app-update-design.md](../specs/backend/in-app-update-design.md) |
| **R1** | Semver + GitHub parse + config/IPC check | **Done** — `CheckForUpdate`; L1 mocked JSON |
| **R2** | UI/tray update surface | **Done** — Controls Updates + tray menu + confirm |
| **R3** | Download + SHA256 + packaging sums | **Done** — `SHA256SUMS` from Package-Portable |
| **R4** | `unstick-updater.exe` apply | **Done** — allowlisted extract + restart |
| **R5** | Version 0.8.0 + RELEASE + zip + tag | **Done** — this release |

## Out of 0.8.0

| Item | Why |
|------|-----|
| Silent auto-update | Trust + portable stop/replace needs confirm |
| MSI / Store | Not required for 1.0 |
| Authenticode required to apply | Cert still blocked; hash + channel pin |
| Delta patches | Complexity without payoff at this size |

## Acceptance (ship v0.8.0)

1. Newer Latest shows a clear CTA from UI/tray.  
2. Confirming Install replaces EXEs, preserves AppData, returns on new `status.version`.  
3. Wrong hash aborts with visible error and does not replace binaries.  
4. Manual zip update still documented.
