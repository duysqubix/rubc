# rubc — GameBoy DMG/CGB emulator: operational backbone.
# `just` (no args) lists every recipe. `just help` for the guided tour.
#
# Conventions:
#   - Build variants: `dev` (fast iterate), `release` (perf, zero diag),
#     `diag` (full self-diagnosis instrumentation for AFK debugging).
#   - Test recipes mirror the ng gate plan: sm83 → blargg → mooneye → subsystem ROMs.
#   - Diagnostics artifacts land under DIAG_DIR for later inspection.

set shell := ["bash", "-uc"]
set positional-arguments

# ---- variables -------------------------------------------------------------

branch    := `git rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD`
ng        := "rubc-ng"
ref       := "reference/test-suites"
goldens   := "reference/goldens/v2"
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
    @echo "    just test            # unit + integration tests (rubc-ng)"
    @echo "    just ng-test         # unit tests (rubc-ng)"
    @echo "    just ng-goldens      # rubc-ng golden harness tests (skip-clean without goldens)"
    @echo "    just check           # fmt-check + clippy + build + unit tests"
    @echo ""
    @echo "  INSPECT"
    @echo "    just cartdump <rom>  # decode the cartridge header"
    @echo "    just env-info        # toolchain + git context"
    @echo ""
    @echo "  WEB"
    @echo "    just roms-bundle     # bundle preloaded MIT test ROMs -> web/public/roms/"
    @echo ""
    @echo "  See 'just --list' for everything."

# ---- build -----------------------------------------------------------------

# Dev build (fast compile, debug assertions on).
build:
    cargo build --workspace

# Perf build: release, NO diagnostics features (pays zero diagnostic cost).
build-release:
    cargo build --workspace --release --no-default-features

# Diagnostics feature belonged to the retired old core. Kept as a working
# compatibility recipe so old scripts fail soft instead of naming a dead crate.
build-diag:
    @echo "diag-full retired with old core; rubc-ng has no diag-full feature yet"

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
    # Keep the browser dev copy (web/src/lib/wasm/) in lockstep with the fresh
    # build. `npm run dev` does NOT recompile Rust, so this committed copy is what
    # dev serves. The rubc-ng switchover forgot this manual copy and shipped the
    # OLD core to dev for days (rubc-xltx); auto-syncing here makes that impossible.
    devwasm="web/src/lib/wasm"
    echo "== wasm-build: sync dev copy =>$devwasm =="
    for f in rubc_wasm_bg.wasm rubc_wasm.js rubc_wasm.d.ts rubc_wasm_bg.wasm.d.ts; do
      [ -f "$out/$f" ] && cp -f "$out/$f" "$devwasm/$f"
    done
    # The PWA loads the wasm at RUNTIME from /rubc_wasm_bg.wasm (emulator.ts
    # init module_or_path), which Next serves from web/public/. The Docker build
    # does NOT rebuild this served copy, so it must be committed current or
    # rubc.app serves a stale core. Keep it in lockstep with the fresh build.
    pubwasm="web/public/rubc_wasm_bg.wasm"
    echo "== wasm-build: sync served copy =>$pubwasm ="
    [ -f "$out/rubc_wasm_bg.wasm" ] && cp -f "$out/rubc_wasm_bg.wasm" "$pubwasm"
    echo "built: $out (+ synced $devwasm + $pubwasm)  (serve: just wasm-serve)"

# Rebuild the wasm and FAIL if the dev copy (web/src/lib/wasm/) drifts from a
# fresh build. Guards rubc-xltx: `npm run dev` does NOT recompile Rust, so a stale
# committed wasm silently serves an old core. Compares the dev copy before/after a
# fresh build (independent of git state). Build is deterministic per-toolchain;
# pin rust-toolchain.toml for cross-machine CI.
wasm-check:
    #!/usr/bin/env bash
    set -euo pipefail
    devwasm="web/src/lib/wasm"
    pubwasm="web/public/rubc_wasm_bg.wasm"
    snap="$(mktemp -d)"; snappub="$(mktemp)"; log="$(mktemp)"
    trap 'rm -rf "$snap" "$snappub" "$log"' EXIT
    cp -rf "$devwasm"/. "$snap"/
    cp -f "$pubwasm" "$snappub"
    if ! just wasm-build >"$log" 2>&1; then
      echo "wasm-check: wasm build FAILED:" >&2; cat "$log" >&2; exit 1
    fi
    ok=1
    diff -rq "$snap" "$devwasm" >/dev/null 2>&1 || ok=0
    cmp -s "$snappub" "$pubwasm" || ok=0
    if [ "$ok" = 1 ]; then
      echo "wasm-check: dev (web/src/lib/wasm) + served (web/public) wasm in sync with source ✓"
    else
      echo "ERROR: committed wasm was STALE vs a fresh build (rubc-xltx)." >&2
      echo "       It has now been resynced — review and commit the change:" >&2
      diff -rq "$snap" "$devwasm" >&2 || true
      cmp "$snappub" "$pubwasm" >&2 || true
      exit 1
    fi

