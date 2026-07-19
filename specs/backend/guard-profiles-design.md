# Guard profiles — design (v0.7 U2)

```
HANDOFF ATOMIC STEP: v0.7 U2 — Gaming / Dev / Quiet profiles
PAUSED / CANCELLED:    Suspend-by-profile; fake boost modes; new Soft actuators; hero marketing tiles
CANONICAL OWNER:       guardian-core presets → config.json + SetProfile IPC; UI Controls
PROOF BEFORE DONE:     L1 apply_profile unit tests; L2 cargo check service+UI
```

## Problem

Whitelist and Soft knobs are powerful but opaque. Users need **named skins** (Gaming / Dev / Quiet) that only retune existing Soft policy — same engine, no Suspend, no “boost.”

## Goal

Three applyable profiles that:

1. Merge a **preset whitelist** (additive — never remove user entries).  
2. Set a small set of **existing Soft knobs** (tripwire thresholds, Idle-under-stress streaks, Soft TTL).  
3. Persist `active_profile` in `config.json` and expose on `StatusSnapshot`.  
4. Appear as three controls under **Controls** (not the hero).

## Non-goals

- New Soft actuators / intensity ceilings in code  
- Per-profile Suspend or experimental Suspend  
- FPS / overclock claims  
- Replacing Whitelist tab  
- Auto-detect “I launched a game” (manual apply only in U2)

## Profiles

| Profile | Intent | Whitelist merge (examples) | Soft knobs |
|---------|--------|----------------------------|------------|
| **Dev** | Default product path | IDE/tool fragments optional (`Code.exe`, `Cursor.exe`, `devenv.exe`, `idea64.exe`) | Today’s defaults: disk 85/95, mem avail 15/8, idle on (streak 4/2), Soft TTL 45s |
| **Gaming** | Protect launchers/games clients | `steam.exe`, `EpicGamesLauncher.exe`, `Battle.net.exe`, `RiotClientServices.exe`, `GalaxyClient.exe`, `upc.exe` / Ubisoft, `Origin.exe`, `EADesktop.exe` | Same Soft as Dev; idle escalate **5** (slightly slower to Efficiency Idle mid-session) |
| **Quiet** | Earlier Soft headroom | *(none required)* | Disk Soft/Hard **75/90**; mem Soft/Hard **20/12**; idle escalate **3**; Soft TTL **30s** (faster restore windows) |

All profiles force / leave:

- `critical_guard_mode = SoftOnly`  
- `experimental_suspend = false` (do not enable)  
- `disk_control_enabled` / `mem_control_enabled` / `idle_under_stress_enabled` = **true**

## Ownership

| Concern | Owner |
|---------|--------|
| Preset tables + `apply_profile` | `guardian-core` (`profiles.rs`) |
| Persist `active_profile` | `GuardianConfig` |
| IPC `SetProfile { profile }` | service `handle_request` → save + Ok |
| Status | `StatusSnapshot.active_profile: String` |
| UI | Controls: three selectable labels Dev / Gaming / Quiet |

## Config / IPC

```rust
// GuardianConfig
#[serde(default = "default_profile_dev")]
pub active_profile: String, // "dev" | "gaming" | "quiet"

// ClientRequest
SetProfile { profile: String },
```

Unknown profile → `ServerPush::Error`. Apply is idempotent.

Whitelist merge: case-insensitive dedupe via existing `add_whitelist` semantics.

## UI

Under Hardware control readout (Controls strip):

```text
Profile · Dev | Gaming | Quiet
```

- Selected = `active_profile` from status.  
- Click → `SetProfile`.  
- Hover: one-line intent (no boost claims).  
- Not in first viewport hero.

## Claim discipline

Profiles change **when Soft acts** and **what is never Soft-capped**. They do not claim freezes prevented or higher FPS.

## Proof

| Layer | Work |
|-------|------|
| L1 | `apply_profile` sets knobs + merges whitelist; SoftOnly preserved |
| L2 | `cargo check -p guardian-service -p guardian-ui` |
| L3 | Not required |

## Implementation slices

| Slice | Work |
|-------|------|
| **U2a** | This design |
| **U2b** | `profiles.rs` + config field + IPC + status |
| **U2c** | Controls UI + USER-GUIDE |

## Acceptance (U2 Done)

1. Applying Gaming merges launcher names and persists `active_profile=gaming`.  
2. Quiet uses earlier Soft tripwire thresholds; SoftOnly unchanged.  
3. Dev restores default Soft knobs.  
4. User whitelist entries survive profile switches.  
5. No hero clutter; no Suspend-by-profile.
