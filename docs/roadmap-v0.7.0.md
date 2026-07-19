# Unstick — v0.7.0 roadmap (UX & ops)

**Status:** **Shipped** (unsigned portable)  
**Theme:** Operator clarity — what Guard did, without dashboard sprawl  
**Parent:** [roadmap-future.md](roadmap-future.md) · **Notes:** [RELEASE-v0.7.0.md](RELEASE-v0.7.0.md) · zip `Unstick-0.7.0-windows-x64.zip`  
**Design:** [session-actions-summary-design.md](../specs/backend/session-actions-summary-design.md) · [guard-profiles-design.md](../specs/backend/guard-profiles-design.md) · [prove-control-export-design.md](../specs/backend/prove-control-export-design.md)  
**Living index:** [roadmap-next-release.md](roadmap-next-release.md)

```
HANDOFF ATOMIC STEP: v0.7 U5 Done — shipped 0.7.0; next Authenticode or v1.0 gates
PAUSED / CANCELLED:    New Soft actuators; Suspend-as-default; timer-res; zero-stutter claims; hero marketing tiles; boost modes; bundling disk_hog in zip by default
CANONICAL OWNER:       release artifacts
PROOF BEFORE DONE:     met — L1 tests; portable zip; GitHub Latest
```

## Goal

Users understand **what Guard did** this session and can run Unstick as a daily tool — without fake health scores or a second product surface.

Builds on 0.6: sensing vs capping / Efficiency Idle are already visible. 0.7 answers “how many times did Soft act, and did it restore?”

## Ship gates

| ID | Work | Done when |
|----|------|-----------|
| **U0** | Design: session actions summary (counts + surface) | **Done** — [session-actions-summary-design.md](../specs/backend/session-actions-summary-design.md) |
| **U1** | Session actions summary in Guard / Monitor | **Done** — `session_*` on status; Monitor + Controls line; soft_restore events |
| **U2** | Profiles: Gaming / Dev / Quiet | **Done** — [guard-profiles-design.md](../specs/backend/guard-profiles-design.md); SetProfile + Controls |
| **U3** | Tray: pressure + capping / idle badge | **Done** — tooltip + icon tone (cap/idle pip); CLI ctrl= |
| **U4** | Optional: in-app “prove control” + config export/import | **Done** — [prove-control-export-design.md](../specs/backend/prove-control-export-design.md) |
| **U5** | Version bump 0.7.0 + RELEASE + zip | **Done** — `Unstick-0.7.0-windows-x64.zip` |

## Suggested order

1. **U0 → U1** (trust after EMERGENCY — smallest clarity win)  
2. **U3** (always-on without opening UI — cheap if status fields already exist)  
3. **U2** (profiles — policy skins; design before code)  
4. **U4** (optional polish)  
5. **U5** ship  

**Parallel (not a 0.7 gate):** Authenticode when cert exists — [signing-blocker.md](signing-blocker.md).

## Design notes (U0–U1)

- Prefer **aggregates** over a second event stream: e.g. session capped / restored / efficiency-idle counts from existing `events.jsonl` + `recent_throttles`.  
- Show on **Monitor** Event log header or a single Guard Controls line — not the first-viewport hero.  
- Claim discipline: counts mean Soft actuators ran; they do **not** mean freezes were prevented.  
- Soft TTL restore should increment “restored” when EcoQoS/Idle/priority return to normal.

## Profiles (U2) — shipped

See [guard-profiles-design.md](../specs/backend/guard-profiles-design.md). Soft-only skins; additive whitelist; no boost claims.

## Out of 0.7.0

| Item | Why |
|------|-----|
| New Soft actuators / PI rewrite | Control depth was 0.6; quality later with soak |
| MSI / Store | Not required for 1.0 |
| Authenticode | Parallel blocker, not UX slice |
| Dashboard sprawl / health scores | Anti-goal in future roadmap |
| Claiming zero launch stutter | L3 honesty unchanged |

## Acceptance (ship v0.7.0)

1. After a cliff or Soft session, user can see **session capped / restored** (and Idle if used) without reading raw JSON.  
2. Soft-only product path unchanged; no new hang surface.  
3. Profiles (if shipped) only change whitelist + Soft policy knobs already in config.  
4. Tray (if shipped) reflects pressure band and capping/idle without opening Guard.  
5. RELEASE + unsigned zip; Latest intent remains unsigned until cert.

## First atomic step

None for 0.7 — shipped. Next: Authenticode when cert, or v1.0 gates in [roadmap-future.md](roadmap-future.md).