# Serve the wasm demo over HTTP (ES modules require http://, not file://).
# Usage: just wasm-serve [port]
wasm-serve port="8000":
    @echo "serving rubc-wasm/web on http://localhost:{{port}}/ (Ctrl-C to stop)"
    cd rubc-wasm/web && python3 -m http.server {{port}}

# ---- web: preloaded test ROMs ---------------------------------------------

# Bundle the 3 MIT-licensed Matt Currie acid2 test ROMs into the web PWA so it
# can play with zero upload. Copies them under web/public/roms/, writes a
# manifest + the MIT notice (attribution is REQUIRED to redistribute), and
# pre-compresses every asset (brotli + gzip) for nginx `gzip_static` / CDN
# brotli. The Next static export copies web/public/roms/ -> web/out/roms/
# (served at /roms/). Idempotent -- safe to re-run after a ROM rebuild.
roms-bundle:
    #!/usr/bin/env bash
    set -euo pipefail
    dst="web/public/roms"
    mkdir -p "$dst"
    srcs=( "{{ref}}/acid2/dmg-acid2.gb" "{{ref}}/acid2/cgb-acid2.gbc" "{{ref}}/cgb-acid-hell/cgb-acid-hell.gbc" )
    files=( "dmg-acid2.gb" "cgb-acid2.gbc" "cgb-acid-hell.gbc" )
    for i in "${!srcs[@]}"; do
      src="${srcs[$i]}"; file="${files[$i]}"
      [ -f "$src" ] || { echo "roms-bundle: missing ROM $src" >&2; exit 1; }
      cp -f "$src" "$dst/$file"
      brotli -k -f -q 11 "$dst/$file"   # -> $file.br
      gzip   -9 -n -k -f    "$dst/$file"   # -> $file.gz  (-n: no mtime/name -> reproducible)
    done
    # Real byte sizes (the acid2 ROMs are 32768 each; computed via wc -c, not hard-coded).
    s_dmg=$(wc -c < "$dst/dmg-acid2.gb" | tr -d ' ')
    s_cgb=$(wc -c < "$dst/cgb-acid2.gbc" | tr -d ' ')
    s_hell=$(wc -c < "$dst/cgb-acid-hell.gbc" | tr -d ' ')
    # Emit valid JSON by hand (fixed shape, integer sizes, comma only on the first two rows).
    {
      printf '[\n'
      printf '  {"id":"dmg-acid2","title":"dmg-acid2","file":"dmg-acid2.gb","mode":"DMG","accuracy":"pixel-exact","license":"MIT (c) 2018 Matt Currie","sizeBytes":%s},\n' "$s_dmg"
      printf '  {"id":"cgb-acid2","title":"cgb-acid2","file":"cgb-acid2.gbc","mode":"CGB","accuracy":"pixel-exact","license":"MIT (c) 2018 Matt Currie","sizeBytes":%s},\n' "$s_cgb"
      printf '  {"id":"cgb-acid-hell","title":"cgb-acid-hell","file":"cgb-acid-hell.gbc","mode":"CGB","accuracy":"pixel-exact","license":"MIT (c) 2018 Matt Currie","sizeBytes":%s}\n' "$s_hell"
      printf ']\n'
    } > "$dst/manifest.json"
    brotli -k -f -q 11 "$dst/manifest.json"
    gzip   -9 -n -k -f    "$dst/manifest.json"   # -n: reproducible (no mtime/name header)
    # MIT requires shipping the copyright + permission notice as attribution.
    cp -f "{{ref}}/mealybug/LICENSE" "$dst/LICENSE.txt"
    printf '\nBundled test ROMs (dmg-acid2, cgb-acid2, cgb-acid-hell) (c) 2018 Matt Currie, MIT -- github.com/mattcurrie\n' >> "$dst/LICENSE.txt"
    echo "== roms-bundle: wrote $dst (3 ROMs + .br + .gz + manifest.json + LICENSE.txt) =="
    ls -la "$dst"

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
# The old diagnostics layer was retired with the old core. The binary does not
# yet wire an ng diagnostics run path, so a `just diag <rom>` runbook is
# intentionally absent until that CLI surface exists.

