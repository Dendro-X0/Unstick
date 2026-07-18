# Unstick
# https://github.com/Dendro-X0/Unstick

**Windows-only** portable Guard that protects the OS drive (SSD/HDD) and RAM under pressure.  
**v0.3:** Soft Disk/Mem Lock + EcoQoS. **Option 2 (D0–D5):** closed-loop hardware control — [design](specs/backend/hardware-control-redesign.md).

See: [specs/backend/guardian-design.md](specs/backend/guardian-design.md) · [docs/USER-GUIDE.md](docs/USER-GUIDE.md) · [docs/roadmap-next-release.md](docs/roadmap-next-release.md)


## Quick start

```bash
pnpm install
pnpm dev
```

This builds (debug) and starts `guardian-service` + `guardian-ui`. Ctrl+C stops both.

Manual / release:

```bash
pnpm build
# or: cargo build --release -p guardian-service -p guardian-ui
./target/release/guardian-service.exe
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
