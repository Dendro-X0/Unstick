# Unstick
# https://github.com/Dendro-X0/Unstick

Windows user-mode background guardian that keeps low-end desktops responsive under **dev builds, MCP load, and gaming** — with behavioral abuse / miner heuristics.

See design: [specs/backend/guardian-design.md](specs/backend/guardian-design.md) · UI: [docs/frontend-spec.md](docs/frontend-spec.md)

## Quick start

```bash
cargo build --release -p guardian-service -p guardian-ui
./target/release/guardian-service.exe
# polished client (Guard / Monitor / Apps / Protect):
./target/release/guardian-ui.exe
```

Optional tray/CLI: `guardian-tray.exe --cli` or `--tray`.

Autostart (current user):

```bash
bash scripts/install-autostart.sh
```

## Workspace

| Path | Role |
|------|------|
| `crates/guardian-core` | Pressure scoring + policy |
| `crates/guardian-win` | Sensors + soft throttle |
| `crates/guardian-detect` | Abuse / miner heuristics |
| `apps/guardian-service` | Background sampler / actor |
| `apps/guardian-ui` | Polished desktop client |
| `apps/guardian-tray` | Tray + CLI status client |
| `fixtures/fake_miner` | L4 abuse decoy |

## Tests

```bash
cargo test -p guardian-core -p guardian-detect
```

## Config / logs

`%LOCALAPPDATA%\Unstick\config.json`  
`%LOCALAPPDATA%\Unstick\guardian.log` (daily rotate)  
`%LOCALAPPDATA%\Unstick\events.jsonl`  
`%LOCALAPPDATA%\Unstick\status.json`

End-user steps: [docs/USER-GUIDE.md](docs/USER-GUIDE.md)
