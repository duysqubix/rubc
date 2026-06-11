# ADR 0002 — rubc-ng: ground-up timing-core rebuild

Status: accepted (2026-06-10). Scope-locks epic rubc-ijzu on branch
`rearch/rebuild`. Supersedes nothing; ADR 0001's terminal verdict is the
founding constraint.

## Decision

Build a new crate **`rubc-ng`** alongside `rubc-core` (old core stays as a
differential oracle and keeps master shippable until W9 switchover).

**Timing spine: hybrid per-T lockstep core with explicit hardware-truth
phase tables.** Not a heap event scheduler; not a SameBoy transliteration.
Every observable — fetch sample, palette latch, STAT edge, IRQ assert,
memory lock, DMA beat — is a declarative `(anchor, offset, phase)` entry in
a `TimingTable`, validated by a golden trace. Three independent profiles by
construction:

- `PpuPublicTiming` — LY/STAT/mode/IRQ/memory-lock observables
- `PpuInternalTiming` — fetcher/FIFO/sprite/window stages
- `OutputLatchTiming` — LCD column latch + palette sampling

None may derive from "last emitted pixel" except through a named, golden-
validated table entry. This makes the ADR-0001 coupling impossible to
recreate by accident.

**Model parameterization is day-one.** `GbModel` enum: DMG0/A/B, MGB,
SGB, SGB2, CGB0/A/B/C/D/E, AGB — threaded through boot profiles, timing
tables, conflict maps, and PPU quirks before W1 begins. Priority models:
DMG-B, MGB, CGB-E. **"100%" is defined as: every applicable test ROM
passes on the hardware model it targets** (mooneye's model-exclusive
variants are separate expectations under separate models, as SameBoy does).

**CPU contract inversion:** the SM83 core keeps its proven instruction
semantics (436/436) but no longer drives time — it emits bus intents
(read sample, write drive, idle, interrupt poll); the spine applies them
at table-defined phases. Double-speed: CPU phases every CPU-T; PPU/APU dot
events every second CPU-T.

## Reuse rulings

| Asset | Ruling |
|---|---|
| SM83 semantics/opcodes/ALU | keep; replace bus coupling with intents |
| APU | port intact; clock ownership changes (DIV-APU edges) |
| MBC/cartridge/battery | keep |
| CGB compat mode + palettes | keep; re-hang off GbModel boot profiles |
| Goldens + S2/W2 sidecars | promote to PRIMARY acceptance tests |
| Co-scheduler | reuse concepts only; future-drain is banned armor |
| Savestate | new v3 (GbModel + table version + phase state) |
| Frontends | keep behind an `EmulatorCore` trait; switch at W9 |

## Waves (each gated by golden/ROM sets; details in epic rubc-ijzu beads)

W0 spine+harness → W0.5 golden expansion (schema, loader, ROM/model
manifest, assertion macros, CI) → W1 public PPU schedule (incl. the four
ROMs S3 regressed) → W2 BG fetch geometry (heterogeneous delta profile;
future-drain = hard fail) → W3 output column latch → W4 window/sprites →
W5 CPU integration/interrupts → W6 DMA/timer/double-speed → W7 APU/MBC →
W8 multi-model completion → W9 frontends/savestate switchover, then old
core removal in a final cleanup.

## Kill / checkpoint criteria

No ROM-name special cases. No future-draining writes. No constants without
golden provenance. No deriving public edges from internal pixels without a
model-specific golden proving the relation. A wave that cannot match its
golden without regressing green waves revises the timing table, not local
code. If rubc-ng does not beat the old core on mealybug by W4, pause before
the CPU/APU port. Highest-risk seam: W1/W5 (public STAT under independent
internal geometry — exactly where ADR 0001's S3 was killed; here it is
designed for rather than retrofitted).

## Effort

Honest estimate ~110–184 focused agent-sessions. "Better than SameBoy"
means measurably: equal intended-model ROM conformance, golden agreement on
captured observables, safe-Rust/wasm product integration, auditable timing
tables, performance no worse than current release.
