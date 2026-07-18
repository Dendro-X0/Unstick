# Thermal / power axis — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P2 (thermal/power)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-win sensors → guardian-core pressure/advisory → runtime/UI
PROOF BEFORE DONE:     L1 classify + soft-only-under-heat tests; L3 status power/thermal fields
```

## Goal

Treat thermal/power limiting as a **separate axis** from CPU/disk offenders. Throttling looks like stutter but Suspend/hard throttle can make it worse; prefer soft ladder + advisory ([Apple thermal guidance](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/RespondToThermalStateChanges.html) spirit on Windows proxies).

## Sensors (Windows)

| Signal | API | Field |
|--------|-----|--------|
| AC vs battery | `GetSystemPowerStatus` | `on_battery`, `battery_percent` |
| Cooling mode | `CallNtPowerInformation(SystemPowerInformation)` → `CoolingMode` | `cooling_mode`: `active` / `passive` / `unknown` |
| CPU clock headroom | `CallNtPowerInformation(ProcessorInformation)` → `CurrentMhz/MaxMhz` | `cpu_mhz_ratio` 0..1 |

## Levels (Apple-shaped)

| Level | Condition | Action |
|-------|-----------|--------|
| Nominal | AC, active/unknown cooling, mhz_ratio ≥ 0.90 | none |
| Fair | on_battery **or** passive cooling **or** mhz_ratio &lt; 0.85 | `thermal_some` mid; status advisory soft |
| Serious | (passive **and** mhz_ratio &lt; 0.70) **or** battery ≤ 20% | higher `thermal_some`; advisory; **suppress Suspend** even in LastResort |

Hard rules:

- Never force Emergency band from thermal alone
- Never Disk Lock Hard from thermal alone
- Serious ⇒ policy plans at most Idle soft throttle (no Suspend)

## Stall / score

Extend `StallFractions` with `thermal_some` (0..1).

```
Fair → ~0.35, Serious → ~0.70
score += 0.12 * thermal_some   // modest; full boost unchanged
```

## Status / UI

| Field | Type |
|-------|------|
| `on_battery` | bool |
| `battery_percent` | Option&lt;u8&gt; |
| `cooling_mode` | string |
| `cpu_mhz_ratio` | f32 |
| `thermal_level` | `nominal` / `fair` / `serious` |
| `thermal_advisory` | Option&lt;String&gt; |
| `stall_thermal` | f32 |

Guard chip: amber `Thermal · fair` / coral `Thermal · serious` / dim `Battery` when on battery + nominal.

## Out of scope

- Per-zone ACPI temperatures (°C)
- Changing Windows power plan
- macOS `thermalState` (separate P2)
