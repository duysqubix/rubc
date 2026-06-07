# Using rubc

rubc runs Game Boy (DMG) and Game Boy Color (CGB) ROMs three ways:

1. [**Native app**](#1-native-app) — a desktop window, the fastest and
   fullest-featured option.
2. [**In the browser (WebAssembly)**](#2-in-the-browser-webassembly) — no
   install; pick a ROM and play.
3. [**Docker + nginx**](#3-docker--nginx) — one command spins up the browser
   demo on a local web server.

> **ROMs are not included.** rubc plays *your* `.gb` / `.gbc` cartridge dumps.
> Nothing in this repo bundles copyrighted game ROMs.

---

## 1. Native app

### Prerequisites

A recent stable [Rust toolchain](https://rustup.rs) (the repo pins `stable`
via `rust-toolchain.toml`). [`just`](https://github.com/casey/just) is optional
but wraps the common commands.

### Build and run

```sh
git clone https://github.com/duysqubix/rubc.git
cd rubc
cargo build --release

# Play a game (a window opens):
cargo run --release -p rubc -- run path/to/game.gbc
```

With `just`:

```sh
just run  path/to/game.gb      # run quietly
just trun path/to/game.gb      # run with debug logging
```

### Controls

| Key | Game Boy button |
|-----|-----------------|
| Arrow keys | D-pad |
| <kbd>X</kbd> | A |
| <kbd>Z</kbd> | B |
| <kbd>Enter</kbd> | Start |
| <kbd>Right Shift</kbd> / <kbd>Backspace</kbd> | Select |
| <kbd>Esc</kbd> | Quit |

Print this mapping any time:

```sh
cargo run -p rubc -- controls
```

### Saves

Cartridges with battery-backed RAM persist to a `.sav` file **next to the
ROM** — `crystal.gbc` ⟶ `crystal.sav`. The file is written when you quit and
periodically while you play, so an in-game save survives closing the emulator.
Drop in an existing `.sav` from another emulator and rubc picks it up on boot.

### Other subcommands

```sh
cargo run -p rubc -- cartdump path/to/game.gbc   # print the cartridge header
cargo run -p rubc -- screenshot ROM -o shot.png  # render N frames to a PNG
cargo run -p rubc -- gif ROM -o clip.gif         # capture a GIF
```

Run `cargo run -p rubc -- --help` (or `<subcommand> --help`) for the full flag
list (frame counts, forced DMG/CGB mode, headless test modes, etc.).

---

## 2. In the browser (WebAssembly)

rubc compiles to WebAssembly and runs entirely client-side — the ROM you pick
never leaves your machine.

### Build the wasm bundle

You need the wasm target and **one** of `wasm-pack` or `wasm-bindgen-cli`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # OR: cargo install wasm-bindgen-cli --version 0.2.91
```

Then:

```sh
just wasm-build      # outputs rubc-wasm/web/pkg/ (JS glue + .wasm)
just wasm-serve      # serves rubc-wasm/web/ at http://localhost:8000/
```

Open <http://localhost:8000/>, click **Load ROM**, pick a `.gb`/`.gbc`, and
play. Because audio autoplay is blocked by browsers, sound starts muted — click
**Enable sound** once.

> ES modules require `http://` (not `file://`), which is why a tiny server is
> needed. `just wasm-serve` uses Python's `http.server`; any static server
> works, or use the [Docker setup](#3-docker--nginx) below.

### Browser controls

| Key | Game Boy button |
|-----|-----------------|
| Arrow keys | D-pad |
| <kbd>X</kbd> | A |
| <kbd>Z</kbd> | B |
| <kbd>Enter</kbd> | Start |
| <kbd>Right Shift</kbd> / <kbd>Backspace</kbd> | Select |

Saves are **not** persisted in the browser build yet (battery RAM lives only for
the session). Use the native app for persistent `.sav` files.

---

## 3. Docker + nginx

The repo ships a self-contained image that compiles the wasm bundle and serves
the browser demo through nginx — no Rust toolchain needed on your machine, just
Docker.

```sh
docker compose up --build
```

Then open <http://localhost:8080/> and load a ROM.

To stop it:

```sh
docker compose down
```

### What the image does

It is a two-stage build (see [`Dockerfile`](../Dockerfile)):

1. **Builder stage** — a Rust image adds the `wasm32-unknown-unknown` target,
   installs `wasm-bindgen-cli`, and runs the same `cargo build` +
   `wasm-bindgen` steps as `just wasm-build`, producing `rubc-wasm/web/pkg/`.
2. **Runtime stage** — a small `nginx:alpine` image serves
   `index.html` + `index.js` + the generated `pkg/`, with a config
   ([`deploy/nginx.conf`](../deploy/nginx.conf)) that sends `.wasm` as
   `application/wasm` so the browser can stream-compile it.

### Changing the port

The host port is set in [`docker-compose.yml`](../docker-compose.yml):

```yaml
ports:
  - "8080:80"   # host:container — change 8080 if it's taken
```

### Without compose

```sh
docker build -t rubc-wasm .
docker run --rm -p 8080:80 rubc-wasm
```

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Browser console: *"Failed to initialise wasm"* | You opened `index.html` over `file://`. Serve over HTTP (`just wasm-serve` or Docker). |
| `.wasm` loads as `text/plain` / won't stream-compile | Your server isn't sending `application/wasm`. The bundled nginx config handles this; for other servers, register the MIME type. |
| No sound | Click **Enable sound** (browser autoplay policy blocks audio until a user gesture). |
| `wasm-build` fails: *"Need wasm-pack OR wasm-bindgen"* | Install one: `cargo install wasm-pack` or `cargo install wasm-bindgen-cli --version 0.2.91`. |
| Game doesn't save (native) | The cartridge has no battery RAM, or the directory isn't writable. Saves land next to the ROM as `<rom>.sav`. |

For per-ROM hardware-accuracy results, see [ACCURACY.md](ACCURACY.md).
