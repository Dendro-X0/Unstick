# Prove control + config export/import — design (v0.7 U4)

```
HANDOFF ATOMIC STEP: v0.7 U4 — optional prove-control + config JSON export/import
PAUSED / CANCELLED:    Bundling disk_hog in public zip by default; silent full cliff; new Soft actuators
CANONICAL OWNER:       guardian-service IPC; guardian-ui Controls tools row
PROOF BEFORE DONE:     L1 import round-trip / sanitize; L2 cargo check
```

## Goal

Lower friction for (1) moving Soft settings between machines and (2) opt-in Soft soak without memorizing `cargo run … cliff`.

## Config export / import

| Op | Behavior |
|----|----------|
| **Export** | Write pretty JSON of current `GuardianConfig` to `%LOCALAPPDATA%\Unstick\exports\unstick-config.json` |
| **Import** | Read that path (or `imports\unstick-config.json` if export missing), deserialize, `normalize_whitelist` + `normalize_suspend_product_path`, **clear `pause_until`**, save, reload live cfg |

- JSON only — no registry, no binary.  
- Import never invents Suspend-as-default (`normalize_suspend_product_path` already forces SoftOnly unless experimental).  
- No OS file picker in U4 (avoid new deps); paths are fixed under AppData; Ok message includes full path.

IPC:

```rust
ExportConfig,
ImportConfig,
```

## Prove control (opt-in disk soak)

| Rule | Detail |
|------|--------|
| Fixture | Existing `disk-hog.exe` — **not** a product feature |
| Locate | Same directory as `guardian-service.exe`, else error with build hint |
| Args | Default **prove** preset: `512` MiB × `90` s (shorter than `cliff`) |
| Spawn | Detached / no window; one at a time (reject if hog already running) |
| Honesty | UI warns: large TEMP write on OS volume; may stutter; Soft may cap — not a freeze-prevention demo |

IPC:

```rust
StartProveDiskHog,
```

UI (Controls, collapsed tools — not hero):

- `Export config` / `Import config`  
- `Prove Soft control (90s)` with hover warning  

## Non-goals

- Shipping `disk-hog` inside `Unstick-*-windows-x64.zip` by default  
- Full cliff from UI without a separate confirm path (U4 = prove preset only)  
- Auto-stop hog from UI (user can Task Manager; hog self-exits)

## Proof

| Layer | Work |
|-------|------|
| L1 | Export path helper; import clears pause; reject garbage JSON |
| L2 | `cargo check -p guardian-service -p guardian-ui` |

## Acceptance

1. Export writes readable JSON under AppData `exports\`.  
2. Import restores whitelist/profile knobs from that file.  
3. Prove starts disk-hog when present; clear error when absent.  
4. No hero clutter; SoftOnly unchanged.
