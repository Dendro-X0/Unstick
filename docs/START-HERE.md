# Unstick — start here

**Scope:** Windows-only OS-disk / RAM **hardware control** — freeze mitigation + load/thermal relief, not a general performance suite.  
**Option 2:** [hardware-control redesign](../specs/backend/hardware-control-redesign.md) — D0–D5 **Done**.  
**Shipped:** **[v0.8.0](RELEASE-v0.8.0.md)** in-app updates ([roadmap](roadmap-v0.8.0.md)); unsigned portable = current Latest intent until signed.

1. Design: [specs/backend/guardian-design.md](../specs/backend/guardian-design.md) · **updates:** [in-app-update-design.md](../specs/backend/in-app-update-design.md) · **north-star:** [hardware-control-north-star.md](../specs/backend/hardware-control-north-star.md)
2. **Roadmaps:** [next](roadmap-next-release.md) · **[v0.8.0](roadmap-v0.8.0.md)** · [v0.7.0](roadmap-v0.7.0.md) · [future](roadmap-future.md) · [v0.8.0 notes](RELEASE-v0.8.0.md)
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
