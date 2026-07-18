# macOS QoS + App Nap — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P2 (macOS QoS / App Nap)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-core::qos (portable) → guardian-mac (apply on Darwin) → Windows maps to existing priority ladder
PROOF BEFORE DONE:     L1 qos mapping tests; crates compile on Windows; status exposes focus_qos / nap_policy
```

## Goal

Apple’s intended control plane is **QoS classes** + **App Nap / ProcessInfo activities**, not NtSuspend-style pauses ([Energy Efficiency Guide](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/PrioritizeWorkAtTheTaskLevel.html), [App Nap](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/AppNap.html)).

This pass lands the **portable contract** and Windows-side labeling so SoftOnly / focus boost already align with Nap philosophy. Full Darwin apply lives in `guardian-mac` (compiles as stubs off-macOS).

## Portable types (`guardian-core::qos`)

| Type | Values |
|------|--------|
| `QosClass` | `UserInteractive`, `UserInitiated`, `Utility`, `Background`, `Default` |
| `NapPolicy` | `Cooperate` (default — SoftOnly), `ForcePause` (LastResortSuspend analogue) |

### Mapping

| Input | Focus QoS | Background QoS | NapPolicy |
|-------|-----------|----------------|-----------|
| SoftOnly + any focus | UserInteractive (focused tree) | Utility → Background by band | Cooperate |
| LastResort + Serious thermal | UserInteractive | Background | Cooperate (no ForcePause while hot) |
| LastResort + Emergency streak | UserInteractive | Background | ForcePause |

Windows apply (existing):

- Focus QoS UserInteractive → AboveNormal boost
- Background Utility → BelowNormal; Background → Idle
- ForcePause → Suspend (already gated)

macOS apply (future / stub):

- `pthread_set_qos_class_self_np` / GCD QoS / `NSProcessInfo.beginActivity`
- Never invent Suspend; use App Nap + lower QoS

## Status

| Field | Meaning |
|-------|---------|
| `focus_qos` | Planned QoS for focused tree |
| `background_qos` | Planned QoS for offenders |
| `nap_policy` | `cooperate` \| `force_pause` |

## Crate layout

- `crates/guardian-core/src/qos.rs` — types + `plan_qos(...)`
- `crates/guardian-mac` — Darwin sensors/apply; non-macOS stub exports `supported() -> false`

## Out of scope this pass

- Shipping a macOS UI/service binary
- objc2 AppKit integration beyond stubs
- Changing Windows SoftOnly default
