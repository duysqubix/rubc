# rubc-core Implementation Map

## OVERVIEW
Core library for GameBoy DMG/CGB emulator. CPU/Bus/Timer/Interrupts complete; PPU complete (FIFO renderer, acid2 pixel-exact); APU complete (4 channels + sample output); CGB complete (color, double-speed, banking, HDMA); MBC0/1/2/3+RTC/5.

## MODULE MAP
| File | Responsibility | Status |
|------|----------------|--------|
| `lib.rs` | Exports, Error types | Complete |
| `globals.rs` | Memory map, IO regs, OpCode types | Complete |
| `mbc.rs` | `IntoMBC` trait, MBC0/1 impls | Partial |
| `gameboy.rs` | `Gameboy`/`Cpu` structs, `tick()`, Bus | Complete |
| `cartridge.rs` | `Cartridge` enum, ROM loading | Complete |
| `opcodes.rs` | Main opcode map | Complete |
| `opcodes_cb.rs` | CB opcode map | Complete |
| `bits.rs` | ALU macros, bit manipulation | Complete |
| `utils.rs` | Disassembly, address helpers | Complete |
| `logger.rs` | Logging setup | Complete |

## KEY TYPES
- `OpCodeFunc`: `fn(&mut Gameboy, u16) -> OpCycles`
- `OpCodeMap`: `phf::Map<u8, OpCodeFunc>`
- `OpCycles`: `u64`
- `Gameboy`: Main emulator state
- `Cpu`: CPU registers/state
- `Cartridge`: ROM/MBC container
- `IntoMBC`: Trait for MBC implementations

## DISPATCH PATTERN
`Gameboy::tick()` fetches opcode, looks up length in `OPCODE_LENGTHS`, dispatches via `phf::Map`. Handlers return `OpCycles`.

## WHERE TO ADD
- PPU/APU: New modules, wire into `Gameboy::tick()` cycle budget + `memory_read`/`write` for IO regs.
- MBC: New variants in `mbc.rs` (impl `IntoMBC`) + `cartridge.rs` enum + detection.

## CONVENTIONS
- Use `bits.rs` macros for ALU ops (not inline).
- Reference IO register addresses from `globals.rs` (no magic numbers).
- Opcode handlers must return `OpCycles`.

## ANTI-PATTERNS
- Do NOT hardcode IO values (e.g., LY=0x90 stub).
- Do NOT bypass `IntoMBC` trait for cartridge banking.
- Do NOT add opcodes outside the `phf` init maps.
