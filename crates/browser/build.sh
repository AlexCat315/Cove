#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

configuration="${COVE_WASM_CONFIGURATION:-debug}"
target="${COVE_WASM_TARGET:-wasm32-wasip1}"

# `cargo cove setup --wasm` installs the pinned wasi-sdk and a native Ninja and
# records them in Cove's setup state. Every build script reads that state, so
# no shell exports, patched SDK or CMAKE_GENERATOR are needed on any host.
# Only llvm-objcopy has to be invoked directly from this script.
if [[ -z "${COVE_WASI_SDK_PATH:-}" ]]; then
  if ! COVE_WASI_SDK_PATH="$(cargo cove wasm-sdk-path)"; then
    printf 'No usable wasi-sdk. Run `cargo cove setup --wasm` first.\n' >&2
    exit 1
  fi
fi
export COVE_WASI_SDK_PATH
# CMake-based submodules and cove-native-build::configure_cc resolve the
# compiler themselves. Crates that use `cc::Build` directly (cove-system, ...)
# rely on Cargo's CC_<target> variables, so keep them pointed at the same SDK.
# Normalize separators: `cargo cove wasm-sdk-path` prints a native Windows path.
sdk="$(printf '%s' "$COVE_WASI_SDK_PATH" | tr '\\' '/')"
export CC_wasm32_wasip1="$sdk/bin/clang"
export CXX_wasm32_wasip1="$sdk/bin/clang++"
export AR_wasm32_wasip1="$sdk/bin/llvm-ar"
export CFLAGS_wasm32_wasip1="-fwasm-exceptions -mllvm -wasm-use-legacy-eh=false -mllvm -wasm-enable-sjlj -D_WASI_EMULATED_MMAN ${CFLAGS_wasm32_wasip1:-}"
export CXXFLAGS_wasm32_wasip1="-fexceptions $CFLAGS_wasm32_wasip1 ${CXXFLAGS_wasm32_wasip1:-}"
objcopy="$sdk/bin/llvm-objcopy"
if [[ ! -x "$objcopy" && ! -x "$objcopy.exe" ]]; then
  printf 'Missing llvm-objcopy in %s/bin\n' "$COVE_WASI_SDK_PATH" >&2
  exit 1
fi

flags=()
if [[ "$configuration" == "release" ]]; then flags+=(--release); fi
cargo build --locked --package cove-browser --target "$target" --lib "${flags[@]}"
metadata="$(cargo metadata --no-deps --format-version 1)"
workspace_root="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["workspace_root"])' <<< "$metadata")"
target_dir="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' <<< "$metadata")"
output="${COVE_WASM_OUTPUT:-dist}"
mkdir -p "$output/wasi" "$output/fonts"
cp "$workspace_root/crates/text/fonts/"* "$output/fonts/"
"$objcopy" --strip-debug "$target_dir/$target/$configuration/cove_browser.wasm" "$output/cove.wasm"
cp web/* "$output/"
cp node_modules/@bjorn3/browser_wasi_shim/dist/* "$output/wasi/"
cp node_modules/@bjorn3/browser_wasi_shim/LICENSE-* "$output/wasi/"
printf 'Browser built: npm run serve, then open http://127.0.0.1:8080\n'
