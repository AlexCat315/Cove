#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

configuration="${COVE_WASM_CONFIGURATION:-debug}"
target="${COVE_WASM_TARGET:-wasm32-wasip1}"
: "${COVE_WASI_SDK_PATH:?Set COVE_WASI_SDK_PATH to the wasi-sdk installation}"
export CC_wasm32_wasip1="$COVE_WASI_SDK_PATH/bin/clang"
export CXX_wasm32_wasip1="$COVE_WASI_SDK_PATH/bin/clang++"
export AR_wasm32_wasip1="$COVE_WASI_SDK_PATH/bin/llvm-ar"
export CFLAGS_wasm32_wasip1="-fwasm-exceptions -mllvm -wasm-use-legacy-eh=false -mllvm -wasm-enable-sjlj -D_WASI_EMULATED_MMAN ${CFLAGS_wasm32_wasip1:-}"
export CXXFLAGS_wasm32_wasip1="-fexceptions $CFLAGS_wasm32_wasip1 ${CXXFLAGS_wasm32_wasip1:-}"
flags=()
if [[ "$configuration" == "release" ]]; then flags+=(--release); fi
cargo build --locked --package cove-browser --target "$target" --lib "${flags[@]}"
metadata="$(cargo metadata --no-deps --format-version 1)"
workspace_root="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["workspace_root"])' <<< "$metadata")"
target_dir="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' <<< "$metadata")"
output="${COVE_WASM_OUTPUT:-dist}"
mkdir -p "$output/wasi" "$output/devtools" "$output/fonts"
cp "$workspace_root/crates/text/fonts/"* "$output/fonts/"
"$COVE_WASI_SDK_PATH/bin/llvm-objcopy" --strip-debug "$target_dir/$target/$configuration/cove_browser.wasm" "$output/cove.wasm"
cp web/* "$output/"
cp node_modules/@bjorn3/browser_wasi_shim/dist/* "$output/wasi/"
cp node_modules/@bjorn3/browser_wasi_shim/LICENSE-* "$output/wasi/"
cp -R "$workspace_root/crates/devtools/client/." "$output/devtools/"
printf 'Browser built: npm run serve, then open http://127.0.0.1:8080\n'