# Remove all diagnostic run directories and rotating logs.
diag-clean:
    rm -rf "{{diag_dir}}"/* "{{log_dir}}"/*.log 2>/dev/null || true
    @echo "cleaned {{diag_dir}} and {{log_dir}}/*.log"

# ---- test: ng core ---------------------------------------------------------

# Unit + integration tests for the ng core library.
test:
    cargo test -p {{ng}}

# SM83 JSON opcode vectors (the CPU-core acceptance suite, assets/sm83/v1/).
sm83:
    cargo test -p {{ng}} --test sm83_vectors -- --nocapture

# Per-opcode M-cycle trace recipe retired with the old core; ng vectors cover CPU instruction behavior.
mcycle:
    @echo "mcycle recipe retired with old core; use 'just sm83' or 'just ng-test'"

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

# Machine integration tests through ng's real conformance harness.
machine-test:
    cargo test -p {{ng}} --test conformance_matrix -- --nocapture

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
# then runs ng mooneye gates headlessly, detecting pass via the Fibonacci
# register signature after `LD B,B`. The glob arg is accepted for legacy CLI
# compatibility; ng owns its filtered conformance set in the test manifest.
# Usage: just mooneye 'acceptance/timer'   (substring of the path under build/)
mooneye glob: mooneye-build
    @echo "ng mooneye gate (legacy glob '{{glob}}' accepted; manifest controls scope)"
    cargo test -p {{ng}} --test mooneye_cpu_timing -- --nocapture
    cargo test -p {{ng}} --test mooneye_ppu_public_timing -- --nocapture
    cargo test -p {{ng}} --test conformance_matrix conformance_boot_register_and_hwio_profiles_pass_on_intended_models -- --nocapture

# Report pass/fail across the WHOLE mooneye suite (reporting harness, not a
# gate -- does not fail on ROMs needing unimplemented features).
mooneye-report: mooneye-build
    cargo test -p {{ng}} --test conformance_matrix -- --nocapture

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
    cargo test -p {{ng}} --test framebuffer_conformance -- --nocapture

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
    # --until-breakpoint: acid/mooneye ROMs only present their final image at the
    # LD B,B completion breakpoint, not at a fixed frame count (esp. cgb-acid-hell,
    # which mutates LCDC every scanline and never settles on a frame counter).
    "$bin" screenshot "$acid2/dmg-acid2.gb"  --out docs/media/tests/dmg-acid2.png     --until-breakpoint --scale 3 --force-dmg
    "$bin" screenshot "$acid2/cgb-acid2.gbc" --out docs/media/tests/cgb-acid2.png     --until-breakpoint --scale 3 --force-cgb
    "$bin" screenshot assets/cgb-acid-hell.gbc --out docs/media/tests/cgb-acid-hell.png --until-breakpoint --scale 3 --force-cgb
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

# rubc-ng unit tests (timing-core rebuild crate).
ng-test:
    /usr/bin/env cargo test -p {{ng}}

# Full rubc-ng 207-ROM conformance manifest report. This is honest scoring:
# real ROM pass signatures count as PASS; slice-2 oracles are explicit SKIP.
ng-conformance:
    RUBC_NG_CONFORMANCE_FULL=1 /usr/bin/env cargo test -p {{ng}} conformance_matrix_pass_count_is_gated_by_honest_floor -- --nocapture

# rubc-ng golden-gated harness subset; fresh clones skip cleanly without reference/goldens/v2.
ng-goldens:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d "{{goldens}}" ]; then
      echo "skip: {{goldens}} absent"
      exit 0
    fi
    /usr/bin/env cargo test -p {{ng}} golden_

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
