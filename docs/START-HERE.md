# Unstick — start here

1. Design: [specs/backend/guardian-design.md](../specs/backend/guardian-design.md)
2. **Roadmaps:** [next release](roadmap-next-release.md) · [v0.1 detail](roadmap-v0.1.md) · [v0.3.0 release](RELEASE-v0.3.0.md)
3. **User guide:** [USER-GUIDE.md](USER-GUIDE.md)
4. UI: [frontend-spec.md](frontend-spec.md)
5. Dev loop:
   - `pnpm install` then `pnpm dev` (builds debug + starts service & UI)
   - or release: `pnpm build` / `pwsh -File scripts/Package-Portable.ps1`
6. Run packaged binaries (after package):
   - `dist/guardian-service.exe`
   - `dist/guardian-ui.exe`
   - `guardian-tray.exe --cli` — console status (optional)
7. Autostart: `pwsh -File scripts/Install-Autostart.ps1 -StartNow`
8. Uninstall: `pwsh -File scripts/Uninstall-Autostart.ps1 -StopProcesses`
9. **P2 proof:** [p2-proof-checklist.md](p2-proof-checklist.md) · `powershell -File scripts/Verify-P2-Automated.ps1`
10. Ops / soak: [packaging-and-soak.md](packaging-and-soak.md) · [critical-guard-soak.md](critical-guard-soak.md)
