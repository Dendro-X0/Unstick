# Frontend Spec — Unstick Client

## Meta

- **Product:** Unstick
- **Audience:** Developers and gamers on low-end Windows PCs who need freeze prevention + light abuse awareness
- **Reference:** Smart Game Booster–style shell (central CTA, top tabs, bottom hardware gauges) — **not** a game-only booster clone
- **Stack:** Native Rust **eframe/egui** window + existing named-pipe IPC (`guardian-service`). Tray remains optional companion. (Tauri/React deferred — keep one Rust toolchain for v1 UI.)
- **Spec status:** approved for implement (v1.2 Guard-first)
- **API dependency:** `ClientRequest` / `ServerPush` / `StatusSnapshot` / status.json fallback (already shipped)

## Visual direction

- **Theme:** Dark technical console (charcoal `#1A1D22` base) — intentional for this product category
- **Brand:** “UNSTICK” as hero-level mark in the header (not a tiny nav label)
- **Accent:** Teal `#2EC4B6` for healthy gauges / armed state; Coral `#E63946` for primary CTA ring; Amber `#F4A261` for warn/abuse
- **Surfaces:** Single continuous panel — no card stacks in the hero. Tabs are angular segments, not pill clusters
- **Density:** Consumer tool, one job per view — Guard first viewport is brand + CTA + pressure only
- **NOT:** purple gradients, cream/serif marketing, newspaper columns, emoji, multi-layer glassmorphism, inset hero cards, decorative non-interactive profile cards

## App shell

```
┌──────────────────────────────────────────────────────────┐
│  U  v0.1.0                               [─][□][×]       │
│  UNSTICK                              [ LIVE ]           │
│  [ GUARD ] [ MONITOR ] [ WHITELIST ] [ PROTECT ]         │
├──────────────────────────────────────────────────────────┤
│                                                          │
│                    (active tab body)                     │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  CPU ▓▓▓░░  RAM ▓▓░░░  DISK ▓▓▓▓░  PRESSURE ▓▓░░░      │
└──────────────────────────────────────────────────────────┘
```

- Window: ~920×640, resizable, min 780×520
- Chrome icon glyph: **U** (not FG)
- Brand row: **UNSTICK** + dim version only (no competing tagline in the brand row)

## Visual polish (v1.1)

- **Unified shell:** slim chrome (icon + drag strip + controls) → brand row → full-width tab strip
- **Tabs:** equal-width trapezoid segments (top edge flares wider) with teal active underline
- **Window:** Windows 11 DWM rounded HWND corners (`DWMWA_WINDOW_CORNER_PREFERENCE`)
- **Monitor:** CPU + disk sparklines (60 samples @ 1 Hz)
- **Protect:** severity tiers WATCH / HIGH / CRITICAL with painted icons
- **Motion:** footer gauge segments ease fractionally when values change
- **Guard hero:** radial teal vignette, larger CTA with glow rings, centered pressure readout
- **Footer:** hairline accent, column dividers, label + % header row per gauge
- **Palette:** `#39C6B4` teal, `#E68D50` amber warnings, deeper charcoal base

- Bottom gauge bar always visible (reference pattern)

## Guard-first layout (v1.2)

### First viewport (always visible without scroll)

1. Large circular CTA: **ARMED** / **PAUSED** — click Pause 15m / Resume
2. Centered pressure cluster: small **PRESSURE** label, band chip, score
3. Compact status chips when relevant (Disk Lock / Mem Lock / suspended) — centered under CTA
4. Centered **Controls** pill (collapsed by default)

### Symmetry (v1.3)

- Vertically center the Guard hero when Controls is collapsed
- Pressure readout is a centered stack (no left-heavy “Pressure band” label)
- Footer gauges form a centered equal-width block
- PAUSED CTA uses calm teal rings; ARMED keeps coral pulse
- Toasts render as a centered chip with shortened copy

### Responsive layout (v1.4)

- Footer gauges use **equal fractional columns** sized from `available_width` with gutters (no fixed max that cramps labels)
- Pressure **NORMAL** chip is a fixed-height painted pill (24px) — never stretches with the row
- Monitor sparklines use `ui.columns(2)` and fill each column’s width (no clipping on resize)

### Secondary — collapsible **Controls** strip (collapsed by default)

Toggle label: `Controls ▸` / `Controls ▾`. Auto-expand when Disk Lock / Mem Lock is active or `suspended_n > 0`.

Contents:

1. Critical Guard checkbox
2. Mode: **Soft only** (default) / **Last-resort pause**
3. Suspended count chip
4. Safety / recovered / elevation banners (only when non-empty)
5. Safe disk usage: Soft / Hard sliders + Apply + presets `85/95`, `70/90`

Hero also shows **Focus · app.exe** when the service reports a foreground process (LIVE only).

### Removed from Guard

- Decorative **DEV BUILDS** / **GAMES & PLAY** profile cards (non-interactive; cluttered the hero)
- Focus profile is a status label only (`dev` / `play` / `other`) — same scheduling ladder

### Motion

- CTA ring pulse when band ≥ warn or Disk Lock on
- Gauge fills animate toward new values (lerp)

## Routing (tabs)

| Tab | Purpose | Data |
|-----|---------|------|
| Guard | Arm/disarm protection; primary CTA + collapsed controls | status + Pause/Resume + SetCriticalGuard + SetCriticalGuardMode + SetDiskSafeThresholds |
| Monitor | Top processes, sparklines, recent throttles | status |
| Whitelist | Never-throttle / never-suspend entries | AddWhitelist / RemoveWhitelist |
| Protect | Abuse/miner alert summary + trust actions | recent_abuse + TrustPid |

## Page: Monitor

### Purpose
See what is consuming the machine.

### Layout
1. Title + one-line purpose
2. CPU + DISK sparklines
3. Suspended list (if any)
4. Scrollable top-process rows: cpu%, pid, name, mem, Whitelist action
5. Recent throttle events (last few) — empty state: “None this session”

## Page: Whitelist

### Purpose
Manage programs that must never be soft-throttled or suspended.

### Layout
1. Title + short explanation
2. List with Remove (or empty state)
3. Add entry field + Whitelist button
4. Tip linking to Monitor one-click whitelist

## Page: Protect

### Purpose
Surface abuse/miner heuristics without pretending to be AV.

### Layout
1. Explanation line: “Behavioral heuristics — not antivirus”
2. Empty: calm teal panel — no hits
3. Hits list with severity + Trust → `TrustPid`

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
| Critical Guard | `SetCriticalGuard { enabled }` |
| Critical Guard mode | `SetCriticalGuardMode { mode }` (`soft_only` \| `last_resort_suspend`) |
| Whitelist add/remove | `AddWhitelist` / `RemoveWhitelist` |

Offline: show “Service offline — start guardian-service” banner; still render last status.json if present.

## Proof

1. `cargo build --release -p guardian-ui`
2. Start `guardian-service` then `guardian-ui`
3. CodaCtrl MCP `client_session_connect` attempted — **egui has no CDP**; Playwright browsers missing on this machine → evidence via Win32 `PrintWindow` PNGs under `docs/ui-captures/`
4. Guard first viewport shows CTA + pressure without disk sliders or profile cards until Controls expanded
5. CTA Pause/Resume and Apply thresholds still work
