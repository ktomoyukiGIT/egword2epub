#!/bin/bash
# 変換の本体を WebAssembly にして app/pkg/ に置く
set -euo pipefail
cd "$(dirname "$0")"
if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  cargo build -p egword_wasm --profile wasm-release --target wasm32-unknown-unknown
else
  # wasm32 の標準ライブラリがないときは、ソースから作る
  RUSTC_BOOTSTRAP=1 cargo build -p egword_wasm --profile wasm-release --target wasm32-unknown-unknown -Z build-std=std,panic_abort
fi
wasm-bindgen --target web --out-dir app/pkg target/wasm32-unknown-unknown/wasm-release/egword_wasm.wasm
ls -la app/pkg
