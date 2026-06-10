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

## Stage 5 outcome (2026-06-09): `m3_scy_change` is a lockstep ceiling

Stages 1–4 landed behavior-preserving (sub-dot clock, CpuAccessPlan, the
`PpuPhaseTrace` instrument, the named `MODE3_FETCH_START_DELAY_DOTS` lever). The
instrument then **disproved** that stage 5 is reachable by write-commit timing:

- The BG `TileNo` Y-latch for the first wrong tile and the colliding mid-mode-3
  `SCY` write share the **same `dot_ticks`** (the same `ppu.tick_dot()` call),
  but the write commits *after* the fetch within that dot.
- Four commit-timing levers were falsified, each reverted clean with the crown
  jewels intact: DMG LOW/HIGH read split (8819→9503 + `tile_sel` regressed),
  commit-before-`tick_cpu_t` in `step_t` (no-op), threading the write into
  `tick_cpu_t` before `ppu.tick_dot` (no-op), and `write_drive_ticks(SCY)=1`
  (8819→9069 — reproducing a previously-documented dead end).

Root cause: a CPU-instruction-vs-PPU-dot **phase offset**. The `step_m →
step_m_via_t → step_t → write_m` engine places the `m3_scy_change` write burst
about one dot/instruction late relative to the fetch that should see it. No
commit-T within the writing instruction's own M-cycle moves the value across the
fetch dot. The honest fix is a true timestamped event scheduler that orders
write-commit vs PPU-tick at sub-dot granularity *decoupled from instruction
M-cycle boundaries* — materially larger than stages 1–5 and deferred.

**Decision:** `m3_scy_change` (and its same-mechanism siblings) stay gated at
baseline (no regression), documented as the architecture ceiling. The stage 1–3
scaffolding + the trace instrument are kept as a net improvement and the tool
that proved this. Remaining mealybug facets that do *not* write `SCY` mid-mode-3
(palette/window/obj/tile classes) and `oam_bug` subtest 7 may have independent,
fixable mechanisms and are assessed separately.

## Stage 6 outcome (2026-06-09): the near-zero facets are not independently winnable

After the stage-5 ceiling, the near-zero DMG facets were investigated as
potentially-independent bugs (a `PpuPhaseTrace`-backed per-line/per-pixel
localiser was added). Each was empirically falsified or proved coupled, all
reverted clean with the crown jewels at 0/0:

- **`m3_lcdc_obj_size_change`** (150px, sprite-fetch path): hypothesis was a
  too-late `LCDC.2` (OBJ height) sample. A latch sweep showed the opposite —
  fetch-start=190, mid-fetch=170, load (current)=150. Sampling *earlier*
  monotonically worsens it, so the load-time sample is already optimal; the
  residual is the same cross-actor CPU-write-vs-fetch race as `m3_scy_change`.
- **`m3_window_timing`** (103px, window-FIFO path): genuinely a distinct
  mechanism (the ROM writes WX 0..7 across LY0-8; the reference wants a stable
  3-leading-zero window start). But the fix (cancel the WX-derived
  `scx_discard` at the off-edge window start) regressed every sibling window
  facet — `m3_wx_4_change` 3077→9800, `m3_window_timing_wx_0` 1346→2783,
  `m3_lcdc_win_en_change_multiple_wx` 952→970. The variable `scx_discard` is
  load-bearing across the coupled window model; it cannot be fixed in isolation.

**Verdict (Oracle, binding):** the entire mid-mode-3 facet class is either the
lockstep cross-actor ceiling or a coupled-model problem where an isolated fix
regresses siblings. None is independently winnable under the current
architecture. The honest, no-fake-passes outcome is to gate the class at
baseline (exact diffs visible) and pursue the CPU/PPU co-scheduler (`rubc-t2qr`)
as a deliberate, separately-reviewed future effort — not an autonomous rewrite
that risks the shipped pixel-exact crown jewels.

## Stage 5b outcome (2026-06-09): run-ahead co-scheduler + T1/T2 split locate the wall exactly

After the stage-5 lockstep ceiling, the **CPU-run-ahead / PPU-lag** half of the
co-scheduler was actually built (stages S1–S4 + the run-ahead crux): the CPU
advances ahead positing timestamped PPU-visible writes into `pending_ppu_writes`;
the PPU lags and drains them at their dots via `sync_ppu_to` / `drain_ppu_writes_through`.
This is the decoupled-from-M-cycle scheduler stage 5 said was needed, and it
shipped behavior-preserving:

- **`m3_scy_change` 8819 → 3497** (60% better), zero regression, crown jewels 0/0,
  full gate green — all on `rearch/sub-dot-scheduler`, master untouched.
- The residual used a per-phase write-drain with a scalar `dots_after_start = 5`
  (+20 subphases) future-drain — a band-aid, not the hardware model.

The **principled T1/T2 BG-fetch split** (Oracle mechanism-E) was then implemented
faithfully — split each data fetch into T1 (latch address from live SCY) + T2
(read VRAM from latched address), matching SameBoy `display.c`
`GET_TILE_DATA_LOWER/HIGH_T1/T2`, draining SCY at the exact T1 phase-time. It did
**not** reach 0; it regressed (3497 → 9219) but **located the wall precisely**:

- Wall trace, LY0 tile `0x42`: LOW samples `SCY=2` at dot 96, HIGH samples
  `SCY=2` at dot 98, but the `SCY=3` write lands at **dot 103** — ~5 dots *after*
  the HIGH fetch. SameBoy's HIGH sees `SCY=3`.
- The write timestamp is `M_cycle_start + write_drive_ticks*4 subphases`; for SCY
  that lands at dot 103, so the **writing M-cycle starts ~dot 101** while the
  fetch that must see it ran at dot 98. Tuning the intra-M-cycle offset (V4 sweep
  T0=9059/T1=8811/T2=9219/T3=10015) cannot cross dot 98.
- The `+5` band-aid “worked” only by future-draining a not-yet-due write back onto
  an earlier fetch (violating time); it is wrong for other fetch positions (hence
  the 3497 residual, first-wrong pixel x=39).

**Verdict (Oracle, HIGH confidence, binding):** verdict **B — current
co-scheduler phase ceiling**, not hardware-impossible. The missing 5 dots are the
**CPU instruction-stream phase relative to PPU fetch time**, and rubc has no local
“CPU release into mode 3” lever (CPU execution is continuous, not released
per-mode). The only lever that moves the write-burst 5 dots is global
interrupt/HALT/STAT phase, which would regress the passing mooneye intr/STAT
gates. `PPU_MIN_LAG_T` / `PPU_MAX_LOOKAHEAD_T` cannot fix it (the watermark
changes when the PPU catches up, not the event timestamps). The DMG low-phase
tile refetch is **load-bearing** (quarantining it regressed `m3_lcdc_tile_sel_change`
1755→1790) and is kept.

**Decision:** gate `m3_scy_change` at the improved baseline **3497** (the
co-scheduler win is kept), documented as the current co-scheduler phase ceiling.
Reaching 0 requires repositioning the whole CPU instruction stream relative to the
PPU dot clock during mode 3 — a full co-scheduler rewrite (`rubc-t2qr`) that
touches globally-validated STAT/interrupt timing, pursued separately, never as an
autonomous change that risks the pixel-exact crown jewels.
