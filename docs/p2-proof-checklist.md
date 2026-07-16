# P2 Proof Checklist — v0.1 quality bar

Automated CI covers **P2-1**. Items **P2-2 … P2-4** require a human soak on target hardware (ideally the WD Green SATA boot SSD machine).

Sign off by dating each row when complete. Do not tag `v0.1.0` until all four are checked.

| ID | Claim | Automated / Manual | Sign-off |
|----|-------|--------------------|----------|
| P2-1 | `cargo test` + release build green on GitHub Actions | Automated (`.github/workflows/ci.yml`) | |
| P2-2 | Disk Lock L3: gauge ≈ Task Manager Active Time; soft/hard at user % | Manual | |
| P2-3 | L4 decoy: fake-miner / high load suspend; Explorer/Cursor/whitelist never suspended | Manual + fixture | |
| P2-4 | 2h coding (Cursor) + gaming with whitelist — no bad suspends | Manual | |

---

## P2-1 — Automated (local mirror of CI)

```powershell
powershell -File scripts/Verify-P2-Automated.ps1
```

Or:

```bash
cargo test -p guardian-core -p guardian-detect
cargo build --release -p guardian-service -p guardian-ui -p guardian-tray
cargo build --release --manifest-path fixtures/fake_miner/Cargo.toml
```

CI workflow: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)

---

## P2-2 — L3 Disk Lock soak

Setup: `guardian-service` + `guardian-ui`; Guard ARMED; Critical Guard ON; note Soft/Hard % on Guard tab.

1. [ ] Task Manager → Performance → system disk (page file drive).
2. [ ] After ~30s, Guard **DISK** gauge within ~15% of Active Time.
3. [ ] Drive load to Soft threshold → **Disk Lock SOFT · N%** (amber); recent throttles include `disk_lock:soft`.
4. [ ] Drive load to Hard threshold → **Disk Lock HARD · N%** (coral); may show suspended PIDs with `disk_lock:hard`.
5. [ ] Desktop stays usable (mouse/keyboard within ~1s during soft; hard may pause background only).
6. [ ] Kill service mid-HARD → restart → “Recovered N…” / processes resume (P0 regression).

Details: [critical-guard-soak.md](critical-guard-soak.md) § L3b

---

## P2-3 — L4 decoy / suspend safety

```powershell
cargo run --release --manifest-path fixtures/fake_miner/Cargo.toml -- stratum+tcp://example
```

1. [ ] With Critical Guard ON and pressure high (or tripwire), decoy appears in Monitor suspended list **or** Protect abuse list (score ≥70).
2. [ ] `explorer.exe` never in suspended list.
3. [ ] Cursor / whitelisted paths never in suspended list.
4. [ ] `%LOCALAPPDATA%\Unstick\events.jsonl` contains `Suspend` and later `Resume` (or recovery on restart).

Also run a normal `cargo build --release` and confirm **no** abuse alert for cargo/rustc.

---

## P2-4 — False-positive pass (2 hours)

Whitelist Steam / game folders and keep Cursor unprotected only if acceptable; otherwise Cursor is already path-protected.

1. [ ] ≥1h Cursor / VS Code development with MCP or builds running.
2. [ ] ≥1h gaming or game launcher idle + play.
3. [ ] No unexpected pause of whitelisted titles.
4. [ ] No sticky suspend after closing Guard UI (service keeps running; pause only via CTA or thresholds).

**Fail** = any whitelist or Explorer/Cursor suspend without user intent → file bug, do not ship.

---

## After all signed

1. Run `scripts/Package-Portable.ps1`
2. Fill [roadmap-v0.1.md](roadmap-v0.1.md) release notes skeleton
3. Tag `v0.1.0`
