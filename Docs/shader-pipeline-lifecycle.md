# Shader compilation and pipeline lifetime

Cove packages offline shader artifacts. GPU-specific pipeline creation still
happens on the user's device; a precompiled shader library does not eliminate
that driver work.

| Backend | Packaged shader | Device work |
| --- | --- | --- |
| Metal | Platform `metallib` for macOS, iOS and simulator | Load library, specialize functions, create pipeline states |
| Vulkan | SPIR-V | Load modules and create driver pipelines |
| DX12 | DXIL | Load bytecode and create driver PSOs |

`ShaderArtifact.load(from:name:stage:api:)` selects the matching Metal platform
library only when its source fingerprint matches the base MSL artifact. Editing
MSL invalidates the library and falls back to compiling that source. Custom
projects can supply the same artifacts; startup never invokes Slang or downloads
a compiler. `Tools/Shaders/build-metal-libraries.mjs` builds the three platform
libraries. The shader-artifact workflow supplies Xcode's Metal compiler and a
checksum-verified Slang 2026.19 for Windows DXIL generation.

## Background preparation

Use `Device.prepareResources` before publishing a renderer or game scene. It
runs operations serially on a per-device worker queue, transfers the successful
result to the caller, and reports immutable progress snapshots:

```swift
let resources = try await device.prepareResources(
    label: "Preparing materials", totalSteps: 2,
    onProgress: { progress in
        // A worker callback. Dispatch UI updates to MainActor and use sequence
        // to reject older callbacks when tasks are delivered out of order.
        report(progress)
    }
) { preparation in
    let shader = try preparation.step("Shader") {
        try device.makeShaderModule(shaderDescriptor)
    }
    let pipeline = try preparation.step("Pipeline") {
        try device.makeGraphicsPipeline(makeDescriptor(shader))
    }
    return (shader, pipeline)
}
```

The returned value owns its resource leases; destroy handles when their owner
is finished. On failure or cancellation, the preparation releases its remaining
acquisitions, including leases on existing cached objects, without invalidating
other owners. Keep work inside the supplied operation and return the owned
result. Do not publish partially prepared resources to another thread.

`makeShaderModuleAsync`, `makeGraphicsPipelineAsync`, `makeComputePipelineAsync`
and `makeMeshPipelineAsync` provide one-step equivalents. Existing synchronous
creation methods retain their behavior and can block their calling thread.
Custom backends must implement `RHIBackgroundCompilation` before using the async
preparation APIs; all built-in backends implement it.

Metal, Vulkan and DX12 snapshot inputs under the resource lock, release the lock
for expensive compilation, then publish completed objects under the lock. Shader
inputs remain alive until active compilation completes. Existing warmed UI can
continue recording and submitting frames on the same device. The driver still
shares CPU/GPU resources with rendering, so background preparation is not a
promise of zero performance impact.

Progress counts declared steps and actual cold compilation operations. A step
can contain several shaders or pipelines; the fraction is work-item progress,
not a remaining-time estimate. Cancel the enclosing Swift `Task` to cancel
preparation. A driver call already in progress cannot be interrupted; its result
is discarded when it returns, remaining steps are skipped, and owned resources
are retired. Hosts may update cancellation UI immediately and use a generation
token to ignore late callbacks.

## Reuse and host integration

Identical shader modules share content-keyed, reference-counted handles.
Identical pipeline descriptors share reference-counted native pipeline states;
labels do not change their keys. `Device.pipelineStatistics` separates cold
creation time, warm hits, live resources and creation during a frame. These
caches live for the device/process lifetime.

Diffuse uses `NativeRenderer.prepared` before starting its render consumer. Its
editor remains interactive and displays preparation progress, cancellation,
errors and retry in the status bar. The standalone Player displays a loading
view and starts simulation after preparation succeeds. Material and post-effect
variants follow the initial render settings; particle graphics and workgroup
64 are prepared by default. Games with other particle sizes can pass
`particleWorkgroupSizes` to the factory. Additional/custom variants need an
explicit preparation operation before use; settings changed later may create
new variants.

Persistent driver pipeline caches are still future work: Metal binary archives,
Vulkan pipeline caches and DX12 cached PSOs need device/driver/shader invalidation
and safe recompilation fallback. The current release does not promise that a
second application launch skips GPU pipeline creation. See
[Metal archives](https://developer.apple.com/documentation/metal/creating-binary-archives-from-device-built-pipeline-state-objects),
[Vulkan pipeline caches](https://docs.vulkan.org/guide/latest/pipeline_cache.html)
and [DX12 cached PSOs](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ns-d3d12-d3d12_cached_pipeline_state).
