# Unstick — start here

1. Design: [specs/backend/guardian-design.md](../specs/backend/guardian-design.md)
2. **v0.1 roadmap:** [roadmap-v0.1.md](roadmap-v0.1.md)
3. **User guide:** [USER-GUIDE.md](USER-GUIDE.md)
4. UI: [frontend-spec.md](frontend-spec.md)
5. Build / package (Windows):
   - `pwsh -File scripts/Package-Portable.ps1`
   - or `cargo build --release -p guardian-service -p guardian-ui`
6. Run service, then client:
   - `dist/guardian-service.exe`
   - `dist/guardian-ui.exe`
   - `guardian-tray.exe --cli` — console status (optional)
7. Autostart: `pwsh -File scripts/Install-Autostart.ps1 -StartNow`
8. Uninstall: `pwsh -File scripts/Uninstall-Autostart.ps1 -StopProcesses`
9. **P2 proof:** [p2-proof-checklist.md](p2-proof-checklist.md) · `powershell -File scripts/Verify-P2-Automated.ps1`
10. Ops / soak: [packaging-and-soak.md](packaging-and-soak.md) · [critical-guard-soak.md](critical-guard-soak.md)
