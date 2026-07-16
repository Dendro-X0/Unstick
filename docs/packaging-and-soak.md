# Packaging & operations

## Build artifacts

```bash
cargo build --release -p guardian-service -p guardian-tray -p guardian-ui
```

Outputs:

- `target/release/guardian-service.exe`
- `target/release/guardian-ui.exe` — primary polished client
- `target/release/guardian-tray.exe` — optional tray/CLI

Portable layout:

```text
dist/
  guardian-service.exe
  guardian-ui.exe
  guardian-tray.exe
  README.txt
```

Copy the two release binaries into `dist/` (see `scripts/package-portable.sh`).

## Autostart

PowerShell (recommended on Windows):

```powershell
pwsh -File scripts/Install-Autostart.ps1 -StartNow          # service only; UI on demand
pwsh -File scripts/Install-Autostart.ps1 -Tray -StartNow    # + tray icon
pwsh -File scripts/Uninstall-Autostart.ps1 -StopProcesses
```

Legacy bash wrapper: `scripts/install-autostart.sh` (calls the same Run-key idea).

Elevation is **not** required for v1 user-session protection. Cross-session / service-account jobs need an elevated helper (not shipped in v1).

## Logging

Service writes rotating daily logs to:

`%LOCALAPPDATA%\Unstick\guardian.log` (plus dated roll files)

Override level with `RUST_LOG=debug`.

## Package

```powershell
pwsh -File scripts/Package-Portable.ps1
```

Copies release exes + USER-GUIDE + install scripts into `dist/`.

## P2 proof sign-off

Consolidated quality bar (CI + L3/L4/false-positive): [p2-proof-checklist.md](p2-proof-checklist.md)

Local mirror of CI: `powershell -File scripts/Verify-P2-Automated.ps1`

## Soak-test checklist (L3)

1. Start `guardian-service`, then `guardian-ui`.
2. Confirm bottom gauges (CPU / RAM / Disk / Pressure) update and LIVE indicator is on.
3. On Guard tab, click the circular CTA to pause 15m; confirm PAUSED state.
4. Run `cargo build --release` in a large crate while using the desktop (move windows, type).
5. Confirm band may rise to `warn`/`throttle` and Monitor shows `cargo`/`rustc`.
6. Confirm desktop stays interactive (mouse/keyboard respond within ~1s).
7. Resume from CTA; confirm sampling continues.

## Abuse fixture (L4)

```bash
cargo run --manifest-path fixtures/fake_miner/Cargo.toml -- stratum+tcp://example
```

Expect an abuse event in status / `events.jsonl` within ~a few samples once CPU is high (miner token scores immediately; sustained-CPU points accrue after 120s).

Real `cargo build` must **not** produce abuse alerts (toolchain allowlist).

## Failure modes (no kernel driver)

- Processes that ignore thread priority can still saturate cores.
- I/O priority is best-effort; malicious/high-IRP drivers can still stall the system.
- Full-disk + pagefile exhaustion can freeze before user-mode actions apply.
- Protected / elevated processes may refuse priority changes without admin rights (UI shows elevation warning).
- Heuristics are not antivirus — packed/custom miners can evade name/arg tokens.

## P0 safety (crash recovery)

- Suspended PIDs are written to `%LOCALAPPDATA%\Unstick\suspend_ledger.json`.
- On `guardian-service` start, any leftover entries are **resumed** then the ledger is cleared.
- Clean exit / Drop also resumes and clears the ledger.
- Soft Disk Lock requires `disk_busy_streak` consecutive samples (default 2); Hard pause uses the same streak + Critical Guard.
- Max pause duration: `max_suspend_secs` (default 45).

### Manual proof (P0-1)

1. Trigger Disk Lock HARD / Critical Guard so Monitor shows suspended PIDs.
2. Kill `guardian-service.exe` from Task Manager (do not Pause Guard first).
3. Restart `guardian-service.exe` — processes should resume; UI may show “Recovered N paused process(es)”.
