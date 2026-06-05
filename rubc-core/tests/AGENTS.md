# rubc-core/tests/ AGENTS.md

## OVERVIEW
Integration and unit test harness for rubc-core.

## EXISTING TESTS
| File | Purpose |
|------|---------|
| `opcode_test.rs` | SM83 JSON opcode vectors |
| `utils_test.rs` | Checksum/addressing helpers |

## SM83 VECTOR FORMAT
JSON files in `assets/sm83/v1/`.
Structure: `{initial: {pc,sp,a,b,c,d,e,f,h,l,ram:[[addr,val]...]}, final: {...}}`.

## HOW TO RUN
- `just unit-test`: `cargo test -p rubc-core`
- `just test-opcodes`: `cargo test -p rubc-core -- --show-output`
- `just regression-test`: Runs `assets/cpu_instrs/cpu_instrs.gb` via `cargo run`.

## WHERE TO ADD NEW HARNESSES
- `mooneye_test.rs`: Acceptance tests. Requires RGBDS assembly of `.s` or prebuilt ROMs.
- `blargg_test.rs`: `gb-test-roms` integration. Uses prebuilt `.gb`.
- Reference ROMs: `reference/test-suites/` (git-ignored).

## CONVENTIONS
- Use `rubc-core` public API (`GameboyBuilder`).
- Determine pass/fail via CPU registers or serial output.
- Never hand-decode ROM internals.

## ANTI-PATTERNS
- Do NOT commit large test ROMs (use `reference/`).
- Do NOT delete/skip failing tests (fix the emulator).
- Do NOT assume mooneye `.s` files are runnable `.gb` without assembly.
