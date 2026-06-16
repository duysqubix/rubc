# AGENTS.md — rubc binary (native frontend)

## OVERVIEW
Native frontend for the Game Boy emulator. Owns ALL I/O + presentation; zero
emulation logic (that lives in `rubc-ng`). Drives `rubc_ng::MachineNg`.

## FILES
| File | Purpose |
| :--- | :--- |
| `main.rs` | clap CLI, eframe app + per-frame loop, savestate/.sav I/O, headless test-ROM harness |
| `gui.rs` | egui menu bar (File / Debug / About dialogs) |
| `audio.rs` | cpal output: pumps `machine.drain_samples()` (stereo f32) to the default device |
| `capture.rs` | headless screenshot (PNG) + GIF capture from `machine.framebuffer()` |
| `vramview.rs` | detachable egui debug window (VRAM tilesheet/tilemap, OAM, palettes) via `machine.debug_*()` |
| `logger.rs` | pure-Rust `log::Log` impl; `LOG_LEVEL` env (default Warn). Relocated here when rubc-core was retired |

## STACK
`eframe` 0.34 (wgpu) drives a real native window + re-exports `egui`; `cpal`
0.18 audio; `clap` 4.5 CLI; `chrono` (logger timestamps). NOT winit/pixels.

## CLI (clap subcommands)
- `run ROM [--no-gui] [--force-dmg|--force-cgb] [--save FILE]` — boot windowed (or headless test-ROM run)
- `screenshot ROM --out PNG [--frames N]` — headless single-frame capture
- `gif ROM --out GIF [--frames N]` — animated capture
- `cartdump ROM [--raw] [--out FILE]` — decode the cartridge header
- `controls` — print the keyboard map
- bare `ROM` — shorthand for `run ROM`

## PER-FRAME LOOP (main.rs RubcApp::logic)
input (keyboard → `machine.set_button`) → `machine.step_frame()` → periodic
`save_ram()` flush → `drain_samples()` → cpal → framebuffer → egui texture →
optional VRAM snapshot → absolute-deadline pacing (~16.74 ms / 59.7 Hz).

## CONVENTIONS
- Emulation logic NEVER here — only in `rubc-ng`. This crate is I/O + presentation.
- The machine is the single `rubc_ng::MachineNg`; `boot_dmg`/`boot_cgb` return
  `Result` (the old core returned `Self`) — `.expect(...)` at boot.
- `FramePixel` is `DmgShade(u8)` | `CgbRgb555(u16)`; capture.rs maps it to RGBA.

## ANTI-PATTERNS
- Do NOT put emulation logic in `rubc`.
- Do NOT block the eframe loop; respect the per-frame deadline.
- Savestate is wired (`machine.save_state`/`load_state`, v3); do NOT re-stub it.
