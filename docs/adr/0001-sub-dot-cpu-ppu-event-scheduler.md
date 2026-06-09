# ADR 0001 — Sub-dot CPU/PPU event scheduler

- **Status:** Accepted (in progress, branch `rearch/sub-dot-scheduler`, off `v0.1.0`)
- **Tracking:** `rubc-9kf` (epic); supersedes the documented ceiling on `rubc-7si`
- **Date:** 2026-06-09

## Context

The mealybug-tearoom mid-mode-3 test ROMs write `SCY`/`BGP`/`LCDC`/`WX`
*mid-scanline*, during the few dozen dots of PPU mode 3 while the background
fetcher is mid-fetch. The remaining ~15 failures all share one root cause: the
exact dot (and sub-dot phase) at which a CPU register write becomes visible to a
**concurrent** background fetch.

rubc's current model is strict lockstep: each CPU T-cycle calls `ppu.tick_dot()`
exactly once (1 dot / T), and CPU writes "drive" at a per-register T position via
`write_drive_ticks(addr)`. This cannot express the sub-dot ordering hardware
exhibits.

### What is already proven (do not re-attempt)

~10 implementation attempts + 5 Oracle consults, all reverted clean with zero
regressions. **Falsified:** scalar `write_drive_ticks` tuning (SCY T=2 is the
scalar optimum); T1-vs-T2 fetcher address latch (regresses
`m3_lcdc_bg_map_change` 845→1491); 1-T `READ_NEW` write borrow (the real gap is
~3 **dots**, not 1 T); 1-T palette overlay (helps bgp 820→403 but regresses
sprites); commit-before-tick reordering (no-op); `DMG_OUTPUT_LATENCY` sweep.

**Decisive finding:** for `m3_scy_change` tile `0x42` LY0, SameBoy's HIGH-bitplane
fetch reads `SCY=3` (newer) while rubc reads `SCY=2` (older) — rubc's mode-3
fetch sampling sits ~3 dots off from hardware *relative to mode-3 entry*. The bug
only appears when a register write **races** a fetch; `acid2`/`cgb-acid-hell` do
not write registers mid-mode-3 and are pixel-exact under the current model.

## Decision

Adopt a **hybrid timestamped phase scheduler**, not a heap-backed general event
queue.

- Game Boy timing is mostly periodic and deterministic; a fixed sub-dot **phase
  calendar** gives explicit ordering without allocating/sorting millions of
  events per second (keeps real-time perf).
- `Bus` remains the **sole owner** of CPU/PPU/APU/timer state. `Cpu` still borrows
  `&mut dyn CpuBus`; the PPU never gets a back-reference (borrow-checker-clean, no
  interior mutability). Replace "tick 1 dot now" with `advance_to(Time)` over
  fixed sub-dot phases so CPU writes, PPU fetch samples, mode edges, STAT
  settling, and pixel emission have explicit ordering.
- **Time is measured in CPU-T subphases** (`SUBPHASES_PER_T = 4`), *not* PPU dots
  — so CGB double-speed parity is preserved (PPU dots fire every T at normal
  speed, every second T at double speed, matching today's `t_phase`).

### Root-cause hypothesis (what the calibration stage targets)

The CPU write timing is roughly correct relative to instruction execution; the
**BG fetcher's internal sampling starts ~3 dots too early relative to hardware
mode-3 transfer**. The existing `MODE2_DOTS + 3` fudge in
`public_mode3_end_dot()` (`ppu.rs`) is applied at the wrong place. Treat mode-3
entry as a visible STAT/mode edge first, then internal BG fetch sampling after a
3-dot startup phase, then the documented 12-dot dummy-fetch/fill. **Do not** blame
interrupt dispatch (a global 3-dot dispatch error would make many CPU/STAT/timer
tests fragile — they pass). **Do not** retune `write_drive_ticks`.

## Hard invariants (must hold at every stage)

1. **Crown jewels stay 0-regression:** `dmg-acid2` 0/0, `cgb-acid2` 0/0,
   `cgb-acid-hell` 0/0, blargg 21/21, mooneye ppu 12/12 + timer 13/13, SM83
   436/436, 644 lib tests green.
2. **Visible pixel emission is unchanged** for ROMs that do not write mid-mode-3:
   preserve `lcd_x`, framebuffer write order, sprite-overlay order, and the
   HBlank entry dot.
3. **STAT IRQ** stays rising-edge-equivalent until the explicit calibration stage.
4. Pure safe Rust, no unsafe, no interior-mutability hacks.

## Staged plan (each stage independently gated)

1. **Behavior-preserving sub-dot clock skeleton** — `bus/scheduler.rs`: `Time(u64)`,
   `SUBPHASES_PER_T = 4`, fixed phase labels. `tick_cpu_t` still advances
   timer/serial before PPU/APU exactly as today.
2. **Timestamped CPU access plans** — replace ad-hoc `tick(pre); write; tick(rest)`
   with `CpuAccessPlan { start, write_visible_at, read_sample_at, end }`; initial
   offsets encode *current* behavior (reads end-of-M, TAC midpoint, BGP T0,
   SCY/BGP-class at current scalar positions).
3. **PPU fetch/write phase trace diagnostics** (feature-gated) — record `time`,
   `ly`, `line_dot`, `drawing_dots`, `FetchStep`, sampled regs, tile id, low/high
   bytes, CPU-write-visible events. *Before* changing behavior.
4. **Split PPU dot into named phase events** without behavior change — mode edge,
   OAM scan sample, BG tile-no/low/high sample, push, pixel emit, STAT settle.
5. **Calibrate mode-3 fetch-start phase** against `m3_scy_change` — move the `+3`
   concept to internal fetch sampling; keep total mode-3/HBlank boundary
   unchanged. Decisive check before pixel diff: tile `0x42` LY0 → LOW samples
   `SCY=2`, HIGH samples `SCY=3`.
6. **Validate BGP/LCDC/WX race classes one at a time** — each gets a trace
   assertion before generalizing.
7. **Document + remove obsolete timing knobs** — finalize ADR, retire dead
   `write_drive_ticks` tuning paths if subsumed.

## Consequences

- Larger, riskier change than any prior attempt — mitigated by behavior-preserving
  stages 1–4 and continuous gating.
- Stays on a branch; `master` remains shippable and auto-deploys to rubc.app. The
  branch is **not merged until proven 0-regression**.
- If the calibration stages cannot close the gap without regressing the crown
  jewels, the ceiling documented in `docs/ACCURACY.md` stands and the scheduler
  skeleton (stages 1–4) is still a net architectural improvement.
