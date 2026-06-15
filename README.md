<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/media/rubc-dark.png">
  <img src="docs/media/rubc-dark.png" width="420" alt="rubc">
</picture>

</div>

<p align="center">
<a href="#building"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="Build"></a>
<a href="#why-rubc"><img src="https://img.shields.io/badge/unsafe-forbidden-blue" alt="Safe Rust"></a>
<a href="#accuracy"><img src="https://img.shields.io/badge/blargg-21%2F21-brightgreen" alt="Blargg"></a>
<a href="#license"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

> 📖 **Please read [PRELUDE.md](./PRELUDE.md) first** — a short disclaimer and the developer's story behind this project. rubc is the result of an experiment: handing an emulator project over to an AI team to see how far it could get. For contrast, check out its fully human-coded sister project, [**gobc**](https://github.com/duysqubix/gobc) — a Game Boy emulator written in Go (and not *quite* as accurate as this one 🙃) — which is what sparked the whole thing. That one was hand-written, by a human.

**rubc** is a Game Boy / Game Boy Color emulator written in safe Rust. It's built to
*play your games* — full color, real sound, battery saves, keyboard and (on the web) touch — and
it's hardware-accurate enough to prove it, rendering pixel-exact on some of the
toughest PPU test ROMs out there. Standing on the shoulders of giants — Pan Docs,
SameBoy, GBEDG, mooneye, gbdev.

