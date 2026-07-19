# Authenticode signing — status (v0.5.1 T5)

```
HANDOFF ATOMIC STEP: v0.5.1 T5 — signed Latest or document blocker
```

## Desired command

```powershell
pwsh -File scripts/Package-Portable.ps1 -Sign
# optional: $env:UNSTICK_SIGN_THUMBPRINT = "<cert sha1>"
```

Produces signed `dist\*.exe` and refreshes `Unstick-*-windows-x64.zip` with `SIGNING.txt` stating Authenticode OK.

## Current blocker (fill / update)

| Item | Status |
|------|--------|
| Code-signing certificate in CurrentUser/LocalMachine store | **Not available** on this build machine (as of v0.5.1 start) |
| `UNSTICK_SIGN_THUMBPRINT` | unset |
| Public Latest honesty | Remains **unsigned** portable until cert obtained |

## When unblocked

1. Install/import Authenticode cert  
2. Set thumbprint env or rely on `signtool /a`  
3. `Package-Portable.ps1 -Sign` with `REQUIRE_SIGN=1`  
4. Upload new asset to GitHub release (0.5.1 or replace 0.5.0 Latest)  
5. Mark T5 Done on [roadmap-v0.5.1.md](../docs/roadmap-v0.5.1.md)  
