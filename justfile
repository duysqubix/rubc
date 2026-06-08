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
acidhell  := ref / "cgb-acid-hell"
# RGBDS toolchains. Some ROMs use legacy 0.4.x macro syntax (`name: MACRO`,
# STRLWR, strings-as-numbers), others the modern 1.x (`MACRO name`). We keep
# both and pick per ROM. `rgbds_legacy` = 0.4.2 bin dir; `rgbds_modern` = PATH.
rgbds_legacy := env_var_or_default("RGBDS_LEGACY", "/opt/homebrew/opt/rgbds-0.4.2/bin")
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
    @echo "    just blargg <rom>    # headless blargg test ROM (--no-gui)"
    @echo ""
    @echo "  TEST (wave order)"
    @echo "    just sm83            # SM83 JSON opcode vectors (CPU core)"
    @echo "    just blargg [name]   # blargg gb-test-roms (default: cpu_instrs)"
    @echo "    just mooneye <glob>  # mooneye acceptance ROMs by glob"
    @echo "    just test            # unit tests (rubc-core)"
    @echo "    just check           # fmt-check + clippy + build + unit tests"
    @echo ""
    @echo "  INSPECT"
    @echo "    just cartdump <rom>  # decode the cartridge header"
    @echo "    just env-info        # toolchain + git context"
    @echo ""
    @echo "  See 'just --list' for everything."

# ---- build -----------------------------------------------------------------

# Dev build (fast compile, debug assertions on).
build:
    cargo build --workspace

# Perf build: release, NO diagnostics features (pays zero diagnostic cost).
build-release:
    cargo build --workspace --release --no-default-features

# Compile rubc-core with the full diagnostics feature set (flight recorder,
# trace, hash, metrics, snapshot). Exercised by the core's own tests.
build-diag:
    cargo build -p {{core}} --features diag-full

# ---- wasm ------------------------------------------------------------------

# Build the rubc-wasm crate to a browser-ready ES module under
# rubc-wasm/web/pkg/. Prefers `wasm-pack`; falls back to raw cargo +
# `wasm-bindgen` CLI when wasm-pack is absent. Serve the demo afterwards with
# `just wasm-serve` (or `cd rubc-wasm/web && python3 -m http.server`).
wasm-build:
    #!/usr/bin/env bash
    set -euo pipefail
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
    out="rubc-wasm/web/pkg"
    if command -v wasm-pack >/dev/null 2>&1; then
      echo "== wasm-build: wasm-pack =>$out =="
      wasm-pack build rubc-wasm --target web --out-dir web/pkg
    elif command -v wasm-bindgen >/dev/null 2>&1; then
      echo "== wasm-build: cargo + wasm-bindgen CLI =>$out =="
      cargo build -p rubc-wasm --target wasm32-unknown-unknown --release
      wasm-bindgen target/wasm32-unknown-unknown/release/rubc_wasm.wasm \
        --target web --out-dir "$out"
    else
      echo "Need wasm-pack OR wasm-bindgen-cli. Install one of:"
      echo "  cargo install wasm-pack"
      echo "  cargo install wasm-bindgen-cli --version 0.2.122"
      exit 1
    fi
    wasm="$out/rubc_wasm_bg.wasm"
    if [ -f "$wasm" ]; then
      if command -v wasm-opt >/dev/null 2>&1; then
        echo "== wasm-build: wasm-opt -O3 =>$wasm =="
        tmp="$wasm.opt"
        wasm-opt --enable-bulk-memory --enable-sign-ext --enable-mutable-globals --enable-nontrapping-float-to-int -O3 "$wasm" -o "$tmp"
        mv -f "$tmp" "$wasm"
      else
        echo "warning: wasm-opt not found; skipping post-build optimization" >&2
        echo "         install Binaryen (e.g. brew install binaryen) for smaller production wasm" >&2
      fi
    else
      echo "warning: expected wasm artifact not found: $wasm" >&2
    fi
    echo "built: $out  (serve: just wasm-serve)"

