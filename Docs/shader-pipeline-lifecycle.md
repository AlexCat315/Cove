# Shader compilation and pipeline lifetime

Cove's RHI requires compiled shader programs and GPU pipeline creation. The
application currently loads them automatically; the absence of a compilation
screen does not mean there is no compilation.

| Backend | Bundled UI artifact | Work on first use |
| --- | --- | --- |
| Metal | Slang-generated MSL source (`mslSource`) | Compile MSL into an `MTLLibrary`, then create device pipeline states |
| Vulkan | SPIR-V | Load shader modules and create driver pipelines |
| DX12 | DXIL | Load bytecode and create driver PSOs |

The Metal backend also accepts `metallib`, but the current bundled UI uses MSL.
`MetalLibraryCache` compiles each distinct source once per device. RHI descriptor
caches and UI renderer pipeline maps reuse warmed objects within a process.
Vulkan currently passes a null pipeline cache and DX12 does not provide a cached
PSO or pipeline library. Cove has not implemented persistent pipeline caches or
a game-wide warm-up/loading phase. Driver-managed caches may still help, but are
not a guarantee made by this SDK.

The built-in UI has few variants, so automatic first-use creation is practical.
A game with many materials, formats, sample counts and specializations should
enumerate and warm its required pipelines before interactive play. To make this
reliable across launches, the RHI still needs persistent caches with robust keys,
invalidation after shader/driver/device changes and a safe recompile fallback.
Large games may display progress for that work; a small editor need not always
show a dedicated compilation screen.

The cross-platform implementations should use Metal binary archives, Vulkan
pipeline caches and DX12 cached PSOs/pipeline libraries. These are future work,
not functionality supplied by the current process caches. See
[Apple's archive lifecycle](https://developer.apple.com/documentation/metal/creating-binary-archives-from-device-built-pipeline-state-objects),
[Vulkan pipeline caches](https://docs.vulkan.org/guide/latest/pipeline_cache.html)
and [DX12 cached PSO compatibility](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ns-d3d12-d3d12_cached_pipeline_state).

`Tools/Shaders/build-native-ui.mjs` builds the UI DXIL artifacts with pinned
Slang 2026.19 and validates their register/constant ABI against the shared shader
interface. The Windows shader-artifact workflow provides DXC, verifies the Slang
archive checksum and publishes generated files for source/release packaging.
Application startup does not download or invoke Slang/DXC.
