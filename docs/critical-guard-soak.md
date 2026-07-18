# Critical Guard soak checklist (L3 / L4)

## L3 — Freeze prevention soak

1. Start `guardian-service`, then `guardian-ui`.
2. Confirm **Critical Guard** checkbox is ON and “0 suspended”.
3. Run a heavy workload: `cargo build --release` in a large crate, plus Node/MCP fan-out if available.
4. Watch pressure band; under high RAM/disk it may enter `throttle` / `emergency`.
5. Desktop must stay interactive (move windows / type within ~1s).
6. If emergency fires, Monitor shows suspended PIDs; they resume when pressure drops or after `max_suspend_secs` (45s).
7. Toggle Critical Guard OFF → all suspended processes resume immediately.
8. Pause 15m from CTA → no new throttles/suspends.

## L3b — Disk Lock soak (OS drive saturation)

Automated probe (low soft busy% + disk load):

```powershell
powershell -File scripts/Verify-DiskLock-L3.ps1
```

1. Open Task Manager → Performance → Disk 1 (system / page file drive).
2. Confirm Guard **DISK** gauge tracks Active Time within ~15% once PDH is primed.
3. Under sustained disk saturation, Guard shows **Disk Lock SOFT · N%** (amber) using **this machine's calibrated** soft threshold.
4. Under harder saturation, shows **Disk Lock HARD · N%** (coral) and may suspend top disk offenders.
5. Whitelisted / Explorer / Cursor never appear in suspended list.
6. Recent throttles show reasons `disk_lock:soft` / `disk_lock:hard`.
7. After ~40 samples, `disk_calibrated` is true in status; profile persists under `%LOCALAPPDATA%\Unstick\disk_profile.json`.

## L3c — Mem Lock soak (RAM pressure / WS trim)

Automated probe (raises soft available-% temporarily so Soft latches without thrashing):

```powershell
powershell -File scripts/Verify-MemLock-L3.ps1
```

Evidence written to `specs/backend/mem-lock-l3-evidence.md`.

Manual (optional, real low-RAM):

1. Start `guardian-service` + `guardian-ui`; Soft only; note Mem soft/hard available %.
2. Run `cargo run --release --manifest-path fixtures/mem_hog/Cargo.toml -- 1024` (or open many apps).
3. When available RAM drops under Soft %, Guard shows **Mem Lock SOFT · N%**; Monitor recent throttles include `mem_lock:soft` on large RSS offenders (not focus / whitelist / Explorer).
4. Task Manager → Details → Working Set for the offender drops after Soft apply.
5. Hard requires paging evidence — mapped-file heavy IDE work alone should not show **Mem Lock HARD**.

## L4 — Decoy suspend

```bash
cargo run --release --manifest-path fixtures/fake_miner/Cargo.toml -- stratum+tcp://example
```

1. Raise system pressure (optional: open many apps) or wait for high CPU decoy.
2. When tripwire or emergency band hits, decoy should appear in suspended list (if Critical Guard ON).
3. Confirm `explorer.exe` / Cursor never appear in suspended list.
4. Events in `%LOCALAPPDATA%\Unstick\events.jsonl` include `Suspend` / `Resume`.

## Automated

```bash
cargo test -p guardian-core -p guardian-detect
```
