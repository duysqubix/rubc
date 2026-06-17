# rubc web

A mobile-first, installable PWA front-end for the `rubc` Game Boy (DMG/CGB)
emulator. The emulator core runs entirely in the browser as WebAssembly — ROMs
and saves never leave the device.

## Features

- **Client-side only** — load your own `.gb`/`.gbc` ROM; it stays on your device.
- **Local saves** — battery RAM persists per-ROM in IndexedDB, with manual
  `.sav` export (download) and import (upload) for backup or transfer.
- **Touch + keyboard + gamepad** — on-screen D-pad/buttons (multi-touch), desktop
  keyboard, and the Gamepad API for Bluetooth/USB controllers.
- **Installable, offline** — full PWA (manifest + service worker); after the
  first visit the app shell and wasm are cached for offline play.

## The wasm module

The app calls the `RubcWasm` class from `rubc-wasm` (built with
`wasm-bindgen --target web`). The generated glue lives in `src/lib/wasm/`:

- `rubc_wasm.js` / `rubc_wasm.d.ts` — ES-module glue + types
- `rubc_wasm_bg.wasm` — the emulator binary

To refresh it from a fresh emulator build, from the repo root:

```bash
just wasm-build   # builds rubc-wasm AND syncs the dev copy into web/src/lib/wasm/
just wasm-check   # CI/pre-push guard: fails if the committed dev copy is stale
```

`just wasm-build` now auto-syncs `web/src/lib/wasm/`, so the committed copy can't
silently drift from source (it once shipped the retired old core to dev for days —
rubc-xltx). The production Docker build overlays the wasm-opt-optimized binary at
build time, so the committed copy is only a dev convenience for `npm run dev`
(which does not recompile Rust). Run `just wasm-check` to verify they're in sync.

## Develop

```bash
npm install
npm run dev          # http://localhost:3000
```

## Static export

```bash
npm run build        # output: 'export' -> ./out
```

`out/` is a fully static site. The repo's `Dockerfile` builds it and serves it
through nginx (`deploy/nginx.conf`) with the correct `application/wasm` MIME and
PWA caching headers. From the repo root: `docker compose up --build`.
