# Mem Lock — design

```
HANDOFF ATOMIC STEP: none — greenfield from roadmap v0.2 (Mem Lock RSS trim ladder)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-core::pressure (MemLockMode) → policy → guardian-win::throttle
PROOF BEFORE DONE:     L1 unit tests for streaks + ranking; L3 soak low-RAM + whitelist never trimmed hard
```

## Goal

When **RAM pressure is real** (low available + paging evidence), shed working-set from background offenders so the focused tree stays responsive — without Disk Lock’s I/O path, and **without** standby-list / modified-page-list purge.

Disk Lock already calls `EmptyWorkingSet` as a side effect of I/O Soft/Hard. Mem Lock is the **RAM-first** twin: enter on memory stalls, rank by RSS, apply a graduated trim ladder.

## Why not “just EmptyWorkingSet everywhere”

| Concern | Rule |
|---------|------|
| Mapped-file hard faults | Soft entry may use avail % alone; **Hard** requires `paging_pressure_evidence` (same split as `ram_and_faults`) |
| Focus / whitelist | Never Mem-Lock focus tree, protected names, or whitelist |
| SoftOnly | Soft/Hard trim only — never Suspend from Mem Lock |
| LastResort | Hard may escalate to Suspend only after existing streak + thermal gates |
| Standby purge | **Forbidden** in v0.2 (guardian-design non-goal; opt-in later if ever) |

## Contracts

### Config (`GuardianConfig`)

| Field | Default | Meaning |
|-------|---------|---------|
| `mem_lock_enabled` | `true` | Master switch |
| `mem_avail_soft_pct` | `15` | Soft when available RAM **&lt;** this % of total |
| `mem_avail_hard_pct` | `8` | Hard when available **&lt;** this % (and paging evidence) |
| `mem_commit_soft_pct` | `90` | Alternate soft: commit charge ≥ this |
| `mem_commit_hard_pct` | `95` | Alternate hard: commit ≥ this **and** paging evidence |
| `mem_lock_streak` | `2` | Consecutive samples before Soft/Hard latch |
| `mem_lock_hard_requires_paging` | `true` | Hard never latches on avail alone without paging evidence |

IPC (v0.2): `SetMemSafeThresholds { soft_pct, hard_pct }` — available-% sliders (mirror Disk Lock). Commit thresholds stay config/defaults unless UI expands later.

### Status

| Field | Meaning |
|-------|---------|
| `mem_lock` | `off` / `soft` / `hard` |
| `mem_lock_soft_pct` / `mem_lock_hard_pct` | Live available-% thresholds |
| `stall_memory` / `stall_memory_full` | Existing PSI-shaped signals (unchanged) |

### `MemLockMode`

```text
off → soft → hard
```

Independent of `DiskLockMode`. Both may be active; Suspend still shared / streak-gated.

### `PlannedAction`

| Field | Meaning |
|-------|---------|
| `apply_mem_lock: bool` | Run Mem Lock apply ladder for this pid |
| `reason` | `mem_lock:soft` / `mem_lock:hard` (append `:suspend` if LastResort) |

Keep `apply_disk_lock` separate. Executor may call `EmptyWorkingSet` once if either flag is set.

## Ownership & data flow

```
WinSensor (avail, commit, Pages/sec, Page Writes, PF%)
  → PressureInputs + paging_pressure_evidence
  → update_mem_lock_streaks → MemLockMode
  → PolicyEngine (RSS-ranked offenders, focus/whitelist gates)
  → ThrottleExecutor (WS trim ladder ± Suspend)
  → StatusSnapshot.mem_lock*
```

| Step | Owner | Invariant |
|------|-------|-----------|
| Sense | `guardian-win::sensors` | Existing memory/paging PDH fields |
| Trip | `guardian-core::pressure` | Soft ≠ Emergency; Hard may contribute to hard-pressure streak |
| Plan | `guardian-core::policy` | Rank by `memory_bytes` (RSS); never focus/protected |
| Apply | `guardian-win::throttle` | No standby purge; no process kill |

## Enter / exit

### Soft hit (any)

- available% &lt; `mem_avail_soft_pct`, **or**
- commit% ≥ `mem_commit_soft_pct`, **or**
- `stall_memory` ≥ 0.75 (optional tie-break; implement if needed after L1)

After `mem_lock_streak` consecutive soft hits → `MemLockMode::Soft`.

### Hard hit (stricter)

- available% &lt; `mem_avail_hard_pct` **or** commit% ≥ `mem_commit_hard_pct`
- **and** (`!mem_lock_hard_requires_paging` **or** `paging_pressure_evidence`)

After streak → `MemLockMode::Hard` (implies Soft actions + hard ladder).

### Exit

Streak counters clear when the corresponding hit condition is false (same pattern as Disk Lock). Mode drops Soft→Off / Hard→Soft→Off as streaks decay.

### Interaction with pressure bands

| Mem Lock | Pressure effect |
|----------|-----------------|
| Soft | Does **not** force `emergency`; may plan WS trim while band is Warn/Throttle |
| Hard | Counts as hard pressure for Suspend escalation streak (like Disk Hard); tripwire label `mem_lock_hard` optional |

