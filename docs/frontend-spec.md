# Frontend Spec — Unstick Client

## Meta

- **Product:** Unstick
- **Audience:** Developers and gamers on low-end Windows PCs who need freeze prevention + light abuse awareness
- **Reference:** Smart Game Booster–style shell (central CTA, top tabs, bottom hardware gauges) — **not** a game-only booster clone
- **Stack:** Native Rust **eframe/egui** window + existing named-pipe IPC (`guardian-service`). Tray remains optional companion. (Tauri/React deferred — keep one Rust toolchain for v1 UI.)
- **Spec status:** approved for implement
- **API dependency:** `ClientRequest` / `ServerPush` / `StatusSnapshot` / status.json fallback (already shipped)

## Visual direction

- **Theme:** Dark technical console (charcoal `#1A1D22` base) — intentional for this product category
- **Brand:** “UNSTICK” as hero-level mark in the header (not a tiny nav label)
- **Accent:** Teal `#2EC4B6` for healthy gauges / armed state; Coral `#E63946` for primary CTA ring; Amber `#F4A261` for warn/abuse
- **Surfaces:** Single continuous panel — no card stacks in the hero. Tabs are angular segments, not pill clusters
- **Density:** Consumer tool, one job per view
- **NOT:** purple gradients, cream/serif marketing, newspaper columns, emoji, multi-layer glassmorphism, inset hero cards

## App shell

```
┌──────────────────────────────────────────────────────────┐
│  ≡   UNSTICK                         [pause] [─][□][×]  │
│  [ GUARD ] [ MONITOR ] [ APPS ] [ PROTECT ]               │
├──────────────────────────────────────────────────────────┤
│                                                          │
│                    (active tab body)                     │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  CPU ▓▓▓░░  RAM ▓▓░░░  DISK ▓▓▓▓░  PRESSURE ▓▓░░░      │
└──────────────────────────────────────────────────────────┘
```

- Window: ~920×640, resizable, min 780×520
## Visual polish (v1.1)

- **Unified shell:** slim chrome (FG icon + drag strip + controls) → brand row → full-width tab strip
- **Tabs:** equal-width trapezoid segments (top edge flares wider) with teal active underline
- **Window:** Windows 11 DWM rounded HWND corners (`DWMWA_WINDOW_CORNER_PREFERENCE`)
- **Monitor:** CPU + disk sparklines (60 samples @ 1 Hz)
- **Protect:** severity tiers WATCH / HIGH / CRITICAL with painted icons
- **Motion:** footer gauge segments ease fractionally when values change
- **Guard hero:** radial teal vignette, larger CTA with glow rings, centered pressure readout
- **Profiles:** equal-width cards with accent dots (teal = dev, amber = games)
- **Footer:** hairline accent, column dividers, label + % header row per gauge
- **Palette:** `#39C6B4` teal, `#E68D50` amber warnings, deeper charcoal base

- Bottom gauge bar always visible (reference pattern)

## Routing (tabs)

| Tab | Purpose | Data |
|-----|---------|------|
| Guard | Arm/disarm protection; primary CTA | status + Pause/Resume |
| Monitor | Top processes, recent throttle/abuse | status |
| Apps | Dev / Game profiles + allowlist paths | config via AddAllowPath / TrustPid |
| Protect | Abuse/miner alert summary + trust actions | recent_abuse + TrustPid |

## Page: Guard

### Purpose
One-tap engage/pause freeze protection while seeing live pressure.

### Layout
1. Brand row + **v0.1.0** + short line: “Keeps Dev & Play responsive”
2. Large circular CTA (coral ring): **ARMED** when running, **PAUSED** when paused — click toggles Pause 15m / Resume
3. Band chip under CTA: `normal | warn | throttle | emergency`
4. Critical Guard checkbox + `N suspended` chip
5. **Disk Lock** chip when soft/hard active — shows live threshold %
6. **Safe disk usage** panel: Soft / Hard sliders (default 85% / 95%) + Apply → `SetDiskSafeThresholds` (soft limits I/O; hard pauses processes)
7. Two profile shortcuts: **Dev builds** / **Games**

### Motion
- CTA ring pulse when band ≥ warn
- Gauge fills animate toward new values (lerp)

## Page: Monitor

### Purpose
See what is consuming the machine.

### Layout
1. Pressure score + band
2. Scrollable top-process rows: name, pid, cpu%, mem
3. Recent throttle events (last few)

## Page: Apps

### Purpose
Manage what counts as trusted/toolchain vs throttleable.

### Layout
1. Allow-path list from config (read status file / config.json)
2. Add path field + button → `AddAllowPath`
3. Hint: Cursor, cargo, node, Steam/game folders

## Page: Protect

### Purpose
Surface abuse/miner heuristics without pretending to be AV.

### Layout
1. Explanation line: “Behavioral heuristics — not antivirus”
2. Abuse hits list with score + reasons
3. Per-row **Trust** → `TrustPid`

## Bottom gauges

| Gauge | Source |
|-------|--------|
| CPU | `cpu_percent` |
| RAM | used = 1 − available/total |
| DISK | `disk_busy_percent` (PDH system-disk Active Time when available) |
| PRESSURE | `pressure_score * 100` |

Segmented LED-style bars (teal). Amber/coral fill when >70% / >85%.

## Wiring

| UI action | IPC |
|-----------|-----|
| Refresh (1s timer) | `GetStatus` or status.json |
| CTA pause | `Pause { minutes: 15 }` |
| CTA resume | `Resume` |
| Trust | `TrustPid` |
| Apply safe disk | `SetDiskSafeThresholds { soft_pct, hard_pct }` |

Offline: show “Service offline — start guardian-service” banner; still render last status.json if present.

## Proof

1. `cargo build -p guardian-ui`
2. Service running → gauges move; CTA pause flips band UI to paused
3. Protect tab shows fixture hits when fake-miner run (manual L4)
