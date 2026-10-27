# UI MSAA evaluation — 2026-10-27

Keep the desktop UI default at **4x MSAA**. Analytic **1x** is available per window
for applications that control all their geometry. Raw custom triangles still
need multisampling at their boundaries. Ordinary Apple-GPU window frames now
retain 4x coverage using transient tile storage. This setting does not change Diffuse's
3D scene renderer or its temporal antialiasing.

## Measurement

Apple M1 (MacBookAir10,1), macOS 27.0.1, Metal; debug host build, bundled Slang-generated MSL
shaders (compiled and warmed before sampling), GPU validation disabled. The logical scene is 512 × 320, with 1x and 2x
physical scale. Each configuration warms up four frames, then records thirty
frames with a GPU idle between submissions. GPU time is the completed Metal
command buffer's `gpuEndTime - gpuStartTime`, excluding CPU encoding and queue
wait. CPU numbers include encoding/submission, without the subsequent idle wait.
The GPU figure covers this small UI scene and is not a whole-editor FPS estimate.

The persistent baseline preserves and resolves its samples, as the previous
window path did. The transient path resolves once and discards the multisample
attachment. Both use identical 4x geometry, pipelines and physical dimensions.

| Scale | UI samples/storage | GPU median (ms) | CPU median (ms) | MSAA logical (MiB) | Metal allocated (MiB) | Triangle RGB MAE |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1x | analytic 1 | 0.0877 | 0.0773 | 0.000 | 0.000 | 0.5344 |
| 1x | persistent 4 baseline | 0.1461 | 0.0801 | 2.500 | 2.531 | 0.1374 |
| 1x | transient 4 | 0.1182 | 0.0777 | 2.500 | 0.000 | 0.1374 |
| 2x | analytic 1 | 0.1858 | 0.0887 | 0.000 | 0.000 | 0.2640 |
| 2x | persistent 4 baseline | 0.4270 | 0.0906 | 10.000 | 10.125 | 0.0700 |
| 2x | transient 4 | 0.2898 | 0.0895 | 10.000 | 0.000 | 0.0700 |

The reference renders at four times each output dimension and downsamples 4 × 4
samples per output pixel. MAE is the mean absolute 8-bit RGB channel difference
(0–255) in the triangle region. Its unsnapped geometry is identical in both paths.
The full-scene/edge errors in the raw report also include differences from
coordinate snapping and curve tessellation at the reference resolution; they
are not an isolated AA accuracy score.

The transient 4x path reduced GPU duration by about **19% / 32%** relative to
the persistent 4x baseline in this run, while its resolved pixels were **byte for
byte identical** at both scales. MSAA 4x reduced triangle-region error by about
**74%** compared with analytic 1x. Raw custom triangles still have hard boundaries
at 1x, so dropping the global sample count remains a visual tradeoff.

## Keeping 4x while reducing memory

A separate Apple M1 allocation probe used a 3840 × 2160, 4x BGRA8 sRGB attachment:

| Storage | Logical sample capacity | Metal resource allocation |
| --- | ---: | ---: |
| persistent private | 126.6 MiB | **129.5 MiB** |
| memoryless | 126.6 MiB | **0 MiB persistent backing** |

The extra allocation above the logical size comes from the backend's layout and
alignment. Memoryless samples live in GPU tile storage; the final single-sample
image and drawable pools still need memory. The probe's resolved triangle images
were identical. [Raw allocation result](evaluations/msaa-metal-m1-2026-10-27/allocation-4k.json).
Apple documents [memoryless attachment storage](https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus).

Window presentation now keeps **resolved single-sample history** for every frame.
Before each frosted panel, the renderer replays the required prefix, including
previous frosted composites, and resolves it. Only each panel's small blurred
capture survives until the final raster pass. The reveal overlay is drawn inside
that final pass. No window effect loads or preserves MSAA samples between passes.
Replay adds geometry work for each backdrop dependency; the earlier timing table
measures the simple scene and does not measure this additional work.

The reusable MSAA workspace has a **16 MiB logical backing budget** when the
backend needs device memory. Large windows render integer-aligned horizontal
strips with the original full-frame viewport and sample positions; each strip
resolves before being copied to its single-sample destination. A 4K BGRA8 4x
window uses 3840 × 273 samples (**15.996 MiB**) plus a 3.999 MiB strip resolve,
instead of a 126.6 MiB full-frame sample image. Backend alignment can add padding.
This policy is shared by Metal, Vulkan and DX12 and applies equally to ordinary
content, frost and reveals. Hardware reporting actual memoryless storage may
rasterize the whole frame without persistent sample backing.

Vulkan transient images use TRANSIENT_ATTACHMENT without transfer flags and
prefer LAZILY_ALLOCATED memory when available. Other adapters use device-local
memory within the same bounded workspace. Empty Vulkan memory blocks are freed
after their resources retire, instead of remaining resident after suspension.
DX12 uses the bounded workspace and reports resource allocation size; it has no
portable zero-backing MSAA guarantee. Unknown allocation stays unknown.

A 4K resolved BGRA8 image still costs about **31.6 MiB**. An active reveal needs
one additional retained image; exact prior-frame transitions cannot work without
retaining or reconstructing that information. Drawable pools, in-flight resources
and scene textures remain additional costs. This removes the large per-window
MSAA allocation, not all memory used by the application.

The overlapping-frost check compares complete 4x and private-storage strips at
both scales: their output is byte-identical. The old path differed only inside
the second, smaller panel because it reused oversized blur scratch with a
smaller UV footprint; capture-sized textures correct that mismatch. Initial
reveal, progress, reversal, finish and clearing also pass for single-sample,
full memoryless 4x and private strip 4x paths. The checks ran on Metal; private
strips exercise the storage policy needed by other backends, without claiming
physical Vulkan/DX12 GPU validation. [Effect results](evaluations/msaa-metal-m1-2026-10-27/effect-replay.json).

Window snapshots and DevTools distinguish logical target capacity from
`msaaAllocatedBytes` and `msaaMemoryless`. Allocation describes the current texture,
not process RSS, driver pools, 3D scene textures or replaced textures that remain
in flight until GPU completion. Hiding a window releases its targets as before.

## Captures

Analytic 1x:

![Analytic 1x](evaluations/msaa-metal-m1-2026-10-27/1sample-1x.png)

Persistent MSAA 4x baseline:

![MSAA 4x](evaluations/msaa-metal-m1-2026-10-27/4sample-1x.png)

Transient MSAA 4x (same pixels):

![Transient MSAA 4x](evaluations/msaa-metal-m1-2026-10-27/4sample-transient-1x.png)

[Supersampled reference](evaluations/msaa-metal-m1-2026-10-27/reference-1x.png),
[raw measurements](evaluations/msaa-metal-m1-2026-10-27/report.json).

## Reproduce

```sh
COVE_MSAA_EVALUATION=1 ./cove test . --filter MSAAEvaluationTests --no-parallel --testing-library swift-testing
```

`COVE_MSAA_OUTPUT` overrides the output directory; the default is
`/tmp/cove-msaa-evaluation`. This measurement is opt-in and is skipped by ordinary
CI runs. The transient path also has focused pixel-parity, invalid-use, frosted-frame
fallback, reveal and suspend/resume coverage. Next steps before lowering the
global default are analytic boundaries
for custom meshes, text/image/backdrop coverage, and comparable Vulkan/DX12
captures and timings. Those backends have not been assessed by this Metal run.