Existing tripwire `ram_and_faults` (avail &lt; 5% + faults + paging evidence) remains; Mem Lock Soft should fire **earlier** so freezes are prevented before Emergency.

## Action ladder

| Mode | SoftOnly / Cooperate | LastResort (after streak, not Serious thermal) |
|------|----------------------|------------------------------------------------|
| Soft | BelowNormal or Idle (QoS Utility/Background) + **EmptyWorkingSet** on top RSS offenders | same |
| Hard | Soft + `SetProcessWorkingSetSizeEx` shrink (gentle max WS) + Idle | Soft + Suspend top RSS offenders (existing Critical Guard rules) |

### Ranking

1. Exclude protected, whitelist, focus tree (same as Disk Lock / Suspend).
2. Sort by `memory_bytes` descending.
3. Cap actions (`max_actions` / `max_suspend_pids` reuse).

Prefer large idle browsers/helpers over tiny utilities. Do **not** rank by CPU alone.

### Apply details (Windows)

| Step | API | Notes |
|------|-----|-------|
| 1 | `EmptyWorkingSet` | Primary soft trim (already used by Disk Lock) |
| 2 | `SetProcessWorkingSetSizeEx` | Hard only; set max WS toward a fraction of current (e.g. ~50–70% of sampled RSS), never below a floor (~16–32 MiB) |
| 3 | Priority / QoS | Align with `plan_qos` background class |
| 4 | `NtSuspendProcess` | Hard + LastResort + streak only |

**Do not use** in v0.2:

- `EmptySystemWorkingSet` / standby-list purge helpers
- Job `JOB_OBJECT_LIMIT_PROCESS_MEMORY` as default (can OOM-kill; revisit as opt-in)

## QoS / Nap

`plan_qos` already demotes background under Emergency / Disk Hard. Mem Lock Soft → treat like Utility; Mem Lock Hard → Background. Wire `MemLockMode` into `plan_qos` the same way `DiskLockMode` is today (hard → Background QoS; Soft → Utility).

## UI (after backend verify)

| Surface | Behavior |
|---------|----------|
| Guard | Chip `Mem Lock · soft/hard` when not Off |
| Settings | Soft/Hard available-% sliders + Apply (`SetMemSafeThresholds`) |
| Copy | “Trims background working sets when RAM is scarce. Does not clear standby cache.” |

Update `docs/frontend-spec.md` in the UI slice, not this design pass.

## Out of scope (v0.2)

- Standby / modified / modified-no-write list purge
- Multi-session Remote Desktop memory partitioning
- Linux `memory.high` / cgroup apply (PSI fields already portable; apply later)
- macOS jetsam / memory-pressure notifications (beyond QoS stubs)
- Killing processes to free RAM

## Acceptance criteria

- [x] AC1 — Soft latches when avail &lt; soft% for `streak` samples without forcing Emergency
- [x] AC2 — Hard does **not** latch on commit/mapped I/O alone without paging evidence
- [x] AC3 — Focus tree + whitelist never get `apply_mem_lock` / Suspend from Mem Lock
- [x] AC4 — SoftOnly never plans Suspend from Mem Lock Hard
- [x] AC5 — Status exposes `mem_lock` + live soft/hard % (**L3 PASS** — see `mem-lock-l3-evidence.md`)
- [x] AC6 — Disk Lock Soft and Mem Lock Soft can both request WS trim without double Suspend

## Proof plan

| Criterion | Layer | Command / method |
|-----------|-------|------------------|
| AC1–AC4, AC6 | L1 | `cargo test -p guardian-core` (mem_lock streak + policy tests) — **Done** |
| Executor flags | L2 | `cargo check -p guardian-win` / service |
| AC5 + UX | L3 | `powershell -File scripts/Verify-MemLock-L3.ps1` → [`mem-lock-l3-evidence.md`](mem-lock-l3-evidence.md) — **PASS** (2026-07-17) |
| False positive | L4 | `Verify-MemLock-L4.ps1` — [mem-lock-l4-evidence.md](mem-lock-l4-evidence.md) — **PASS** (2026-07-17) |

## Implementation slices

1. **This spec** (design only)
2. `MemLockMode` + `update_mem_lock_streaks` + config/IPC/status
3. Policy RSS ranking + `apply_mem_lock`; wire `plan_qos`
4. Throttle apply (`EmptyWorkingSet` + Hard `SetProcessWorkingSetSizeEx`)
5. UI chip + safe % sliders; USER-GUIDE + frontend-spec
6. Tests + soak notes on roadmap P2/P3 as needed

## Relation to Disk Lock

```text
Disk Lock  → disk-ranked · VeryLow I/O · WS trim · optional Suspend
Mem Lock   → RSS-ranked  · WS trim ladder     · optional Suspend
```

Shared: protected set, SoftOnly, thermal Suspend suppress, ledger recovery.  
Distinct: enter conditions, offender ranking, reason strings, status chips.
