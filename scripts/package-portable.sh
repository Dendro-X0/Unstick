#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --release -p guardian-service -p guardian-tray -p guardian-ui
mkdir -p dist
cp -f target/release/guardian-service.exe dist/
cp -f target/release/guardian-tray.exe dist/
cp -f target/release/guardian-ui.exe dist/
cp -f README.md dist/README.txt
cp -f docs/USER-GUIDE.md dist/ 2>/dev/null || true
cp -f docs/packaging-and-soak.md dist/
cp -f docs/frontend-spec.md dist/ 2>/dev/null || true
cp -f scripts/Install-Autostart.ps1 dist/ 2>/dev/null || true
cp -f scripts/Uninstall-Autostart.ps1 dist/ 2>/dev/null || true
echo "Portable package in dist/"
