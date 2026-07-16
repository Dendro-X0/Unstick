# Unstick — v0.1 release roadmap

**Goal:** Ship a **production-ready portable v0.1** for Windows low-end PCs: freeze prevention that is safe by default, recoverable after crash, installable without a full MSI, and honest about limits.

**Version:** `0.1.0` (Cargo workspace)  
**Surface:** Portable `dist/` + optional HKCU autostart (MSI deferred to v0.2)

---

## Release definition (what “done” means)

v0.1 is production-ready when:

1. A non-developer can start Guard, set safe disk %, whitelist games, and leave it running.
2. A service crash/kill **cannot** leave processes permanently suspended.
3. Soak on DRAM-less SATA boot SSD shows fewer multi-minute freezes under build/MCP load.
4. CI builds + unit tests are green on every push.
5. Release notes list known limits (no kernel IOPS cap, no Mem Lock yet, heuristics ≠ AV).

---

## Current baseline (already in tree)

| Pillar | Status |
|--------|--------|
| Pressure bands + tripwires | Done |
| Soft throttle + job CPU caps | Done |
| Critical Guard (NtSuspend + ledger) | Done |
| Disk Lock (PDH + adaptive queue + user soft/hard %) | Done |
| Whitelist | Done |
| Abuse heuristics + Protect UI | Done |
| Portable package + soak docs | Done (P2 manual soak pending) |

---

## Phased roadmap

```mermaid
flowchart LR
  P0[P0 Safety]
  P1[P1 Ops]
  P2[P2 Proof]
  P3[P3 Polish]
  Ship[v0.1 tag]
  P0 --> P1 --> P2 --> P3 --> Ship
```

### P0 — Safety (blocking)

| ID | Work | Owner | Proof | Status |
|----|------|-------|-------|--------|
| P0-1 | **Suspend recovery on start** — persist ledger; resume orphans if service died dirty | `guardian-win` / service | Kill service mid-suspend → restart → processes resume | **Done** |
| P0-2 | **Watchdog / max-suspend** — startup resume-all for stale ledger; `max_suspend_secs` documented | service | Unit + soak docs | **Done** |
| P0-3 | **Safe defaults** — streak before hard; UI safety banner when paused | UI + config | Copy review | **Done** |
| P0-4 | **Elevation honesty** — `apply_denied` in status + amber UI warning | service / UI | Elevated target | **Done** |

### P1 — Ops (shipping)

| ID | Work | Owner | Proof | Status |
|----|------|-------|-------|--------|
| P1-1 | **PowerShell package + autostart + uninstall** (`scripts/*.ps1`) | scripts | Clean install/remove on Win11 | **Done** |
| P1-2 | **Autostart service + optional tray**; UI launched on demand | scripts / tray | Reboot → service up | **Done** |
| P1-3 | **File logging** — rotate `guardian.log` under AppData; default `info` | service | Survive overnight | **Done** |
| P1-4 | **Version in UI + status** — show `0.1.0` | UI / IPC | Visible | **Done** |
| P1-5 | **End-user README** — start/stop, whitelist, safe disk %, risks of pause | `docs/USER-GUIDE.md` | Peer read | **Done** |

### P2 — Proof (quality bar)

Master checklist: [p2-proof-checklist.md](p2-proof-checklist.md) · local gate: `scripts/Verify-P2-Automated.ps1`

| ID | Work | Owner | Proof | Status |
|----|------|-------|-------|--------|
| P2-1 | **GitHub Actions** — `cargo test -p guardian-core -p guardian-detect` + release build + fixture + package | `.github/workflows/ci.yml` | Green CI / local verify script | **Done** (artifact; sign CI green on first push) |
| P2-2 | **L3 Disk Lock soak** — Task Manager Active Time vs gauge; soft/hard at user % | soak checklist | Signed off | **Ready** (manual) |
| P2-3 | **L4** — fake-miner / decoy suspend; Explorer/Cursor/whitelist never suspended | `fixtures/fake_miner` | Signed off | **Ready** (manual; fixture builds) |
| P2-4 | **False-positive pass** — gaming + Cursor coding 2h with whitelist | manual | No bad suspend | **Ready** (manual) |

### P3 — Polish (v0.1.x allowed to slip small items)

| ID | Work | Notes |
|----|------|-------|
| P3-1 | Tray balloon on Disk Lock HARD | Nice-to-have |
| P3-2 | Event viewer in UI (last N events) | Nice-to-have |
| P3-3 | Code signing | Strongly recommended before public download; can ship unsigned private beta |

---

## Explicitly out of v0.1 (v0.2+)

| Item | Why deferred |
|------|----------------|
| **Mem Lock** (RSS trim ladder) | Large feature; document as next after v0.1 |
| MSI / MSIX / Store | Portable + PS1 uninstall is enough for first cohort |
| Windows SCM service | Session exe + autostart is simpler |
| Kernel / minifilter I/O QoS | Non-goal |
| Standby-list purge | Dangerous; opt-in later |
| Multi-volume Disk Lock (games drive) | System disk only for freeze root cause |

---

## Suggested milestone order (calendar-agnostic)

1. **Week A — P0** safety/recovery (no ship without this)  
2. **Week B — P1** scripts, log, user guide, version string  
3. **Week C — P2** CI + soak on the WD Green boot SSD machine  
4. **Tag `v0.1.0`** portable zip + release notes  
5. **v0.1.1** hotfixes from soak only  
6. **v0.2** Mem Lock + installer

---

## Release notes skeleton (fill at tag)

```text
Unstick v0.1.0
- Soft throttle under CPU/RAM/disk pressure
- Disk Lock with user safe soft/hard Active Time %
- Critical Guard process pause with auto-resume
- Whitelist for games/apps
- Abuse/miner heuristics (not antivirus)

Limits:
- User-mode only; cannot hard-cap IOPS on Win10/11 desktop
- May not control elevated processes without admin
- Crash recovery: ledger resume on restart (P0-1)
- Mem Lock not included (see v0.2)
```

---

## Decision log

| Decision | Choice |
|----------|--------|
| Installer for v0.1 | Portable + PowerShell autostart/uninstall |
| Mem Lock | Defer to v0.2 |
| Hard ship gate | P0 suspend recovery + P2 soak on target hardware |
| Adaptive disk | User busy% authoritative; adaptive queue only |
