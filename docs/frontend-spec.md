# Frontend Spec — Unstick Client

## Meta

- **Product:** Unstick
- **Audience:** Windows users who need **disk/RAM hardware control** under pressure — not a general PC-optimizer audience
- **Reference:** Compact Guard shell (central CTA, top tabs, bottom hardware gauges) — **not** a booster-suite clone
- **Stack:** Native Rust **eframe/egui** window + existing named-pipe IPC (`guardian-service`). Tray remains optional companion. (Tauri/React deferred — keep one Rust toolchain for v1 UI.)
- **Spec status:** D5 hardware-control framing (v1.5)
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
3. Compact status chips when relevant (Disk/RAM control capping·holding, Disk Lock / Mem Lock, suspended) — centered under CTA
4. Centered **Controls** pill (collapsed by default)

### Secondary — collapsible **Controls** strip (collapsed by default)

Toggle label: `Controls ▸` / `Controls ▾`. Auto-expand when Disk/Mem Lock is active, disk/mem **control** is holding/capping, or `suspended_n > 0`.

Contents:

1. **Hardware Guard** checkbox (master enable)
2. Mode: **Soft only** (default); **Last-resort pause** only if `experimental_suspend`
3. Suspended count chip (rarely non-zero on Soft-only path)
4. Safety / recovered / elevation banners (only when non-empty)
5. **Hardware control** readout: envelope calibrated/learning, `u_disk` / `u_mem`, setpoint band, mode + intensity
6. **Advanced thresholds ▸** (collapsed): Soft/Hard disk Active Time % and RAM available % sliders + Apply + presets

Hero also shows **Focus · app.exe** when the service reports a foreground process (LIVE only).

### Motion

- CTA ring pulse when band ≥ warn, Disk/Mem Lock on, or control **capping**
- Gauge fills animate toward new values (lerp)

## Routing (tabs)

| Tab | Purpose | Data |
|-----|---------|------|
| Guard | Arm/disarm protection; primary CTA + collapsed controls | status + Pause/Resume + SetCriticalGuard + SetCriticalGuardMode + SetDiskSafeThresholds |
| Monitor | Top processes, sparklines, event log | status + `Events { limit }` / events.jsonl |
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
5. **Event log** — last ~40 from session / `events.jsonl` (throttle, suspend, resume, info)

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
| Refresh (1s timer) | `GetStatus` + `Events { limit: 40 }` or status.json / events.jsonl |
| CTA pause | `Pause { minutes: 15 }` |
| CTA resume | `Resume` |
| Trust | `TrustPid` |
| Apply safe disk | `SetDiskSafeThresholds { soft_pct, hard_pct }` |
| Critical Guard | `SetCriticalGuard { enabled }` |
| Critical Guard mode | `SetCriticalGuardMode { mode }` (`soft_only` default; `last_resort_suspend` only if `experimental_suspend`) |
| Whitelist add/remove | `AddWhitelist` / `RemoveWhitelist` |

Offline: show “Service offline — start guardian-service” banner; still render last status.json if present.

## Proof

1. `cargo build --release -p guardian-ui`
2. Start `guardian-service` then `guardian-ui`
3. CodaCtrl MCP `client_session_connect` attempted — **egui has no CDP**; Playwright browsers missing on this machine → evidence via Win32 `PrintWindow` PNGs under `docs/ui-captures/`
4. Guard first viewport shows CTA + pressure without disk sliders or profile cards until Controls expanded
5. CTA Pause/Resume and Apply thresholds still work
