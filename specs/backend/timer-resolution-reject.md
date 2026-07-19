# Timer resolution / multimedia power flag — reject (S2)

```
HANDOFF ATOMIC STEP: v0.5.0 S2 — investigate timer-resolution lever → reject
PAUSED / CANCELLED:    Adopting timeBeginPeriod globally; forcing HighQoS timer for games
CANONICAL OWNER:       specs/backend (no code path)
PROOF BEFORE DONE:     This note + north-star matrix Verdict = Reject
```

## Question

Should Unstick apply `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION` (or `timeBeginPeriod` / multimedia timer APIs) to background offenders as a fluidity / power lever?

## What the OS documents

Source: [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation) (ProcessPowerThrottling).

| Flag / API | Effect |
|------------|--------|
| `PROCESS_POWER_THROTTLING_EXECUTION_SPEED` | EcoQoS — efficient cores / clocks (**already Soft path**) |
| `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION` | Process timer-resolution requests are **ignored**; timers no longer guaranteed higher resolution → power efficiency |
| Clearing IGNORE (StateMask 0 while ControlMask set) | Explicitly **honor** timer-resolution requests (games/media apps use this when minimized on Win11) |
| `timeBeginPeriod` (winmm) | Raises **system-wide** timer resolution while held — power cost, contended globally |

Windows 11 may already ignore timer-resolution requests for fully occluded / minimized / non-audible window owners unless the process opts out of IGNORE.

## Fit to Unstick mission

| Mission | Timer IGNORE as Soft demotion? |
|---------|--------------------------------|
| Freeze mitigation (disk/RAM cliff) | **Weak** — does not cut I/O queue or paging; EcoQoS + I/O/mem-prio already address offenders |
| Load / thermal relief | **Marginal** — only helps processes that already raised timer resolution; many hogs never call `timeBeginPeriod` |
| Targeted fluidity for focus | **Risk** — mis-applied IGNORE on a game/media child, or system-wide `timeBeginPeriod` from Guard itself, hurts timing or burns power |

## Risks if adopted

1. **Wrong polarity UX** — IGNORE helps *power* by coarsening timers; apps that need 1 ms timers (games, DAW, browsers) feel worse if demoted incorrectly.
2. **Overlap with EcoQoS** — EXECUTION_SPEED already demotes background work; stacking IGNORE adds complexity without disk/RAM cliff proof.
3. **Global timer pollution** — Guard calling `timeBeginPeriod` to “smooth” the desktop is a classic fake optimizer anti-pattern (raises power for everyone).
4. **Hard to prove L3** — no clear link from IGNORE → lower `u_disk` / fewer freezes in our soak fixtures.
5. **Restore surface** — another soft flag to TTL-restore; more hang/feel-stuck footguns.

## Verdict

**Reject** for Unstick product path (v0.5.0 and default backlog).

Keep using:

- EcoQoS (`EXECUTION_SPEED`) + priority + I/O + mem-prio + paging-gated WS  
- Freeze-safe envelope control + thermal stress headroom  

Do **not** implement:

- Applying `IGNORE_TIMER_RESOLUTION` to planned offenders  
- Guard-owned `timeBeginPeriod` / `timeEndPeriod`  
- Forcing HighQoS timer honors on focus (apps that need it already do so themselves)

## Revisit only if

A future investigation shows a **named** background class (e.g. updater services with known high-res timers) where IGNORE measurably reduces DPC-adjacent wakeups **and** focus/whitelist trees never receive it — with L3 evidence. Until then: **Reject**.

## Related

- [hardware-control-north-star.md](hardware-control-north-star.md) lever matrix  
- [ecoqos-efficiency-design.md](ecoqos-efficiency-design.md)  
- [docs/roadmap-v0.5.0.md](../../docs/roadmap-v0.5.0.md) S2  
