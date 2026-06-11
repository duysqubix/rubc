# rubc-ng

A ground-up rebuild of the rubc Game Boy timing core, targeting 100%
intended-model conformance on the test-ROM suites. See
[ADR 0002](../docs/adr/0002-rubc-ng-timing-core-rebuild.md) for the
architecture and [ADR 0001](../docs/adr/0001-sub-dot-cpu-ppu-event-scheduler.md)
for why the old core could not be incrementally fixed.

## Why this exists

The old core (`rubc-core`) welds internal fetch/output geometry to the
public STAT/interrupt timeline — proven a hard ceiling in ADR 0001. `rubc-ng`
positions internal geometry and public-observable timing **independently**,
each derived from SameBoy hardware-truth golden traces, so that coupling is
unrepresentable by construction.

## Architecture (one screen)

- **Hybrid per-T spine** with declarative hardware-truth `TimingTable` entries:
  every observable is a named `(anchor, offset, phase)` rule validated by a
  golden trace. Three independent profiles: `PpuPublicTiming` (LY/STAT/mode/IRQ),
  `PpuInternalTiming` (fetcher/FIFO/sprites/window), `OutputLatchTiming`
  (LCD column latch + palette sampling).
- **CPU emits bus intents** (`ReadSample`/`WriteDrive`/`Idle`/`IntrPoll`); the
  spine applies them at table phases. The CPU never ticks peripherals.
- **`GbModel` parameterization day-one** (DMG0..AGB): "100%" means every ROM
  passes on its intended hardware model.

## The review discipline (binding)

Every test asserts machine-**produced** values against an **independent**
oracle (golden trace, spec rule, or the blargg-proven old core via
differential). A test where `actual` is copied from the golden and compared to
itself is rejected; every gate must demonstrably fail under perturbation. This
is enforced at lead review — see the anti-self-reference section of ADR 0002.

## Status

Subsystems land as golden-gated waves. As of the W8a milestone the assembled
machine **executes ROMs end-to-end** (CPU drives a real `MachineBus`; blargg
`01-special` reaches `Passed` through emulated serial). Remaining work is
tracked in the `bd` issue tracker under epic `rubc-ijzu`: full `cpu_instrs` +
`mem_timing`, then the 207-ROM conformance matrix (the 100% gate), then the
frontend/savestate switchover.

## Testing

```
just ng-test       # rubc-ng unit + integration tests
just ng-goldens    # the golden-gated subset (skip-clean without reference/)
just check         # full workspace gate (fmt + clippy + build + test)
```

Golden traces live under the git-ignored `reference/goldens/` and are
reproducible from `reference/goldens/instrumentation.patch` against SameBoy.