# Serve the wasm demo over HTTP (ES modules require http://, not file://).
# Usage: just wasm-serve [port]
wasm-serve port="8000":
    @echo "serving rubc-wasm/web on http://localhost:{{port}}/ (Ctrl-C to stop)"
    cd rubc-wasm/web && python3 -m http.server {{port}}

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

# Visually tour test ROMs in the GUI, auto-advancing after a timeout.
#   group   = acid2 | mealybug | acid-hell | blargg | sound | ppu | all  (default: ppu)
#             ppu = acid2 + mealybug + acid-hell (the visual PPU conformance set)
#   seconds = how long to show each ROM before auto-advancing (default: 6)
# Esc / close window = skip to next ROM early. Ctrl-C = abort the whole tour.
# Cycle test ROMs in the GUI with auto-timeout. Usage: just tour [group] [seconds]
tour group="ppu" seconds="6":
    #!/usr/bin/env bash
    set -uo pipefail
    cargo build --release -q -p rubc
    bin="./target/release/rubc"
    ref="{{ref}}"
    timeout_bin="$(command -v gtimeout || command -v timeout)"
    if [ -z "$timeout_bin" ]; then echo "need 'timeout' (brew install coreutils)"; exit 1; fi
    secs="{{seconds}}"

    # Build the ROM list for the requested group.
    roms=()
    add() { for f in "$@"; do [ -f "$f" ] && roms+=("$f"); done; }
    case "{{group}}" in
      acid2)     add "$ref"/acid2/dmg-acid2.gb "$ref"/acid2/cgb-acid2.gbc ;;
      mealybug)  add "$ref"/mealybug/*.gb ;;
      acid-hell) add "$ref"/cgb-acid-hell/cgb-acid-hell.gbc ;;
      blargg)    add "$ref"/gb-test-roms/cpu_instrs/cpu_instrs.gb \
                     "$ref"/gb-test-roms/instr_timing/instr_timing.gb \
                     "$ref"/gb-test-roms/mem_timing/mem_timing.gb \
                     "$ref"/gb-test-roms/halt_bug.gb \
                     "$ref"/gb-test-roms/oam_bug/oam_bug.gb ;;
      sound)     add "$ref"/gb-test-roms/dmg_sound/dmg_sound.gb \
                     "$ref"/gb-test-roms/cgb_sound/cgb_sound.gb ;;
      ppu)       add "$ref"/acid2/dmg-acid2.gb "$ref"/acid2/cgb-acid2.gbc \
                     "$ref"/cgb-acid-hell/cgb-acid-hell.gbc \
                     "$ref"/mealybug/*.gb ;;
      all)       add "$ref"/acid2/*.gb* "$ref"/cgb-acid-hell/*.gbc \
                     "$ref"/mealybug/*.gb \
                     "$ref"/gb-test-roms/cpu_instrs/cpu_instrs.gb \
                     "$ref"/gb-test-roms/instr_timing/instr_timing.gb \
                     "$ref"/gb-test-roms/mem_timing/mem_timing.gb \
                     "$ref"/gb-test-roms/halt_bug.gb \
                     "$ref"/gb-test-roms/oam_bug/oam_bug.gb \
                     "$ref"/gb-test-roms/dmg_sound/dmg_sound.gb \
                     "$ref"/gb-test-roms/cgb_sound/cgb_sound.gb ;;
      *)         echo "unknown group '{{group}}' (acid2|mealybug|acid-hell|blargg|sound|ppu|all)"; exit 1 ;;
    esac

    n=${#roms[@]}
    if [ "$n" -eq 0 ]; then echo "no ROMs found for group '{{group}}' (is reference/ present?)"; exit 1; fi
    echo "== tour: {{group}} -- $n ROM(s), ${secs}s each. Esc=skip, Ctrl-C=abort =="
    # Abort the whole tour on Ctrl-C instead of just skipping a ROM.
    trap 'echo; echo "tour aborted"; exit 130' INT
    i=0
    for rom in "${roms[@]}"; do
        i=$((i+1))
        echo "[$i/$n] $(basename "$rom")  (${secs}s)"
        # timeout sends SIGTERM after $secs; the window closes and we advance.
        LOG_LEVEL=warn "$timeout_bin" --foreground "${secs}" "$bin" run "$rom" || true
    done
    echo "== tour complete =="

# ---- diagnose -------------------------------------------------------------
#
# The diagnostics layer (flight recorder, trace, hash, metrics, snapshot) lives
# in rubc-core behind the `diag-full` feature and is exercised by the core's own
# tests. The binary does not yet wire a diagnostics run path, so a `just diag
# <rom>` runbook is intentionally absent until that CLI surface exists.

# Remove all diagnostic run directories and rotating logs.
diag-clean:
    rm -rf "{{diag_dir}}"/* "{{log_dir}}"/*.log 2>/dev/null || true
    @echo "cleaned {{diag_dir}} and {{log_dir}}/*.log"

# ---- test: CPU core --------------------------------------------------------

# Unit tests for the core library.
test:
    cargo test -p {{core}}

# SM83 JSON opcode vectors (the CPU-core acceptance suite, assets/sm83/v1/).
sm83:
    cargo test -p {{core}} --lib cpu::core::tests::vector_run -- --show-output

# Per-opcode M-cycle count traces (branch-timing regression; legacy name, still useful on the per-T CPU).
mcycle:
    cargo test -p {{core}} --lib cpu::mcycle -- --show-output

# ---- test: blargg gb-test-roms (prebuilt .gb) ------------------------------

# Run a blargg test ROM by name (default cpu_instrs) headlessly through the emulator core; reports PASS/FAIL via serial or the cart-RAM result protocol.
# Usage: just blargg [cpu_instrs|instr_timing|halt_bug|mem_timing|...]
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
    LOG_LEVEL=warn cargo run -q -- run --no-gui --test blargg "$rom"

# All machine integration tests (serial capture, mooneye signature, blargg
# cpu_instrs through the machine harness -- the source-of-truth CPU regression).
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
# then runs the matching ROMs HEADLESSLY through the rubc-core integration
# harness (Machine::run_mooneye), detecting pass via the Fibonacci register
# signature after `LD B,B`. A filtered run is a hard gate: every matched ROM
# must pass.
# Usage: just mooneye 'acceptance/timer'   (substring of the path under build/)
mooneye glob: mooneye-build
    MOONEYE_GLOB='{{glob}}' cargo test -p rubc-core --test mooneye_test -- --nocapture

# Report pass/fail across the WHOLE mooneye suite (reporting harness, not a
# gate -- does not fail on ROMs needing unimplemented features).
mooneye-report: mooneye-build
    cargo test -p rubc-core --test mooneye_test -- --nocapture

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

# Visual PPU tests: dmg-acid2 + cgb-acid2 (pixel-exact gates) + mealybug-tearoom
# (reporting). Compares the rendered framebuffer to reference images vendored
# under reference/test-suites/acid2 + mealybug (git-ignored; skips if absent).
acid2:
    cargo test -p rubc-core --test acid2_test -- --nocapture

# Build an RGBDS ROM, auto-selecting the toolchain version by the assembly's
# macro syntax. Legacy 0.4.x ROMs use `name: MACRO` + STRLWR; modern 1.x ROMs
# use `MACRO name`. We grep the .asm and call the matching rgbasm/rgblink/
# rgbfix (legacy from {{rgbds_legacy}}, modern from PATH). Idempotent.
# Usage: just rgbds-build <dir> <basename>   e.g. just rgbds-build {{acidhell}} cgb-acid-hell
rgbds-build dir base:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="{{dir}}"; base="{{base}}"
    asm="$dir/$base.asm"
    [ -f "$asm" ] || { echo "no $asm"; exit 1; }
    # Detect legacy syntax: a `LABEL: MACRO` line means RGBDS <= 0.4.x.
    if grep -Eq '^[A-Za-z_][A-Za-z0-9_]*:[[:space:]]+MACRO' "$asm"; then
      bin="{{rgbds_legacy}}"; ver="legacy 0.4.x"
      [ -x "$bin/rgbasm" ] || { echo "legacy RGBDS not found at $bin (set RGBDS_LEGACY)"; exit 1; }
    else
      bin="$(dirname "$(command -v rgbasm)")"; ver="modern (PATH)"
      command -v rgbasm >/dev/null || { echo "rgbasm not on PATH"; exit 1; }
    fi
    echo "rgbds-build: $base via $ver ($($bin/rgbasm --version))"
    ( cd "$dir" && "$bin/rgbasm" -o "$base.o" "$base.asm" \
      && "$bin/rgblink" -n "$base.sym" -m "$base.map" -o "$base.gbc" "$base.o" \
      && "$bin/rgbfix" -v -p 255 "$base.gbc" )
    echo "built: $dir/$base.gbc"

# Build cgb-acid-hell (legacy RGBDS 0.4.x) and copy the ROM into assets/.
acid-hell-build:
    just rgbds-build {{acidhell}} cgb-acid-hell
    cp -f "{{acidhell}}/cgb-acid-hell.gbc" assets/cgb-acid-hell.gbc
    @echo "assets/cgb-acid-hell.gbc ready"

# ---- gallery: generate README / docs visual assets -------------------------
#
# Renders the project's screenshots + GIFs headlessly into docs/media/ using the
# `screenshot` / `gif` subcommands. Deterministic (fixed frame counts, no input)
# and re-runnable: every output is a freshly-rendered frame, never a placeholder.
# Test-ROM screenshots are run to their "Passed" result screen; gameplay assets
# capture a stable/animated frame. Uses the release binary for speed.
gallery:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release -q -p rubc
    bin="./target/release/rubc"
    blargg="{{blargg}}"
    acid2="{{ref}}/acid2"
    mkdir -p docs/media/tests
    echo "== gallery: gameplay GIF + hero screenshot =="
    # Pokemon Crystal (CGB): colourful scrolling intro (skip the boot logos) +
    # the title screen as the hero still.
    "$bin" gif assets/crystal.gbc --out docs/media/crystal-intro.gif \
        --frames 36 --every 3 --scale 3 --skip 1385 --force-cgb
    "$bin" screenshot assets/crystal.gbc --out docs/media/crystal-title.png \
        --frames 3400 --scale 3 --force-cgb
    echo "== gallery: acid2 PPU conformance stills =="
    "$bin" screenshot "$acid2/dmg-acid2.gb"  --out docs/media/tests/dmg-acid2.png     --frames 120 --scale 3 --force-dmg
    "$bin" screenshot "$acid2/cgb-acid2.gbc" --out docs/media/tests/cgb-acid2.png     --frames 120 --scale 3 --force-cgb
    "$bin" screenshot assets/cgb-acid-hell.gbc --out docs/media/tests/cgb-acid-hell.png --frames 200 --scale 3 --force-cgb
    echo "== gallery: blargg test-ROM PASS screens =="
    "$bin" screenshot "$blargg/cpu_instrs/cpu_instrs.gb"   --out docs/media/tests/cpu_instrs.png   --frames 4000 --scale 3 --force-dmg
    "$bin" screenshot "$blargg/instr_timing/instr_timing.gb" --out docs/media/tests/instr_timing.png --frames 400  --scale 3 --force-dmg
    "$bin" screenshot "$blargg/mem_timing/mem_timing.gb"   --out docs/media/tests/mem_timing.png   --frames 1500 --scale 3 --force-dmg
    "$bin" screenshot "$blargg/mem_timing-2/mem_timing.gb" --out docs/media/tests/mem_timing-2.png --frames 1500 --scale 3 --force-dmg
    "$bin" screenshot "$blargg/halt_bug.gb"               --out docs/media/tests/halt_bug.png     --frames 800  --scale 3 --force-cgb
    "$bin" screenshot "$blargg/dmg_sound/dmg_sound.gb"     --out docs/media/tests/dmg_sound.png     --frames 4000 --scale 3 --force-dmg
    "$bin" screenshot "$blargg/cgb_sound/cgb_sound.gb"     --out docs/media/tests/cgb_sound.png     --frames 4000 --scale 3 --force-cgb
    echo "== gallery: done =="
    ls -l docs/media docs/media/tests

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

# Decode and print a cartridge header (title, MBC, ROM/RAM size, CGB flag).
# Usage: just cartdump <rom> [--raw]
cartdump *args:
    cargo run -q -- cartdump {{args}}

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
