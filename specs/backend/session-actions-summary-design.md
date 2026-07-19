# Session actions summary — design (v0.7 U0)

```
HANDOFF ATOMIC STEP: v0.7 U0 — session actions summary design
PAUSED / CANCELLED:    New Soft actuators; hero marketing tiles; fake “freezes prevented” metrics
CANONICAL OWNER:       guardian-service (counters) → StatusSnapshot; guardian-ui Monitor / Controls
PROOF BEFORE DONE:     Spec review (this doc) → U1 impl with L1 counter tests + L2 UI check
```

## Problem

After EMERGENCY / soft capping, users see tripwire chips and a raw Event log, but not a **session-level answer** to: “Did Soft act, and did it restore?”

`recent_throttles` is tick-local. Event log lists per-PID lines. Soft restores (`restore_not_in_plan`, `expire_soft_demotions`) today log only at `debug` — **no `GuardianEvent`**, so “restored” cannot be derived from `events.jsonl` alone.

## Goal (U1 deliverable)

Expose **session aggregates** since service start (or last explicit reset — none in v0.7):

| Field | Meaning |
|-------|---------|
| `session_capped` | Soft apply successes that UI would label **capped** (disk/mem control or Disk/Mem Lock reasons) |
| `session_efficiency_idle` | Soft applies whose reason contains `efficiency_idle` (subset of capped; also counted in capped) |
| `session_restored` | Soft demotions successfully returned to Normal (left-plan or Soft TTL) |
| `session_suspended` / `session_resumed` | Experimental Suspend path only (usually 0 on Soft-only) |

Claim text (UI / USER-GUIDE): counts mean **actuators ran / restored** — **not** freezes prevented.

## Non-goals

- Second event stream or SQLite history  
- Hero chips / marketing tiles / “health score”  
- Persisting counters across service restarts (uptime-scoped is enough for trust-after-EMERGENCY)  
- Changing Soft ladder or Idle gate behavior  

## Ownership

| Concern | Owner | Notes |
|---------|--------|------|
| Increment counters | `apps/guardian-service` `GuardianState` / tick | Single writer; no UI-side recount |
| Soft restore observability | `ThrottleExecutor` return values → runtime | U1: use `expire_soft_demotions` already-returned list; make `restore_not_in_plan` return restored PIDs similarly |
| IPC | `StatusSnapshot` new `u32` fields (`#[serde(default)]`) | Backward compatible |
| Display | `guardian-ui` Monitor Event log **header** + optional one line under Controls | Not first-viewport hero |
| Tray (U3 later) | May show “capping” from mode; not required to show session counts |

## Increment rules (normative)

### `session_capped`

On each successful Soft apply that emits `GuardianEvent::Throttle` with reason prefix:

- `disk_control:`  
- `mem_control:`  
- `disk_lock:`  
- `mem_lock:`  

Increment **once per applied PID per tick** (same as today’s event emission). Do **not** count:

- Generic pressure Soft (`BelowNormal` without those prefixes) as capped — those stay Event log **throttle** only (optional separate `session_throttled` — **out of U1**).  
- Failed / denied applies (`apply_denied`).

### `session_efficiency_idle`

When capped reason contains `efficiency_idle`, also `+= 1`.

### `session_restored`

When Soft restore succeeds for a PID **not** on the suspend ledger:

- From `expire_soft_demotions` (TTL)  
- From `restore_not_in_plan` (left plan)  
- From `restore_all` when band returns to Normal / intensity 0 (count each PID restored in that call)

Do **not** double-count the same PID in one tick if both TTL and leave-plan fire (dedupe by pid set that tick).

Optional U1 polish: push `GuardianEvent::Info { message: "restored · {name} · soft_demote_ttl" }` (or reuse `Resume` with reason `soft_restore:*`) so Event log matches counters — **preferred**: `Resume` with `reason` prefixed `soft_restore:` so existing Monitor row works without a new event kind.

### Suspend path

`session_suspended` / `session_resumed` from existing `Suspend` / `Resume` events (non-`soft_restore` reasons). Soft-only product path expects zeros.

## StatusSnapshot shape (U1)

```rust
// on StatusSnapshot — all #[serde(default)]
pub session_capped: u32,
pub session_efficiency_idle: u32,
pub session_restored: u32,
pub session_suspended: u32,
pub session_resumed: u32,
```

No reset IPC in U0/U1. Counters clear when service process restarts (`service_uptime_secs` is the window label).

## UI placement (U1) — frontend contract

**Primary:** Monitor → above Event log:

```text
This session · capped 12 · idle 3 · restored 11
```

- Dim / secondary typography; one line.  
- Hover: “Soft actuators applied / Efficiency Idle / Soft restored since service start. Not a freeze-prevention score.”  
- Hide the line when all zeros **or** show `This session · no Soft actions yet` — prefer always show the line for discoverability.

**Secondary (optional same slice):** Controls strip under Hardware control readout — same string, still not hero.

**Not allowed:** new hero chip; pressure cluster badges for counts; fake “protected N times.”

## Wiring

| Source | Consumer |
|--------|----------|
| `GetStatus` / `status.json` | UI binds `session_*` |
| Existing Event log | Unchanged; soft restore lines appear if U1 emits `Resume`/`Info` |

## Proof plan

| Layer | U0 | U1 |
|-------|----|----|
| Spec | **This doc + frontend-spec section** | — |
| L1 | — | **Done** — `session_actions` unit tests |
| L2 | — | `cargo check -p guardian-ui -p guardian-service` |
| L3 | — | Not required for UX-only; optional cliff hog glance that capped/restored move |

## Implementation slices after U0

| Slice | Work |
|-------|------|
| **U1a** | Counters + restore return values + StatusSnapshot |
| **U1b** | Monitor (and optional Controls) one-liner + USER-GUIDE sentence |
| **U1c** | Soft restore → `Resume`/`Info` for Event log parity (same PR preferred) |

## Acceptance (U0 Done)

1. This design committed under `specs/backend/`.  
2. `docs/frontend-spec.md` names Monitor placement + claim hover.  
3. Roadmap U0 → **Done**; next atomic **U1**.  

## Explicitly deferred

| Item | To |
|------|-----|
| Persist counters across restarts | 0.7+ / 1.0 if requested |
| Tray shows session counts | U3 (badge = live mode, not totals) |
| Profiles | U2 |
| “Freezes averted” | Never |
