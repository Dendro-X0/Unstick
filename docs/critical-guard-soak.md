# Critical Guard soak checklist (L3 / L4)

## L3 — Freeze prevention soak

1. Start `guardian-service`, then `guardian-ui`.
2. Confirm **Hardware Guard** checkbox is ON and Soft-only path (0 suspended expected).
3. Run a heavy workload: `cargo build --release` in a large crate, plus Node/MCP fan-out if available.
4. Watch pressure band; under high RAM/disk it may enter `throttle` / `emergency`.
5. Desktop must stay interactive (move windows / type within ~1s).
6. Distinguish **sensing** (EMERGENCY / tripwire · monitoring) from **capping** (Disk/RAM cap chips or tripwire · soft capping). Brief spikes often sense without capping.
7. If emergency fires with experimental Suspend, Monitor shows suspended PIDs; they resume when pressure drops or after `max_suspend_secs` (45s). Soft path: Event log shows **capped** / restore instead.
8. Toggle Hardware Guard OFF → soft demotions restore; any suspended processes resume.
9. Pause 15m from CTA → no new throttles/suspends.
10. **Thermal note (optional):** on battery or when Windows reports Passive cooling / thermal Fair|Serious, control band shifts lower (more headroom). Confirm “Thermal · fair/serious” chip and earlier capping under the same `u` than on AC/Nominal.

## L3b — Disk Lock soak (OS drive saturation)

Automated probe (low soft busy% + disk load):

```powershell
powershell -File scripts/Verify-DiskLock-L3.ps1
```

Manual / freeze-cliff soak (`disk_hog` is a **fixture**, not a product feature):

```bash
# Default soak: 1024 MiB × ~3 min
cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml

# Cliff preset: 2048 MiB × ~5 min (sustained OS-volume pressure)
cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml -- cliff

# Custom: MiB secs (MiB 64–8192, secs 15–1800)
cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml -- 1536 240
```

Ensure `%TEMP%` (or the hog path printed) lives on the **OS / pagefile volume**. Pair with `mem_hog` if you need dual-axis pressure.

1. Open Task Manager → Performance → Disk 1 (system / page file drive).
2. Confirm Guard **DISK** gauge tracks Active Time within ~15% once PDH is primed.
3. Under sustained disk saturation, Guard shows **Disk Lock SOFT · N%** (amber) using **this machine's calibrated** soft threshold.
4. Under harder saturation, shows **Disk Lock HARD · N%** (coral); Soft-only path shows **soft capping** / Event log **capped** (not Suspend).
5. Distinguish tripwire **monitoring (u below band)** vs **soft capping · disk iN** — brief spikes often only monitor.
6. Whitelisted / Explorer / Cursor never appear in suspended list.
7. After ~40 samples, `disk_calibrated` is true in status; profile persists under `%LOCALAPPDATA%\Unstick\disk_profile.json`.
8. Stop hog → control mode returns toward **released**; soft demotions restore (or TTL ~45s).

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

## L4b — False-positive soak (P2-4)

```powershell
powershell -File scripts/Verify-P2-4-FalsePositive.ps1 -Minutes 60
```

LastResort under periodic `cargo test` + disk-hog pulses. Fails if Explorer / Cursor / Steam whitelist / shells are suspended or Cursor/Explorer are soft-throttled. Ends with SoftOnly restart sticky check.

Optional human gaming hour still recommended before a non-prerelease public tag.

## Automated

```bash
cargo test -p guardian-core -p guardian-detect
```
