# Mem Lock L4 false-positive checklist

**Gate:** V2-1 / M1 — [roadmap-next-release.md](../../docs/roadmap-next-release.md)  
**Design:** [mem-lock-design.md](mem-lock-design.md) § Proof L4  
**Probe:** `powershell -ExecutionPolicy Bypass -File scripts/Verify-MemLock-L4.ps1`

## Claim

Under **mapped-I/O / IDE-like** memory activity (large resident buffers + build pulses), Mem Lock must **not** latch **Hard** while `mem_lock_hard_requires_paging` is true and the pagefile is quiet. Soft may appear if available-% is forced for the probe; Hard is the failure.

## Matrix

| # | Scenario | Expect | Method |
|---|----------|--------|--------|
| L4-1 | High hard-fault style activity with healthy avail + quiet pagefile | `mem_lock` ≠ `hard` | `mapped-io-hog` |
| L4-2 | Coding pulse (`cargo check` / build) concurrent | `mem_lock` ≠ `hard` | probe script pulses |
| L4-3 | Protected / IDE names never get `mem_lock` throttle | no hit | status `recent_throttles` scan |
| L4-4 | L1: commit/faults without paging evidence | Soft only | `cargo test -p guardian-core mem_hard_requires_paging` |

## Sign-off

| Check | Date | Result |
|-------|------|--------|
| L4-1..L4-3 automated probe | 2026-07-17 | **PASS** (5m; see evidence) |
| L4-4 unit | 2026-07-17 | **PASS** |
| Evidence | | [mem-lock-l4-evidence.md](mem-lock-l4-evidence.md) |

## Fix noted during L4

v0.1.2 path gating could omit `Cursor.exe` path when idle, so path-only `\cursor\` protection missed. Mitigations in tree: IDE names in `ProtectedSet` + always resolve path for known IDE exe names.
