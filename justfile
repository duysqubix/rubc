# rubc — GameBoy DMG/CGB emulator: operational backbone.
# `just` (no args) lists every recipe. `just help` for the guided tour.
#
# Conventions:
#   - Build variants: `dev` (fast iterate), `release` (perf, zero diag),
#     `diag` (full self-diagnosis instrumentation for AFK debugging).
#   - Test recipes mirror the wave plan: sm83 → blargg → mooneye → subsystem ROMs.
#   - Diagnostics artifacts land under DIAG_DIR for later inspection.

set shell := ["bash", "-uc"]
set positional-arguments

# ---- variables -------------------------------------------------------------

branch    := `git rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD`
core      := "rubc-core"
ref       := "reference/test-suites"
blargg    := ref / "gb-test-roms"
mooneye   := ref / "mooneye-test-suite"
diag_dir  := env_var_or_default("DIAG_DIR", "/tmp/logs/rubc/diag")
log_dir   := "/tmp/logs/rubc"

# Default: show the recipe list.
default:
    @just --list --unsorted

# ---- help ------------------------------------------------------------------

# Guided tour of the most useful recipes.
help:
    @echo "rubc — common workflows:"
    @echo ""
    @echo "  BUILD"
    @echo "    just build           # dev build (fast)"
    @echo "    just build-release   # perf build, NO diagnostics (zero-cost)"
    @echo "    just build-diag      # full diagnostics instrumentation"
    @echo ""
    @echo "  RUN"
    @echo "    just run <rom>       # run quietly (LOG_LEVEL=warn)"
    @echo "    just trun <rom>      # run verbose (LOG_LEVEL=debug)"
    @echo "    just diag <rom>      # AFK self-diagnosis run -> {{diag_dir}}/<run>/"
    @echo ""
    @echo "  TEST (wave order)"
    @echo "    just sm83            # SM83 JSON opcode vectors (CPU core)"
    @echo "    just blargg [name]   # blargg gb-test-roms (default: cpu_instrs)"
    @echo "    just mooneye <glob>  # mooneye acceptance ROMs by glob"
    @echo "    just test            # unit tests (rubc-core)"
    @echo "    just check           # fmt-check + clippy + build + unit tests"
    @echo ""
    @echo "  DIAGNOSE (AFK runbook)"
    @echo "    just diag <rom>      # produce full artifact bundle"
    @echo "    just last-diag       # path to the most recent diag run"
    @echo "    just diag-tail       # tail the flight recorder of last run"
    @echo ""
    @echo "  See 'just --list' for everything."

# ---- build -----------------------------------------------------------------

# Dev build (fast compile, debug assertions on).
build:
    cargo build --workspace

# Perf build: release, NO diagnostics features (pays zero diagnostic cost).
build-release:
    cargo build --workspace --release --no-default-features

# Full self-diagnosis build (flight recorder, trace, hash, metrics, snapshot).
build-diag:
    cargo build -p {{core}} --features diag-full
    cargo build -p rubc --features diag-full

# ---- run -------------------------------------------------------------------

# Run a ROM quietly. Usage: just run <rom> [extra args...]
run *args:
    LOG_LEVEL=warn cargo run -- {{args}}

# Run a ROM verbose (debug logs to stdout + {{log_dir}}/). Usage: just trun <rom>
trun *args:
    LOG_LEVEL=debug cargo run -- {{args}}

# Run a ROM with maximum tracing. Usage: just trace-run <rom>
trace-run *args:
    LOG_LEVEL=trace cargo run -- {{args}}

# ---- diagnose (AFK runbook) ------------------------------------------------

# Full self-diagnosis run. Produces a timestamped artifact bundle for offline
# debugging. Usage: just diag <rom> [extra args...]
# Artifacts: run.json rubc.log anomalies.jsonl metrics.json hash.csv
#            trace.bgb flight.bin flight.tail.txt snapshot.json serial.txt
diag rom *args:
    mkdir -p "{{diag_dir}}"
    LOG_LEVEL=debug cargo run -p rubc --features diag-full -- \
      --rom "{{rom}}" \
      --diag-dir "{{diag_dir}}" \
      --flight-recorder 1048576 \
      --trace-bgb \
      --hash frame \
      --metrics \
      --snapshot-on-panic \
      --snapshot-on-stuck \
      --panic-on-stuck \
      --max-frames 600 \
      {{args}}

# Print the path to the most recent diag run directory.
last-diag:
    @d=$(ls -dt "{{diag_dir}}"/*/ 2>/dev/null | head -1); \
      if [ -n "$d" ]; then echo "$d"; else echo "no diag runs yet under {{diag_dir}}"; fi

# Tail the decoded flight recorder of the most recent diag run.
diag-tail:
    @d=$(ls -dt "{{diag_dir}}"/*/ 2>/dev/null | head -1); \
      if [ -n "$d" ] && [ -f "$d/flight.tail.txt" ]; then \
        echo "== $d/flight.tail.txt =="; tail -50 "$d/flight.tail.txt"; \
      else echo "no flight.tail.txt in latest diag run"; fi

