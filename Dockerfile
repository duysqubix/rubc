# syntax=docker/dockerfile:1
#
# rubc WebAssembly PWA — multi-stage build.
#
# Stage 1 compiles the rubc-core emulator to WebAssembly (wasm-bindgen +
# wasm-opt). Stage 2 builds the Next.js mobile PWA (static export) against that
# wasm. Stage 3 is a tiny nginx image serving the exported site with the correct
# `application/wasm` MIME type and the headers ES modules need.
#
# Build + run with `docker compose up` (see docker-compose.yml), or directly:
#   docker build -t rubc-wasm .
#   docker run --rm -p 8080:80 rubc-wasm
# then open http://localhost:8080/ and load a .gb/.gbc ROM.

# ---- Stage 1: build the wasm bundle -----------------------------------------
FROM rust:1.83-bookworm AS wasm-builder

# wasm-bindgen-cli version must match the wasm-bindgen crate dependency.
ARG WASM_BINDGEN_VERSION=0.2.122

WORKDIR /build

RUN apt-get update \
 && apt-get install -y --no-install-recommends binaryen \
 && rm -rf /var/lib/apt/lists/* \
 && cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_VERSION}" --locked

# Copy the workspace. .dockerignore keeps target/, reference/, and the
# generated pkg/ out of the build context.
COPY . .

# The repo ships a rust-toolchain.toml (channel = stable). rustup honours it on
# the first cargo invocation inside /build, so add the wasm target AFTER the
# copy — to whatever toolchain that file selects — or the build fails with a
# missing wasm32 `core` (E0463).
RUN rustup target add wasm32-unknown-unknown

# Compile rubc-wasm to wasm and generate web/pkg/ (the JS glue + .wasm).
RUN cargo build -p rubc-wasm --target wasm32-unknown-unknown --release \
 && wasm-bindgen target/wasm32-unknown-unknown/release/rubc_wasm.wasm \
      --target web \
      --out-dir rubc-wasm/web/pkg \
 && wasm-opt --enable-bulk-memory -O3 \
      rubc-wasm/web/pkg/rubc_wasm_bg.wasm \
      -o rubc-wasm/web/pkg/rubc_wasm_bg.wasm.opt \
 && mv -f rubc-wasm/web/pkg/rubc_wasm_bg.wasm.opt \
      rubc-wasm/web/pkg/rubc_wasm_bg.wasm

# ---- Stage 2: build the Next.js PWA (static export) -------------------------
FROM node:22-bookworm-slim AS web-builder
WORKDIR /web

# Install deps against the committed lockfile (cached unless package*.json change).
COPY web/package.json web/package-lock.json ./
RUN npm ci

# App sources, then overlay the freshly-optimized wasm from stage 1 so the
# bundle never ships a stale hand-committed binary.
COPY web/ ./
COPY --from=wasm-builder /build/rubc-wasm/web/pkg/rubc_wasm_bg.wasm ./src/lib/wasm/rubc_wasm_bg.wasm
COPY --from=wasm-builder /build/rubc-wasm/web/pkg/rubc_wasm.js       ./src/lib/wasm/rubc_wasm.js

# Produce the static export in /web/out.
RUN npm run build

# ---- Stage 3: serve the static export --------------------------------------
FROM nginx:1.27-alpine AS runtime

# Replace the default site config with one that serves .wasm correctly.
COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf

# The Next.js static export (HTML, hashed JS/wasm under _next/, manifest, sw.js).
COPY --from=web-builder /web/out /usr/share/nginx/html

EXPOSE 80

# nginx:alpine already sets a sensible CMD; keep it explicit for clarity.
CMD ["nginx", "-g", "daemon off;"]
