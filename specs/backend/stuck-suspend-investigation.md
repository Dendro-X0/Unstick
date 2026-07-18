# Investigation: stuck NtSuspend (Chrome / Terminal)

## Symptom

User observed Chrome frozen and Terminal shown as suspended in Task Manager. Processes did not resume when resources freed; only Task Manager kill recovered them. Unstick Critical Guard / Disk Lock Hard is the likely suspend source.

## Evidence (code)

1. Suspend: `throttle.rs` `NtSuspendProcess` when policy plans `ThrottleLevel::Suspend` (Emergency or Disk Lock Hard + Critical Guard).
2. Chrome / Terminal / PowerShell were **not** in default `ProtectedSet`.
3. Tick order in `runtime.rs`: resume `expired_pids` → then `apply(plan)` re-suspended same PIDs while Emergency/Hard persisted. Ledger `or_insert` reset the 45s clock → **max_suspend_secs never stuck**.
4. `resume_pids` removed ledger entry **before** `NtResumeProcess`; failure → orphan with no retry.

## Root cause

Primary: same-tick re-suspend after max-suspend resume under sustained Emergency/Disk Lock Hard.  
Secondary: failed resume dropped ledger; browsers/shells unprotected by default.

## Fix (implemented)

1. **Cooldown** — after successful resume, refuse `NtSuspend` for that PID for `max_suspend_secs` (`throttle.rs`).
2. **Ledger** — only remove entry after successful `NtResumeProcess`; retry on failure.
3. **ProtectedSet** — default-protect Chrome/Edge/Firefox/Brave + Windows Terminal / PowerShell / cmd / conhost.

## Proof

- L1: `cargo test -p guardian-core` — `browsers_and_shells_never_suspended` passes (23 tests).
- L3: restart `guardian-service` with this build; if anything is still frozen, restart once more to run P0 orphan resume.
