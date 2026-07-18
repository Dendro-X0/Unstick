# Hard-fault cause split — design

```
HANDOFF ATOMIC STEP: none — greenfield from os-stutter-factors P0 item (2)
PAUSED / CANCELLED:    none
CANONICAL OWNER:       guardian-win::sensors (PDH Memory/Paging File) → guardian-core::pressure
PROOF BEFORE DONE:     L1 cargo test -p guardian-core; L3 status shows paging fields + discounted mapped I/O
```

## Problem

`Memory\Pages/sec` (hard faults) includes pagefile **and** memory-mapped / cache file reads ([MS phantom hard faults](https://learn.microsoft.com/en-us/archive/blogs/clinth/the-case-of-the-phantom-hard-page-faults)). Treating all hard faults as RAM pressure causes false Emergency / `ram_and_faults`.

Unstick today: `page_fault_count()` is a stub (`None` → falls back to `used_swap`), so the fault signal is already weak/wrong.

## Approach (user-mode, no ProcMon)

Sample PDH:

| Counter | Role |
|---------|------|
| `\Memory\Pages/sec` | All hard-fault pages (mapped + pagefile) |
| `\Memory\Page Writes/sec` | **Pagefile writes only** (MS: Page Writes/Output are pagefile) |
| `\Paging File(_Total)\% Usage` | Pagefile fullness |

Heuristic (Q139609): high Pages/sec + healthy Available + low Paging File % Usage (+ low Page Writes) ⇒ **mapped I/O**, not thrash.

### Classification

- `looks_like_mapped_io`: hard_faults ≥ 200 **and** avail > 12% **and** paging_file_pct < 15 **and** pagefile_writes < 50/s
- `paging_pressure_evidence`: paging_file_pct ≥ 20 **or** pagefile_writes ≥ 100/s **or** avail < 8%

### Pressure

- `fault_pressure`: if mapped → weight ×0.15; else full `hard_faults/2000`
- Tripwire `ram_and_faults`: avail < 5% **and** hard_faults ≥ 500 **and** `paging_pressure_evidence`

### Sample / status

| Field | Meaning |
|-------|---------|
| `hard_faults_per_sec` | Pages/sec |
| `pagefile_writes_per_sec` | Page Writes/sec |
| `paging_file_pct` | Paging File(_Total) % Usage |

## Out of scope

- Exact per-IRP attribution to `pagefile.sys` (needs ProcMon / ETW)
- Moving/resizing pagefile