[**📊 Accuracy breakdown →**](docs/ACCURACY.md) &nbsp;&nbsp;
[**📖 Usage guide →**](docs/USAGE.md) &nbsp;&nbsp;
[**🕹️ Play in your browser →**](#play-in-your-browser)

<table>
  <tbody>
    <tr>
      <td align="center">
        <img src="docs/media/crystal-intro.gif" width="220">
      </td>
      <td align="center">
        <img src="docs/media/crystal-title.png" width="220">
      </td>
    </tr>
  </tbody>
</table>

*Pokémon Crystal running on rubc — color, sound, saves, and all.*

## What it's like to play

Drop in a `.gb`, `.gbc`, or `.zip` and rubc just plays it. Game Boy Color titles
run in full color; the four-channel sound chip plays through your speakers; and
battery games like Pokémon Crystal write a `.sav` next to the ROM and pick up right
where you left off. Play on the desktop with the keyboard, or
[in your browser](#play-in-your-browser) — installable, offline, with on-screen touch
controls and gamepad support on mobile. It runs at a true ~59.7 Hz, so games feel
exactly like they did on the original hardware.

## Feature matrix

Legend: ✅ = full hardware accuracy (regression-guarded in CI) &nbsp;·&nbsp; 🟡 = partial / known limitation &nbsp;·&nbsp; ❌ = not started

| Subsystem | Status | Notes |
|---|:-:|---|
| SM83 CPU | ✅ | All 256 main + 256 CB opcodes. **Per-T-cycle** engine: one M-cycle steps four T-cycles internally, with memory observable at individual T-positions (reads latch end-T4, writes drive at per-register T). Passes Blargg `cpu_instrs` 11/11, `instr_timing`, and all 436 SingleStepTests/sm83 vectors. |
| Interrupts | ✅ | EI 1-instruction delay, HALT bug, dispatch priority, peripherals ticked during the ISR. Passes `interrupt_time` and `halt_bug`. |
| Joypad | ✅ | All 8 buttons — D-pad + A/B + Start/Select. |
| Timers (DIV/TIMA) | ✅ | Obscure-behaviour-accurate (TIMA reload, falling-edge detection, DIV-write quirks); CGB double-speed scaling. Passes mooneye `acceptance/timer` 13/13. |
| LCD / PPU | ✅ | Pixel-FIFO renderer, per-dot mode scheduler, STAT/LYC interrupt timing. **Pixel-exact on `dmg-acid2`, `cgb-acid2`, *and* `cgb-acid-hell`.** Mid-mode-3 register-race edge cases are a documented limitation — see [accuracy](#accuracy). |
| APU (sound) | ✅ | Full 4-channel emulation: 2× pulse with NR10 sweep, wave with 32-sample wave RAM, noise with 7/15-bit LFSR, frame sequencer at 512 Hz, stereo output via `cpal`. **Passes all 12/12 Blargg `dmg_sound` AND 12/12 `cgb_sound`.** |
| CGB mode | ✅ | BG / OBJ palette RAM, VRAM + WRAM bank switching, double-speed via STOP + KEY1, HDMA / GDMA, OPRI. First-class — not a retrofit. |
| Serial port | 🟡 | Output captured for test ROMs; full link-cable transfers not yet implemented. |
| Battery saves | ✅ | `.sav` written next to the ROM, auto-loaded on boot. Portable to/from other emulators. |
| Debugger | ✅ | Detachable egui window: VRAM tile sheet + tilemap, live OAM/sprite view, composited screen, scroll-tracking viewport, BG state. |
| WebAssembly | ✅ | The whole core compiles to wasm and runs entirely client-side. Native PWA front-end (installable, offline, touch + gamepad). |

### Cartridge MBC support

| MBC | Status | Notes |
|---|:-:|---|
| ROM_ONLY (MBC0) | ✅ | — |
| MBC1 (+ RAM + BATTERY) | ✅ | 1 MiB+ multicart wiring quirk out of scope. |
| MBC2 (+ BATTERY) | ✅ | Built-in 512×4-bit RAM. |
| MBC3 (+ RTC + RAM + BATTERY) | ✅ | Latch-able RTC counter (RAM persisted; wall-clock RTC persistence is future work). |
| MBC5 (+ RAM + BATTERY) | ✅ | — |
| MBC6 / MBC7 / HuC1 / HuC3 / MMM01 | ❌ | Not started. |

### Test ROM scorecard (regression-guarded in CI)

| Suite | rubc | Notes |
|---|:-:|---|
| `cpu_instrs` | ✅ **11/11** | Every sub-test. |
| `instr_timing` | ✅ PASS | |
| `mem_timing` / `mem_timing-2` | ✅ PASS | |
| `halt_bug` | ✅ PASS | |
| `interrupt_time` | ✅ PASS | DMG + CGB double-speed. |
| `dmg_sound` | ✅ **12/12** | All DMG audio quirks. |
| `cgb_sound` | ✅ **12/12** | CGB-aware quirks. |
| `oam_bug` | 🟡 **7/8** | Sub-test 7 (`timing_effect`) needs sub-dot OAM-corruption phase — see [accuracy](#accuracy). |
| `dmg-acid2` / `cgb-acid2` | ✅ **Pixel-exact** | 0 / 23040 pixels off. |
| `cgb-acid-hell` | ✅ **Pixel-exact** | Matt Currie's undocumented CGB PPU torture test — 0 / 23040. |
| SingleStepTests `sm83` | ✅ **436/436** | Per-opcode CPU acceptance vectors. |
| Mooneye acceptance (DMG-ABC + CGB) | ✅ **94/115** | Remainder are model-mutually-exclusive boot variants or the mealybug ceiling below. |
| `mealybug-tearoom` | 🟡 Partial | Hardest mid-mode-3 PPU timing class — see [accuracy](#accuracy). |

## Installing

You'll need a recent stable [Rust toolchain](https://rustup.rs). That's the only hard
requirement — the emulation core has no C dependencies, no `-sys` crates, and no FFI.

```bash
# Build from source
git clone https://github.com/duysqubix/rubc && cd rubc
cargo build --release            # optimized native build

# Or grab the task runner and let it drive
cargo install just               # or: brew install just / apt install just
just build                       # fast dev build (debug assertions on)
just check                       # full pre-commit gate: fmt + clippy + build + test
```

The project uses [`just`](https://github.com/casey/just) as its task runner. Run `just`
with no arguments (or `just help`) to list every recipe.

## Usage

`rubc` exposes a handful of subcommands plus a bare-window shorthand:

| Command | Purpose |
|---|---|
| `rubc` | Open the emulator window with no ROM — use **File → Load ROM** to pick one. |
| `rubc run ROM [options]` | Boot the emulator and run a ROM directly. |
| `rubc cartdump ROM [--raw]` | Dump the cartridge header (and optional opcode disassembly). |
| `rubc screenshot ROM --out PNG` | Render headlessly and capture a frame. |
| `rubc gif ROM --out GIF` | Capture an animated GIF. |
| `rubc controls` | Print the key mapping. |

```bash
# Play a game
cargo run --release -p rubc -- run path/to/game.gbc
cargo run --release -p rubc -- run path/to/game.gb --force-cgb   # force CGB on a DMG ROM
cargo run --release -p rubc -- run path/to/blargg.gb --no-gui    # headless (CI / test ROMs)
LOG_LEVEL=debug cargo run -p rubc -- run path/to/game.gbc

# Inspect a cartridge
cargo run -p rubc -- cartdump path/to/game.gbc
cargo run -p rubc -- cartdump --raw path/to/game.gbc

# Or use the just recipes
just run path/to/game.gb         # run quietly
just trun path/to/game.gb        # run with debug logging
just tour ppu                    # cycle the PPU test ROMs in the GUI
```

Run `rubc --help` for the full flag reference and `rubc <command> --help` for per-subcommand help.

### Drag-and-drop and `.zip`

You can drop a `.gb`, `.gbc`, or `.zip` straight onto the window — a zip is auto-extracted
and the first ROM inside is loaded.

## Play in your browser

### ▶ [**rubc.app**](https://rubc.app) — play it now, no install required

Live at **[rubc.app](https://rubc.app)**. Open it on your phone and *Add to Home Screen*
to install the PWA — offline-capable, with on-screen touch controls and gamepad support.

The whole core compiles to WebAssembly and runs entirely client-side — your ROM never
leaves your machine. There's a mobile-first PWA front-end (installable, offline-capable,
with touch controls and Gamepad support).

```bash
# Native dev build of the wasm demo
rustup target add wasm32-unknown-unknown
just wasm-build                  # -> rubc-wasm/web/pkg/
just wasm-serve                  # http://localhost:8000/

# Or the full PWA + nginx, no toolchain needed beyond Docker
docker compose up --build        # then open http://localhost:8080/
```

Saves live in the browser's IndexedDB, keyed per-ROM, with manual `.sav` export / import.
See the [**usage guide**](docs/USAGE.md) for the full walkthrough of all three ways to run
rubc (native, browser, Docker), saves, and troubleshooting.

## Controls

| Key | Game Boy button |
|---|---|
| Arrow keys | D-pad |
| <kbd>X</kbd> | A |
| <kbd>Z</kbd> | B |
| <kbd>Enter</kbd> | Start |
| <kbd>Right Shift</kbd> / <kbd>Backspace</kbd> | Select |
| <kbd>Esc</kbd> | Quit |

On mobile (the web PWA) the same buttons are an on-screen D-pad + face-button overlay, and
Bluetooth / USB controllers work through the Gamepad API. Run `rubc controls` any time to
print the desktop mapping.

## Accuracy

rubc is developed test-first: every subsystem is gated against a real hardware test ROM
before it's considered done. The full per-ROM breakdown lives in
[`docs/ACCURACY.md`](docs/ACCURACY.md), and these screenshots are rubc's own rendered
output running each test ROM to completion:

<table>
  <tr>
    <td align="center"><img src="docs/media/tests/cpu_instrs.png" width="180"><br><sub>cpu_instrs</sub></td>
    <td align="center"><img src="docs/media/tests/instr_timing.png" width="180"><br><sub>instr_timing</sub></td>
    <td align="center"><img src="docs/media/tests/mem_timing.png" width="180"><br><sub>mem_timing</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/media/tests/dmg_sound.png" width="180"><br><sub>dmg_sound</sub></td>
    <td align="center"><img src="docs/media/tests/cgb_sound.png" width="180"><br><sub>cgb_sound</sub></td>
    <td align="center"><img src="docs/media/tests/halt_bug.png" width="180"><br><sub>halt_bug</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/media/tests/dmg-acid2.png" width="180"><br><sub>dmg-acid2</sub></td>
    <td align="center"><img src="docs/media/tests/cgb-acid2.png" width="180"><br><sub>cgb-acid2</sub></td>
    <td align="center"><img src="docs/media/tests/cgb-acid-hell.png" width="180"><br><sub>cgb-acid-hell</sub></td>
  </tr>
</table>

> **A note on the hard stuff.** rubc renders **pixel-exact** on `cgb-acid-hell` — Matt
> Currie's deliberately-undocumented CGB PPU torture test that hammers `LCDC` mid-scanline
> and trips up most emulators. Getting the final two pixels right meant emulating an obscure
> CGB tile-select bus conflict, cross-checked dot-for-dot against SameBoy.
>
> The remaining gap is the rest of the **mealybug-tearoom** suite: it writes `SCY`, `BGP`,
> `LCDC`, and `WX` *mid-scanline* while the background fetcher is mid-fetch. A handful render
> pixel-exact; the rest are gated at their measured pixel difference so they can never
> regress. Reproducing them faithfully needs a timestamped sub-dot event scheduler — a
> different model from the cycle-stepped CPU↔PPU coupling rubc uses today (which is itself
> what makes the acid2 tests pixel-exact). These are documented as a known limitation rather
> than chased with timing hacks that would risk those passes.

## Why rubc?

- **It actually plays games.** Color, sound, battery saves, keyboard, gamepad, and touch —
  Pokémon Crystal runs start to finish, in color, at the right speed. Accuracy isn't the
  goal in itself; it's how rubc makes your games look and sound the way they should.
- **Correctness you can trust.** Every subsystem is gated against a real hardware test ROM
  before it's considered done. The CPU advances one M-cycle at a time, ticking four T-cycles
  internally; memory access is observable at individual T-cycles within each M-cycle, so
  memory and timer ordering matches the real machine.
- **DMG + CGB from the ground up.** Dual-mode is a first-class design target, not a
  retrofit — color, double-speed, VRAM/WRAM banking, and HDMA are all native.
- **Genuinely safe.** The emulation core is `#![forbid(unsafe_code)]` with no C bindings, no
  `-sys` crates, and no FFI. It builds anywhere Rust does, with no system libraries to chase.

Under the hood there's also a feature-gated diagnostics layer (flight recorder, BGB-format
trace, state hashing, snapshots) that made the hard timing bugs reconstructable from
artifacts alone. It's internal tooling — off by default and compiled to nothing — not
something you need to play games, but it's how the accuracy above got built.

## Testing

The core's unit + integration tests run against the SingleStepTests/sm83 vectors, the
WLA-DX-built mooneye suite, the acid2/mealybug pixel diffs, and the Blargg ROMs (via serial
or cart-RAM result protocol).

```bash
just test                        # cargo test -p rubc-ng (the full core gate)
just sm83                        # SM83 JSON opcode vectors
just blargg cpu_instrs           # run a Blargg ROM headlessly by name
just mooneye 'acceptance/timer'  # run a mooneye glob (needs `brew install wla-dx`)
just acid2                       # acid2 + mealybug pixel diffs
just check                       # fmt-check + clippy + build + test (pre-commit gate)
```

Reference docs and test ROMs live (git-ignored) under `reference/`; the subsystem → doc map
is in the root [`AGENTS.md`](AGENTS.md).

## Project layout

```
rubc/
├── rubc-ng/              # the emulator library — timing core, bus, PPU, APU, MBCs
├── rubc/                 # the native binary — winit/egui window, cpal audio, input
├── rubc-wasm/            # WebAssembly bindings + the original vanilla web demo
├── web/                  # the Next.js mobile PWA front-end
├── deploy/               # nginx config for the Docker demo
├── docs/                 # ACCURACY.md, USAGE.md, media, the PPU/CPU doc map
└── justfile              # task runner (run `just` to list recipes)
```

The ng core owns CPU, bus, PPU, APU, cartridge banking, savestates, and test-ROM
harnesses. Each major directory has its own `AGENTS.md` with conventions and quirks —
start with the root one before contributing.

## References

- [Pan Docs](https://gbdev.io/pandocs/About.html) — canonical Game Boy hardware reference
- [GBEDG](https://hacktix.github.io/GBEDG/) — Game Boy emulator development guide
- [Gekkio's *Complete Technical Reference*](https://gekkio.fi/files/gb-docs/gbctr.pdf) — cycle-by-cycle timing
- [SameBoy](https://github.com/LIJI32/SameBoy) — cycle-accurate reference emulator (consulted heavily for the CGB PPU timing)
- [mealybug-tearoom-tests](https://github.com/mattcurrie/mealybug-tearoom-tests) — Matt Currie's PPU torture suite + the *Comprehensive Game Boy PPU Documentation*
- [mooneye-test-suite](https://github.com/Gekkio/mooneye-test-suite) · [gb-test-roms](https://github.com/retrio/gb-test-roms) — the acceptance + Blargg ROMs

## License

Released under the MIT License. See [LICENSE](LICENSE).

Game Boy is a trademark of Nintendo. rubc is an independent project and is not affiliated
with or endorsed by Nintendo.
