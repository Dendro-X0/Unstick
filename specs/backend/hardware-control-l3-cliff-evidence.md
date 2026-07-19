# Hardware-control L3 cliff soak — evidence

```
HANDOFF ATOMIC STEP: v0.5.1 T2 — cliff soak evidence
PAUSED / CANCELLED:    Claiming freeze prevented without before/after notes
CANONICAL OWNER:       human soak + Guard UI observation
PROOF:                 Fill a Run section below; attach status snippets if useful
```

## Purpose

Prove that under **sustained** OS-volume disk pressure (`disk_hog cliff` or equivalent), Unstick moves from **sensing** (EMERGENCY / tripwire · monitoring) to **soft capping**, then **releases** when the hog stops — without indefinite demotion hangs.

Fixture (not a product feature):

```bash
cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml -- cliff
# 2048 MiB × ~300s; ensure TEMP is on the OS / pagefile volume
```

## Checklist (each run)

1. [ ] `guardian-service` + `guardian-ui` LIVE; Hardware Guard ARMED; Soft only  
2. [ ] Note machine: CPU/RAM, OS drive type (SSD/HDD), Windows build  
3. [ ] Start cliff hog; watch Task Manager Active Time on system disk  
4. [ ] Record whether tripwire shows **monitoring** vs **soft capping · disk iN**  
5. [ ] Record Disk Lock Soft/Hard if shown; Event log **capped** lines  
6. [ ] Desktop usable? (mouse/keyboard within ~1s)  
7. [ ] Stop hog; within ~30–60s control → released / soft restores (or TTL ≤45s)  
8. [ ] No Explorer / Cursor / whitelist in suspended list  

## Run log

### Run 1 — _template_

| Field | Value |
|-------|--------|
| Date (UTC) | |
| Machine | |
| OS drive | SSD / HDD — model if known |
| Unstick version | 0.5.0 / 0.5.1 |
| Hog | `cliff` / custom MiB×secs |
| TEMP on OS volume? | yes / no |
| Peak DISK Active Time (TM) | |
| Peak Guard DISK % | |
| Peak pressure / band | |
| Tripwire text | monitoring / soft capping i? |
| Disk/RAM cap chips | |
| Event log capped? | yes / no (reasons) |
| Desktop usable during hog? | |
| After stop: released? | time to release |
| Notes / FP | |

_Copy this table for Run 2+_

## Conclusion (fill after ≥1 real run)

- Soft capping observed under sustained load: yes / no / partial  
- Release after ease: yes / no  
- Recommend setpoint change?: none / describe with data  
- Blockers for public trust:  
