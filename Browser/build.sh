#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

# The compiler and SDK versions must match. Override for another installed pair.
sdk="${COVE_WASM_SDK:-swift-6.4.1-RELEASE_wasm}"
configuration="${COVE_WASM_CONFIGURATION:-debug}"
swift run --package-path ../../Tools/Developer cove-dev browser build-fonts
export COVE_TEXT_ARTIFACT_ROOT="Browser/.build/font-vendor"
exports=(cove_render cove_set_scale cove_font_alloc cove_font_load cove_atlas cove_atlas_size
         cove_pointer cove_key cove_text cove_vertices cove_vertex_bytes
         cove_indices cove_index_count cove_snapshot cove_snapshot_size
         cove_alloc cove_free cove_dispatch cove_response cove_response_size
         cove_events cove_hello cove_shader cove_shader_size)
flags=()
for symbol in "${exports[@]}"; do flags+=(-Xlinker "--export=$symbol"); done
if [[ "$configuration" == "release" ]]; then flags+=(-Xlinker --strip-debug); fi

swift build --build-system native --disable-build-manifest-caching --swift-sdk "$sdk" -c "$configuration" \
  --product CoveUIBrowserPrototype -Xcxx -fno-exceptions -Xswiftc -static-stdlib \
  -Xcc -mexception-handling -Xlinker -lsetjmp \
  -Xswiftc -Xclang-linker -Xswiftc -mexec-model=reactor "${flags[@]}"
bin_path="$(swift build --build-system native --swift-sdk "$sdk" -c "$configuration" --show-bin-path)"
output="${COVE_WASM_OUTPUT:-dist}"
mkdir -p "$output/wasi" "$output/devtools"
mkdir -p "$output/fonts"
cp ../Text/Fonts/* "$output/fonts/"
cp "$bin_path/CoveUIBrowserPrototype.wasm" "$output/cove.wasm"
cp web/* "$output/"
cp node_modules/@bjorn3/browser_wasi_shim/dist/* "$output/wasi/"
cp node_modules/@bjorn3/browser_wasi_shim/LICENSE-* "$output/wasi/"
cp -R ../DevTools/Client/. "$output/devtools/"
printf 'Browser prototype built: npm run serve, then open http://127.0.0.1:8080\n'
