#!/usr/bin/env bash
# Thin wrapper — prefer scripts/Install-Autostart.ps1 on Windows.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$ROOT/scripts/Install-Autostart.ps1" "$@"