# Show the run.json summary of the most recent diag run.
diag-summary:
    @d=$(ls -dt "{{diag_dir}}"/*/ 2>/dev/null | head -1); \
      if [ -n "$d" ] && [ -f "$d/run.json" ]; then cat "$d/run.json"; \
      else echo "no run.json in latest diag run"; fi

# Remove all diagnostic run directories and rotating logs.
diag-clean:
    rm -rf "{{diag_dir}}"/* "{{log_dir}}"/*.log 2>/dev/null || true
    @echo "cleaned {{diag_dir}} and {{log_dir}}/*.log"

# ---- test: CPU core --------------------------------------------------------

# Unit tests for the core library.
test:
    cargo test -p {{core}}

# Alias kept for muscle memory.
unit-test: test

# SM83 JSON opcode vectors (the CPU-core acceptance suite).
sm83:
    cargo test -p {{core}} --test opcode_test -- --show-output

# Alias kept for muscle memory.
test-opcodes: sm83

# Per-opcode M-cycle count traces (taken/not-taken branch timing).
mcycle:
    cargo test -p {{core}} mcycle_count -- --show-output

# ---- test: blargg gb-test-roms (prebuilt .gb) ------------------------------

# Run a blargg test ROM by name (default cpu_instrs). The ROM boots and reports
# pass/fail via the serial channel. Usage: just blargg [cpu_instrs|instr_timing|halt_bug|...]
blargg name="cpu_instrs":
    #!/usr/bin/env bash
    set -euo pipefail
    rom=""
    for cand in \
      "{{blargg}}/{{name}}/{{name}}.gb" \
      "{{blargg}}/{{name}}.gb" \
      "assets/{{name}}/{{name}}.gb"; do
      [ -f "$cand" ] && rom="$cand" && break
    done
    if [ -z "$rom" ]; then echo "blargg ROM not found for '{{name}}'"; exit 1; fi
    echo "== blargg: $rom =="
    LOG_LEVEL=warn cargo run -- "$rom" --panic-on-stuck

# Run the blargg cpu_instrs sub-tests through the NEW M-cycle machine harness
# (rubc-core::machine integration tests). This is the source of truth now;
# the legacy `blargg`/`regression-test` recipes drive the old binary.
cpu-roms:
    cargo test -p rubc-core machine::tests::blargg -- --show-output

# All machine integration tests (serial capture, mooneye signature, blargg).
machine-test:
    cargo test -p rubc-core machine::

# The canonical CPU regression: blargg cpu_instrs via serial.
regression-test:
    @just blargg cpu_instrs

# blargg memory-timing suites.
mem-timing:
    @just blargg mem_timing
    @just blargg mem_timing-2

# ---- test: mooneye acceptance (WLA-DX .s source) ---------------------------

# Mooneye tests ship as WLA-DX assembly (NOT RGBDS), built via the suite's own
# Makefile (`wla-gb`/`wlalink`). Install once: `brew install wla-dx`. The build
# lands in `<suite>/build/**/*.gb` (git-ignored). This recipe builds on demand,
# then runs each matching ROM, detecting pass via the Fibonacci register
# signature after `LD B,B`.
# Usage: just mooneye 'acceptance/timer/*'   (glob is relative to build/)
mooneye glob: mooneye-build
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s globstar nullglob
    found=0; pass=0; fail=0
    for rom in "{{mooneye}}"/build/{{glob}}.gb; do
      [ -f "$rom" ] || continue
      found=$((found+1))
      name="${rom#*/build/}"; name="${name%.gb}"
      if LOG_LEVEL=error cargo run -q -- "$rom" --mooneye --panic-on-stuck; then
        echo "PASS  $name"; pass=$((pass+1))
      else
        echo "FAIL  $name"; fail=$((fail+1))
      fi
    done
    echo "----"
    echo "mooneye '{{glob}}': $found found, $pass pass, $fail fail"
    [ "$found" -gt 0 ] && [ "$fail" -eq 0 ]

# Build the entire mooneye suite to <suite>/build/ via WLA-DX (idempotent).
mooneye-build:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v wla-gb >/dev/null 2>&1; then
      echo "WLA-DX not installed (wla-gb/wlalink). Install: brew install wla-dx"
      echo "Mooneye here uses WLA-DX, NOT RGBDS (see Makefile: WLA=wla-gb)."
      exit 1
    fi
    make -C "{{mooneye}}" >/dev/null
    n=$(find "{{mooneye}}/build" -name '*.gb' | wc -l | tr -d ' ')
    echo "mooneye: $n ROMs built in {{mooneye}}/build/"

# ---- quality gates ---------------------------------------------------------

# Format the whole workspace.
fmt:
    cargo fmt --all

# Verify formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Clippy with warnings denied.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Full pre-commit gate: fmt-check + clippy + build + unit tests.
check: fmt-check clippy build test

# ---- inspection ------------------------------------------------------------

# Dump a ROM's disassembly to <rom>.txt and exit. Usage: just disasm <rom>
disasm rom:
    cargo run -- "{{rom}}" --disassemble
    @echo "wrote {{rom}}.txt"

# Print cargo + toolchain + git context (useful in bug reports).
env-info:
    @echo "git:    $(git rev-parse --short HEAD 2>/dev/null) ({{branch}})"
    @rustc --version
    @cargo --version
    @echo "wla-dx: $(wla-gb -v 2>/dev/null | head -1 || echo 'not installed')"
    @echo "diagdir: {{diag_dir}}"

# ---- housekeeping ----------------------------------------------------------

# cargo clean + remove generated mooneye ROMs.
clean:
    cargo clean
    rm -rf target/mooneye 2>/dev/null || true
