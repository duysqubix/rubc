# AGENTS.md — rubc-ng (the timing core)

The sole emulation core (old `rubc-core` retired). `#![forbid(unsafe_code)]`,
pure Rust, no C/`-sys`/FFI. Architecture: [ADR 0002](../../docs/adr/0002-rubc-ng-timing-core-rebuild.md).

## ARCHITECTURE (one screen)
- **Hybrid per-T spine.** CPU steps one M-cycle = 4 T-cycles; the `ClockSpine`
  ([time.rs](time.rs)) tracks `now` in *subphases* (4/T, 16/M). Memory is
  observable at individual T-positions.
- **CPU emits bus intents** ([bus_intent.rs](bus_intent.rs)) — `ReadSample` /
  `WriteDrive` / `Idle` / `IntrPoll`; the spine applies them at table phases.
  **The CPU never ticks peripherals.**
- **Declarative `TimingTable`** ([timing.rs](timing.rs)): every observable is a
  named `(Anchor, offset, PhaseRule)` entry. THREE INDEPENDENT profiles —
  `PpuPublicTiming` (LY/STAT/mode/IRQ), `PpuInternalTiming` (fetcher/FIFO/
  sprite/window), `OutputLatchTiming` (LCD column latch + palette). Public
  edges are NOT derived from internal pixels — coupling is unrepresentable.
- **`GbModel` day-one** ([model.rs](model.rs)): 13 variants (Dmg0/A/B, Mgb,
  Sgb/Sgb2, Cgb0/A/B/C/D/E, Agb). "100%" = every ROM passes on its INTENDED
  model. Boot profiles + timing tables are model-parameterized.

## MODULE MAP
| File | Responsibility |
| :--- | :--- |
| [lib.rs](lib.rs) | `forbid(unsafe_code)`; public re-exports |
| [machine.rs](machine.rs) | `MachineNg`: `from_rom`/`boot_dmg`/`boot_cgb`/`boot_cgb_native`, `step_frame`, `framebuffer`, `set_button`, `drain_samples`, `save_state`/`load_state` (v3), `debug_*`; `MachineBus`; `FramePixel`, `Button`, `RunStopNg` |
| [time.rs](time.rs) | `Time` (subphases), `ClockSpine`, `ClockPhase` (CpuT0-T3) |
| [timing.rs](timing.rs) | `TimingTable`, `TimingEntry`, `Observable`, `Anchor`, `PhaseRule`, `TimingDomain` |
| [bus_intent.rs](bus_intent.rs) | `CpuBusIntent`, `IntentOutcome`, `CpuIntentSource` |
| [model.rs](model.rs) | `GbModel`, `is_cgb`/`is_dmg_family`, `priority_name` |
| [ppu_public.rs](ppu_public.rs) | `PpuPublic` — LY/STAT/mode/IRQ public timeline |
| [ppu_internal.rs](ppu_internal.rs) | `PpuInternal` — BG fetcher, FIFO, sprites, window |
| [output_latch.rs](output_latch.rs) | `LcdOutputLatch` — column latch + palette sampling |
| [pixel_fifo.rs](pixel_fifo.rs) | FIFO renderer (private) |
| [apu.rs](apu.rs) | `Apu`: 4 channels + 512 Hz frame sequencer + stereo `drain_samples` |
| [timer.rs](timer.rs) | DIV/TIMA/TMA/TAC, edge detection, double-speed |
| [cartridge.rs](cartridge.rs) | `Cartridge`: MBC0/1/2/3+RTC/5, battery RAM |
| [golden.rs](golden.rs) | `GoldenTrace`/`GoldenRow` TSV oracle + `assert_*_golden` macros |
| [conformance.rs](conformance.rs) | `ConformanceReport`; `FULL_MANIFEST_PASS_FLOOR = 157` |
| [manifest.rs](manifest.rs) | `Manifest`/`RomManifestEntry`/`Expectation` (per-ROM intended models + pass kind) |
| [serde_arrays.rs](serde_arrays.rs) | macro serde for large VRAM/WRAM arrays (savestate) |
| [cpu/](cpu/) | SM83: `core.rs` (per-M stepping), `opcodes.rs`+`opcodes_cb.rs` (256+256), `alu.rs`, `regs.rs`, `scheduler.rs` (per-T windows) |

## CONVENTIONS
- **Golden-trace discipline (binding).** Every subsystem is gated against an
  INDEPENDENT oracle — golden trace ([golden.rs](golden.rs)), spec rule, or
  real test-ROM pass signature. Goldens live under git-ignored
  `reference/goldens/`, reproducible from `instrumentation.patch` vs SameBoy.
- **Savestate v3** (`SAVESTATE_VERSION = 3`): serde_json; `TimingTable` is
  `#[serde(skip)]` and rebuilt from model on load.
- **Conformance floor RATCHETS up, never down.** 157/207; full gate via
  `RUBC_NG_CONFORMANCE_FULL=1`. Universal ROMs scored on HIGHEST-priority
  intended model (CGB debt is not laundered behind DMG passes).

## ANTI-PATTERNS (ADR 0002 — rejected at review)
- **No self-reference.** A test copying `actual` from the golden and comparing
  to itself is REJECTED. The machine must PRODUCE every asserted value from its
  own state + inputs. Every gate must FAIL under perturbation (1-tick shift /
  wrong register) with a first-divergence diagnostic.
- **No silent golden fallback** when data is absent — absence is a capture gap
  to FIX, not a reason to weaken the assertion.
- **No ROM-name special cases.** Tables are model-parameterized, not ROM-specific.
- **No timing constant without golden provenance.** No future-draining writes.
- **No deriving public PPU edges from internal pixels.**

## TESTS (rubc-ng/tests/)
`conformance_matrix.rs` (157 floor) · `sm83_vectors.rs` (436) ·
`mooneye_cpu_timing.rs` · `mooneye_ppu_public_timing.rs` ·
`framebuffer_conformance.rs` (acid2/mealybug/cgb-acid-hell pixel-diff) ·
`w2_bg_fetcher.rs` / `w3_output_latch.rs` / `w4_window_sprite.rs` /
`w8b_pixel_fifo.rs` (golden-gated, skip-clean without `reference/`).
Run: `just ng-test` · `just ng-goldens` · `just check`.
