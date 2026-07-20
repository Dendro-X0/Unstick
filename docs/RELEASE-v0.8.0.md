# Unstick v0.8.0 — release notes

**Channel:** portable release (**unsigned** until Authenticode cert — private beta / local OK)  
**Surface:** Portable zip + optional HKCU autostart  
**Version:** `0.8.0`  
**Roadmap:** [roadmap-v0.8.0.md](roadmap-v0.8.0.md)  
**Design:** [in-app-update-design.md](../specs/backend/in-app-update-design.md)

---

## Why 0.8.0

**In-app updates** after 0.7 UX/ops: discover GitHub Latest and install the portable zip with user confirm — download, SHA256 verify, stop, replace EXEs via `unstick-updater`, restart — without deleting AppData config.

### Highlights

- **Check for updates** — service queries `Dendro-X0/Unstick` Latest; UI Controls + tray menu
- **Install update** — confirm dialog; requires published `SHA256SUMS` asset; wrong hash aborts
- **`unstick-updater.exe`** — allowlisted extract over install dir; restarts service/UI/tray
- **Packaging** — `Package-Portable.ps1` builds updater and writes `SHA256SUMS` beside the zip
- **Manual fallback** — USER-GUIDE still documents zip extract-over-install

### Unchanged / honesty

- Windows x64 portable only  
- SoftOnly default; Suspend remains experimental-only  
- No silent auto-replace  
- Unsigned builds warn at confirm; Authenticode remains a parallel blocker for public Latest  

## Install / update

See USER-GUIDE **Updating**. Prefer in-app Check → Install when already on 0.8+. First install of 0.8 still uses the zip from this release.

```powershell
powershell -File scripts/Package-Portable.ps1
```

## Proof

- L1: `cargo test -p guardian-core` (semver, release JSON, SHA256SUMS, allowlist)  
- L2: `cargo check -p guardian-service -p guardian-ui -p guardian-tray -p unstick-updater`  
- Package includes `unstick-updater.exe` + root `SHA256SUMS`

## Next

- Authenticode-signed public Latest when cert exists ([signing-blocker.md](signing-blocker.md))  
- v1.0 gate: signed zip + soak on ≥2 machine classes + hang-free Soft path ([roadmap-future.md](roadmap-future.md))  
