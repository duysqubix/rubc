# rubc

A cycle-accurate **Game Boy (DMG)** and **Game Boy Color (CGB)** emulator written
in safe, dependency-light Rust.

The goal is correctness first: rubc is built and verified against the
[Blargg `gb-test-roms`](https://github.com/retrio/gb-test-roms) and
[Gekkio `mooneye-test-suite`](https://github.com/Gekkio/mooneye-test-suite)
hardware test suites, with an M-cycle-stepped CPU driving T-cycle-accurate
peripherals.

> **Status:** under active development. The CPU and core timing are complete and
> test-passing; graphics (PPU) and audio (APU) are in progress. See
> [Status](#status) for the detailed breakdown.

---

## Highlights

- **100% safe Rust.** The core library is `#![forbid(unsafe_code)]` — no
  `unsafe`, no C bindings, no `-sys` crates, no FFI.
- **Cycle-accurate by construction.** The CPU advances one **M-cycle** at a time;
  the bus ticks peripherals **4 T-cycles** per M-cycle and samples the bus
  *after* they advance (tick-then-sample), so memory and timer behaviour matches
  hardware ordering.
- **DMG + CGB from the ground up.** Dual-mode is a first-class target, not a
  retrofit.
- **Built-in self-diagnosis.** A feature-gated diagnostics layer (flight
  recorder, BGB-format trace, state hashing, metrics, machine snapshots) makes
  failures reconstructable from artifacts alone — and compiles to zero cost when
  disabled.

## Status

| Subsystem | State | Verified by |
|-----------|-------|-------------|
| **SM83 CPU** | ✅ Complete | All 512 opcodes pass [SingleStepTests](https://github.com/SingleStepTests/sm83) vectors (state + cycle count + IME/EI) |
| **Instruction timing** | ✅ Per-opcode M-cycle counts | M-cycle count harness (branch taken/not-taken, 5-M interrupt dispatch) |
| **Memory bus** | ✅ M-cycle invariant | OAM-DMA beat → 4×tick → tick-then-sample latch |
| **Timer (DIV/TIMA/TMA/TAC)** | ✅ Complete | Falling-edge detection, reload state machine, write quirks; Blargg `cpu_instrs` + unit gates |
| **Interrupts** | ✅ Edge-accurate | EI 1-instruction delay, HALT bug, `ie_push` cancel, dispatch priority; Blargg `02-interrupts` |
| **MBC0 / MBC1** | ✅ ROM banking | All 11 Blargg `cpu_instrs` ROMs pass through the banked path |
| **CPU instructions (Blargg)** | ✅ 11/11 individual ROMs | `just cpu-roms` |
| **PPU (graphics)** | 🚧 In progress | — |
| **APU (audio)** | ⬜ Planned | — |
| **CGB extras** (double-speed, palettes, banking, HDMA) | 🚧 In progress | — |
| **MBC2 / MBC3 / MBC5** | ⬜ Planned | — |

The full Blargg + mooneye coverage plan is tracked as issues (see
[Issue tracking](#issue-tracking)).

## Quick start

Requires a recent stable Rust toolchain and [`just`](https://github.com/casey/just).

```sh
# Build everything
just build

# Run a ROM (quiet)
just run path/to/game.gb

# Run a ROM with debug logging
just trun path/to/game.gb

# Run the unit-test suite
just unit-test
```

Plain Cargo works too:

```sh
cargo run -p rubc -- path/to/game.gb
cargo test -p rubc-core
```

## Project layout

```
rubc/
├── rubc-core/          # The emulator library (core logic, no rendering)
│   └── src/
│       ├── cpu/        # SM83 core: step_m state machine, all 512 opcodes, ALU
│       ├── bus/        # M-cycle bus, CpuBus trait, timer, cartridge/MBC, FlatBus
│       ├── diag/       # Feature-gated diagnostics (flight recorder, trace, hash)
│       └── machine.rs  # Bootable Machine{Cpu,Bus} runner + test-ROM harness
├── rubc/               # The binary: windowing + rendering + (eventual) GUI
│   └── src/
└── justfile            # Operational backbone (build / test / diagnose recipes)
```

### Architecture in one line

The CPU and bus are **siblings** joined by the `CpuBus` trait: the CPU borrows
the bus `&mut` for exactly one `*_m` call per M-cycle. No peripheral holds a
back-reference, which keeps the design borrow-checker-clean and free of
interior-mutability hacks.

## Testing

rubc is developed test-first against real hardware test ROMs.

```sh
just unit-test        # cargo test -p rubc-core
just test-opcodes     # SM83 JSON opcode vectors (SingleStepTests)
just cpu-roms         # all 11 Blargg cpu_instrs individual ROMs
just machine-test     # machine integration tests (serial capture, signatures)
just regression-test  # Blargg cpu_instrs via serial
just check            # fmt-check + clippy (-D warnings) + build + test
```

### Hardware test ROMs

Reference docs and test suites are expected under `reference/` (git-ignored).

- **Blargg `gb-test-roms`** ship prebuilt `.gb` files and run directly.
- **Mooneye** ships [WLA-DX](https://github.com/vhelin/wla-dx) assembly source
  (`.s`), **not** RGBDS and not prebuilt ROMs. Build them with WLA-DX:

  ```sh
  brew install wla-dx      # provides wla-gb + wlalink
  just mooneye-build       # assembles the suite to <suite>/build/**/*.gb
  just mooneye 'acceptance/timer/*'   # build (if needed) + run a glob
  ```

## Diagnostics

The core ships an opt-in, zero-cost-when-off diagnostics layer for debugging hard
timing bugs. It *observes* — it never participates in emulation timing.

```sh
just diag path/to/rom.gb   # full AFK artifact bundle into the diag dir
just diag-summary          # summarise the most recent run
```

Cargo features: `diagnostics`, `flight-recorder`, `metrics`, `trace`, `hash`,
`snapshot`, and `diag-full` (all of the above). Default features are empty, so a
normal build pays nothing.

## Issue tracking

This repo uses [beads (`bd`)](https://github.com/gastownhall/beads) for durable,
dependency-aware task tracking:

```sh
bd ready     # what's available to work on
bd show <id> # issue detail
bd stats     # project health
```

## License

See repository for license information.