# Self-overhead design (v0.1.2)

**Status:** implement  
**Pairs with:** [self-overhead-investigation.md](self-overhead-investigation.md)  
**Version target:** `0.1.2`

## Invariants

1. SoftOnly / Critical Guard defaults unchanged.  
2. Full process enumeration remains (Mem/Disk Lock need RSS + per-proc I/O).  
3. `path` / `name`: `name` every tick; `path` gated (warm CPU ≥8% or cmdline gate). Whitelist by name still works for cold processes.  
4. Abuse detect still sees `cmd_line` when heuristics need it (script hosts, hot/top offenders, miner-ish names).  
5. IPC `GetStatus` always returns the latest in-memory snapshot every tick.  
6. `status.json` remains valid JSON for UI file fallback; schema unchanged (compact encoding OK).  
7. Default `sample_idle_ms` / `sample_busy_ms` unchanged.

## Change 1 — Gated cmdline collection

**Owner:** `crates/guardian-win/src/sensors.rs`

For each process after computing name/cpu/io:

```text
need_cmd =
  name is script host (powershell, wscript, cscript, mshta)
  OR cpu_percent >= 50
  OR (disk_read + disk_write)_Bps >= 200_000
  OR name lower contains miner-ish token (xmrig, minerd, cpuminer, nicehash, monero)

need_path = need_cmd OR cpu_percent >= 8
```

Only then call `proc.cmd()` / `proc.exe()`. Otherwise `cmd_line` / `path` = None.

**Detect impact:** Encoded PowerShell still covered (script host always). Stratum-only args on a cold low-CPU process may miss until it heats — acceptable for v0.1.2; decoy L4 uses hot CPU + name tokens. Path-based allow/suspicious heuristics apply once a process is warm (≥8% CPU) or otherwise gated.

**Tests:** Unit tests for `need_cmdline` in `guardian-win`.

## Change 2 — Compact + throttled `status.json`

**Owner:** `apps/guardian-service/src/runtime.rs`

- Serialize with `serde_json::to_string` (not `to_string_pretty`).  
- Track `last_status_write: Option<Instant>` on `ServiceInner`.  
- Write file when:
  - never written yet, OR
  - band is Warn | Throttle | Emergency (every tick), OR
  - Normal and `last_status_write` elapsed ≥ 1000 ms.  
- Always assign `g.last_status = Some(status)` every tick.

## Change 3 — Adaptive UI repaint

**Owner:** `apps/guardian-ui/src/app.rs`

After gauge lerps:

```text
settled = abs(disp - target) < 0.005 for cpu/ram/disk/pressure
         AND abs(lit - target_lit) < 0.05
if interacting (pointer any down / key) OR not settled OR toast present:
  request_repaint_after(66ms)   # ~15 Hz lerp burst
else:
  request_repaint_after(1000ms) # match status cadence
```

Keep IPC poll at ~900 ms. Pulse animation only needs frames during burst; when settled, static is fine.

## Proof plan

| Layer | Command |
|-------|---------|
| L1 | `cargo test -p guardian-core -p guardian-detect` (+ win if tests added) |
| L2 | `powershell -File scripts/Verify-P2-Automated.ps1` |
| L3 | `Measure-SelfOverhead.ps1 -Label after` vs before; evidence in `self-overhead-l3-evidence.md` |

## Non-goals

Changing sample intervals, IPC schema shrink, tray paint changes, skipping full process enumeration.
