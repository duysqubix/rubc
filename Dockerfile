# syntax=docker/dockerfile:1
#
# rubc WebAssembly demo — multi-stage build.
#
# Stage 1 compiles the rubc-core emulator to WebAssembly and runs
# wasm-bindgen to generate the `pkg/` JS glue. Stage 2 is a tiny nginx
# image that serves the static demo (index.html + index.js + pkg/) with the
# correct `application/wasm` MIME type and the headers ES modules need.
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

# ---- Stage 2: serve the static demo -----------------------------------------
FROM nginx:1.27-alpine AS runtime

# Replace the default site config with one that serves .wasm correctly.
COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf

# Static demo: index.html + index.js + the generated pkg/ from stage 1.
COPY rubc-wasm/web/index.html rubc-wasm/web/index.js /usr/share/nginx/html/
COPY --from=wasm-builder /build/rubc-wasm/web/pkg /usr/share/nginx/html/pkg

EXPOSE 80

# nginx:alpine already sets a sensible CMD; keep it explicit for clarity.
CMD ["nginx", "-g", "daemon off;"]
