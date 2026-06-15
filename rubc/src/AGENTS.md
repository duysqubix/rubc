# AGENTS.md - rubc Frontend

## OVERVIEW
Frontend crate for the GameBoy emulator. Handles I/O, rendering, and input.

## FILES
| File | Purpose |
| :--- | :--- |
| `main.rs` | CLI, winit event loop, emulator orchestration |
| `gui.rs` | egui integration for overlays |

## RUNTIME LOOP
1. `input.update()`: Handle winit events.
2. `emulator.update()`: Tick CPU cycles (target ~59.7 Hz).
3. `window.request_redraw()`: Trigger render.
4. `emulator.draw()`: Fill pixels buffer.
5. `pixels.render_with()`: Render world + egui.

## WHERE TO WIRE
- PPU: Replace dummy `draw()` with PPU framebuffer (160x144).
- Audio: Implement backend (cpal/rodio) fed by APU samples.
- Joypad: Map winit keyboard events to JOYP (0xFF00) in `rubc-ng`.

## CLI FLAGS
- `rom_file`: Positional argument.
- `--disassemble`: Dump disasm to <ROM>.txt and exit.
- `--breakpoints=<addrs>`: Log CPU state at PC addresses.
- `--panic-on-stuck`: Halt on illegal opcode.
- `--test-mode`: Enable ROM/RAM writes.

## CONVENTIONS
- Emulation logic belongs in `rubc-ng`.
- `rubc` is strictly for I/O and presentation.
- Use `pixels` for rendering, `egui` for UI.

## ANTI-PATTERNS
- Do not put emulation logic in `rubc`.
- Do not keep dummy `draw()` pattern.
- Do not block winit loop; respect per-frame cycle budget.
