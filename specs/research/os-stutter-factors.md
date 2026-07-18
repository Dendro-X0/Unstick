# OS stutter & responsiveness factors (Windows · Linux · macOS)

Research note for Unstick roadmap. Sources are official Microsoft Learn, Linux Kernel Docs, and Apple Developer documentation (plus WWDC Instruments guidance).

## Thesis

System stutter is usually **wait time**, not peak utilization:

| Stall class | What the user feels |
|-------------|---------------------|
| Scheduler / Ready | Laggy UI while other threads run |
| Interrupt / softirq | Glitches with “normal” CPU% |
| Memory → disk | Whole-desktop hitch on faults/reclaim |
| Block I/O saturation | Freeze while Active Time / queue climbs |
| Thermal / power | Sudden FPS drop without a single hot process |

Official tooling measures **stalls** (WPA Ready & DPC/ISR, Linux PSI/delayacct, Instruments hangs). Unstick today mostly scores **utilization proxies** (CPU%, disk busy%, commit%, hard faults) and remediates with priority / I/O / WS / optional Suspend.

## Windows (Microsoft Learn)

### CPU & dispatcher
[CPU Analysis](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/cpu-analysis): limited CPUs → time-sharing; Ready threads wait; context switches by priority, affinity, quantum. **Interference** from unrelated busy threads delays critical-path work.

**Unstick:** focus AboveNormal boost + progressive BelowNormal/Idle on offenders addresses this class.

### DPC / ISR
Same doc: ISRs must be short; DPCs run **before any thread**. Excess DPC/ISR time starves high-frequency work (video, animation, audio).

**Unstick gap:** user-mode cannot shorten driver DPCs. Detect-only ETW advisory is the honest product response.

### Hard page faults & commit
[Page file guidance](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/how-to-determine-the-appropriate-page-file-size-for-64-bit-versions-of-windows): hard faults resolve from disk (DLLs, mapped files, pagefile). High Pages/sec can cause **system-wide delays**; correlate with disk hosting the pagefile. Commit charge cannot exceed commit limit.

**Caveat** ([phantom hard faults](https://learn.microsoft.com/en-us/archive/blogs/clinth/the-case-of-the-phantom-hard-page-faults)): mapped-file I/O also increments Pages/sec — not always RAM shortage.

**Unstick:** commit ≥95% tripwire; hard faults from PDH Pages/sec; mapped I/O discounted via Page Writes + Paging File % (see `hard-fault-cause-split-design.md`). Disk latency via Avg. Disk sec/Transfer.

### Disk
PAL / Hyper-V checklists: Pages/sec thresholds and disk queue/latency matter together. Queue length alone is weaker than **service time**.

**Unstick:** PDH PhysicalDisk Active Time + adaptive Disk Lock. Still missing latency-based tripwire.

## Linux (Kernel Docs)

### PSI
[`/proc/pressure/{cpu,memory,io}`](https://docs.kernel.org/accounting/psi.html): `some` = ≥1 task stalled; `full` = all non-idle stalled (thrashing). Triggers via `poll()` for load-shedding.

**Unstick (Linux port):** PSI should be the primary pressure band input — closer to “felt latency” than raw busy%.

### Delay accounting
[Delay accounting](https://docs.kernel.org/accounting/delay-accounting.html): per-task waits for CPU, sync blkio, swapin, reclaim, thrash, compact, IRQ/SOFTIRQ. Explicitly intended to feed priority / io priority / rss limit decisions.

**Unstick:** maps 1:1 to offender ranking and soft remediation.

## macOS (Apple)

### QoS
[Energy Efficiency — task QoS](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/PrioritizeWorkAtTheTaskLevel.html): QoS drives scheduling, CPU/I/O throughput, timer latency. Mis-labeled background work fights interactive work.

**Unstick (macOS):** focus profile → elevate User Interactive / User Initiated; demote Background / Utility.

### App Nap & activities
[App Nap](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/AppNap.html): system already throttles inactive apps’ CPU/I/O/timers. Prefer cooperating with Nap / `ProcessInfo` activities over Suspend-like pauses.

### Thermal
[Thermal states](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/RespondToThermalStateChanges.html): Fair → reduce proactively; Serious/Critical → cut CPU/GPU/I/O/frame rate to minimum for user actions.

### Hangs
WWDC Instruments: main-thread work &gt; ~100ms → hang (busy vs blocked). Complementary to system-freeze Guard.

## Improvement backlog (from this study)

| Pri | Item | Why | Status |
|-----|------|-----|--------|
| P0 | Disk **latency** sensor + tripwire | Official: pair faults with transfer time | **Done** — `disk_latency_sec` / soft 15ms / hard 40ms |
| P0 | Hard-fault **cause** split (pagefile vs mapped) | Avoid false Emergency | **Done** — Pages/sec + Page Writes + PF%; mapped discount |
| P1 | ETW DPC/ISR **advisory** (no fake fix) | Documented stutter Unstick cannot remediate | **Done** — PDH DPC/IRQ % + detect-only advisory |
| P1 | Abstract pressure as **stall fractions** (PSI-shaped) | Portable model; Linux-ready | **Done** — `StallFractions` + `stall_*` status |
| P2 | Thermal / power axis | Throttle ≠ offender CPU | **Done** — cooling/mhz/battery + Suspend suppress |
| P2 | macOS QoS + App Nap ladder | Official control plane | **Done** — portable `plan_qos` + `guardian-mac` stubs |

## Out of scope for user-mode Guard

- Kernel IOPS QoS / storage stack redesign
- Fixing bad third-party drivers’ long DPCs
- Guaranteeing hard real-time deadlines
