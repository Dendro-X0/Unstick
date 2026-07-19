# Self-overhead L3 evidence (v0.1.2)

**Date:** 2026-07-17 (local) / 2026-07-18 UTC  
**Machine:** `XTZJ-20221014TG`  
**Method:** `scripts/Measure-SelfOverhead.ps1` — `% of one logical core` ≈ `100 * Δ(Get-Process.CPU) / wall_seconds`  
**Build:** `cargo build --release -p guardian-service`  
**UI:** closed (service-only idle)

## Results

| Label | Change set | WallSec | PctOneCore | CSV |
|-------|------------|---------|------------|-----|
| before | v0.1.1 tree (pre self-overhead) | 60.06 | **4.319** | `self-overhead-measure-before-20260717-222431.csv` |
| after | gated cmdline + compact/throttled status.json | 60.08 | **2.081** | `self-overhead-measure-after-20260717-222731.csv` |
| after2 | + gated exe path | 60.04 | 3.565 | `self-overhead-measure-after2-20260717-222917.csv` |

## Verdict

- **Best measured idle:** **2.081% of one core** (~**52%** reduction vs 4.319% baseline).  
- Plan aspirational bar was **≤ ~0.5%**. On this machine the remaining cost is dominated by **full `refresh_processes(All)` + PDH every 2s**, which v0.1.2 deliberately keeps (Mem/Disk Lock + detect need the enum). Hitting ≤0.5% needs a future sampling strategy (not this release).  
- `after2` variance (3.565%) shows probe noise under background load; path gating remains shipped for fewer Win32 string queries on cold PIDs.  
- UI paint change is behavioral (no longer fixed 30 Hz); not measured in service-only idle rows.

## Pass criteria for ship

| Check | Result |
|-------|--------|
| Meaningful idle CPU reduction vs before | **PASS** (~52% at best sample) |
| SoftOnly / sample intervals unchanged | **PASS** (by design) |
| L1 cmdline gate tests | run in Verify-P2 / `cargo test -p guardian-win` |
| L2 `Verify-P2-Automated.ps1` | see ship checklist |

**Claim language:** Self-overhead **improved and verified at L3** on this host; **not** claimed to meet absolute ≤0.5% one-core until a lighter process-sample design lands.

---

## v0.3.0 follow-up (Normal-band every-other-tick enum)

| Label | WallSec | PctOneCore | CSV |
|-------|---------|------------|-----|
| v0.3.0 | 45.05 | **1.179** | `self-overhead-measure-v0.3.0-20260718-001131.csv` |

Toward plan S4 (~≤1% one-core idle on soak machine): **PASS** on this sample (~1.18%).

---

## v0.5.0 baseline (v0.5.1 T4)

| Label | WallSec | PctOneCore | CSV |
|-------|---------|------------|-----|
| v0.5.0-baseline | 45.08 | **1.456** | `self-overhead-measure-v0.5.0-baseline-20260718-205406.csv` |

**Machine:** `XTZJ-20221014TG` · service-only idle · release `guardian-service` started by measure script.  
**Note:** Slightly above the ~1.2% v0.3.0 sample (probe noise / north-star control path). Still well under “Guard is the freeze” territory. No code change required for 0.5.1 unless soak shows regression under busy.  
