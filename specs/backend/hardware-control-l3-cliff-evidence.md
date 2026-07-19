# Hardware-control L3 cliff soak — evidence

```
HANDOFF ATOMIC STEP: v0.6 I4 — WD Green L3 re-soak Run 2
PAUSED / CANCELLED:    Claiming freeze prevented / zero launch stutter
CANONICAL OWNER:       human soak + Guard status.json observation
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

1. [x] `guardian-service` + Guard ARMED; Soft only  
2. [x] Note machine: CPU/RAM, OS drive type (SSD/HDD), Windows build  
3. [x] Start cliff hog; watch Task Manager Active Time / `status.json` on system disk  
4. [x] Record whether tripwire / intensity shows **monitoring**, **soft capping · disk iN**, or **efficiency idle · i3**  
5. [x] Record Disk Lock Soft/Hard if shown; Event log / throttle reasons (`*_efficiency_idle`)  
6. [x] Desktop usable? (mouse/keyboard within ~1s) — **idle yes; new process may stutter under ~1 s queue**  
7. [x] Stop hog; within ~30–60s control → released / soft restores (or TTL ≤45s)  
8. [x] No Explorer / Cursor / whitelist in suspended list — Soft-only expected  

## Run log

### Run 1 — WD Green boot volume cliff (2026-07-19)

| Field | Value |
|-------|--------|
| Date (UTC) | ~2026-07-19 (local soak) |
| Machine | `XTZJ-20221014TG` — ~16 GB RAM (~12.6 GB / 79% in use during soak) |
| OS drive | **SATA SSD** WDC WDS240G2G0A-00JH30 (WD Green 240 GB) — **System + page file** (Disk 1 C: E:) |
| Unstick version | **0.5.0** LIVE |
| Hog | Sustained OS-volume load (cliff / equivalent); Active Time pegged |
| TEMP on OS volume? | yes (hog targets TEMP on OS volume) |
| Peak DISK Active Time (TM) | **100%** sustained (~60s graph flat) |
| Peak Avg response time (TM) | **~948 ms** (severe latency cliff) |
| Transfer rates (TM) | Read ~140 KB/s · Write ~5.4 MB/s (busy ≠ throughput) |
| Peak Guard DISK % | **100%** |
| Peak pressure / band | **EMERGENCY 0.77** |
| Tripwire text | **`disk_busy_hard — soft capping · disk i2`** |
| Disk/RAM cap chips | Soft capping intensity **2** (max Soft ladder) |
| Event log capped? | Not captured in screenshots |
| Desktop usable during hog? | **Idle / no input: no full freeze.** **Launching apps / screenshot (new process): temporary freeze/stutter**, recovers — not a prolonged hang or crash |
| After stop: released? | _Closed in Run 2_ |
| Notes / FP | NVMe Disk 0 idle; bottleneck is **slow OS+pagefile SATA SSD**. Soft i2 engaged at cliff. New-process I/O still queues behind ~1s response time. |

### Run 2 — WD Green + Idle-under-stress (2026-07-19)

| Field | Value |
|-------|--------|
| Date (UTC) | **2026-07-19T11:48Z** |
| Machine | `XTZJ-20221014TG` — same WD Green boot volume as Run 1 |
| OS drive | **SATA SSD** WDC WDS240G2G0A-00JH30 (System + page file) |
| Unstick version | **0.5.0 tree + Idle-under-stress (I1–I3)** release service LIVE |
| Hog | `disk_hog cliff` (2048 MiB / 300s preset); stopped ~35s after **i3** held ~20s |
| TEMP on OS volume? | yes |
| Peak Guard DISK Active % | **100%** |
| Peak disk latency (`status.json`) | **~701 ms** |
| Peak pressure / band | **EMERGENCY ~0.90** (`disk_busy_hard`) |
| First capping | **~3 s** → disk i1 |
| First i2 | **~6 s** |
| First **i3** | **~9 s** — reasons include **`disk_control:efficiency_idle`** (+ ecoqos / io_verylow) |
| Tripwire / intensity | Sustained **`disk_busy_hard`** with **disk_control_intensity = 3** (Efficiency Idle) |
| Disk/RAM chips (status) | Disk capping/holding at **i3**; mem i0 |
| Event / throttle reasons | `disk_control:efficiency_idle`, `disk_control:ecoqos`, `disk_control:io_verylow` |
| Desktop usable during hog? | **No prolonged hang** — soak poller kept reading `status.json` throughout; Soft-only **`suspended: []`**; no Explorer/Cursor FP |
| Launch stutter honesty | Peak latency still **hundreds of ms**; short notepad pulse under 100% busy (low latency sample) ~**0.8 s** create — **does not claim zero stutter** under ~1 s OS queue (Run 1 still applies) |
| After stop: released? | **Yes** — hog stop ~35 s; **`released` / i0 at ~74 s** (~39 s after stop; within soft restore / ease window) |
| Capture artifact | [`hardware-control-l3-cliff-run2-capture.json`](hardware-control-l3-cliff-run2-capture.json) |

## Conclusion (Run 1)

| Question | Answer |
|----------|--------|
| Soft capping under sustained load? | **Yes** — `disk_busy_hard — soft capping · disk i2` while DISK 100% / EMERGENCY |
| Full OS freeze / crash? | **No** — idle desktop stays up; Soft path did not hard-lock the machine |
| Interactive fluidity under cliff? | **Partial fail** — new process / screenshot causes **temporary freezes** while avg response ~948 ms |
| Soft max intensity enough? | **Insufficient for WD Green cliff** — i2 EcoQoS/I/O demotion cannot create latency headroom when the OS volume itself is saturated at ~1s response |
| Setpoint change from this run? | Band/tripwire fired correctly; problem is **actuator depth vs drive cliff**, not “too late sensing.” Consider 0.6 Idle-under-stress or stronger I/O demotion **only with TTL**; do not claim zero stutter |
| Blockers for public trust | Honest claim: **mitigates hard lockup risk / soft-caps offenders**; **does not eliminate** launch stutter on saturated slow OS SSDs with pagefile |

## Conclusion (Run 2)

| Question | Answer |
|----------|--------|
| Efficiency Idle (i3) under sustained cliff? | **Yes** — intensity **3** + `disk_control:efficiency_idle` within ~9 s of cliff hog |
| Full OS freeze / hang FP? | **No** — Soft-only; no shell/IDE in suspended list; service stayed responsive |
| Soft restore after hog stop? | **Yes** — released ~39 s after stop |
| Launch stutter eliminated? | **No** — honesty unchanged: under high OS-disk latency, new-process hitch may remain |
| Ready for v0.6 ship proof? | **L3 gate for I4 met** — proceed **I5** version bump + RELEASE |

## Implications (product)

1. **Detection path works** on the target WD Green soak machine — sensing → soft capping at i2 (Run 1) → **Efficiency Idle i3** (Run 2).  
2. **User-mode Soft cannot outrun a ~0.7–1 s OS-disk queue** for cold starts (exe map, page-ins). That matches mission honesty (freeze *mitigation*, not zero hitch).  
3. Idle-under-stress deepens Soft actuators with streak + TTL; it does **not** replace a faster OS volume.  
4. Run 1 “after stop” restore gap **closed** in Run 2.
