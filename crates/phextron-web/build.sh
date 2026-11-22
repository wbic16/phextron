#!/bin/sh
# Build the Phextron web UI (WASM) and stage the servable dist/.
#
# Pipeline: cargo (rustc) with the wasm32-unknown-unknown target — nothing
# else. No wasm-bindgen, no wasm-pack, no npm. The module's ABI is three
# exported functions driven by web/glue.js.
#
# One-time setup:  rustup target add wasm32-unknown-unknown
#
# Output: dist/{index.html, glue.js, phextron_web.wasm} — point the console
# server at it with PHEXTRON_WEB_DIR=<this dir>/dist and open /web.
set -e
cd "$(dirname "$0")"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
cargo build --release --target wasm32-unknown-unknown
mkdir -p dist
cp target/wasm32-unknown-unknown/release/phextron_web.wasm dist/
cp web/index.html web/glue.js dist/
ls -la dist/
