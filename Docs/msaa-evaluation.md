# UI MSAA evaluation — 2026-10-27

Keep the desktop UI default at **4x MSAA**. Analytic **1x** is available per window
for applications that control all their geometry. Raw custom triangles still
need multisampling at their boundaries. This setting does not change Diffuse's
3D scene renderer or its temporal antialiasing.

## Measurement

Apple M1 (MacBookAir10,1), macOS 27.0.1, Metal; debug host build, precompiled UI
shaders, GPU validation disabled. The logical scene is 512 × 320, with 1x and 2x
physical scale. Each configuration warms up four frames, then records thirty
frames with a GPU idle between submissions. GPU time is the completed Metal
command buffer's `gpuEndTime - gpuStartTime`, excluding CPU encoding and queue
wait. CPU numbers include encoding/submission, without the subsequent idle wait.
The GPU figure covers this small UI scene and is not a whole-editor FPS estimate.

| Scale | UI samples | GPU median (ms) | CPU median (ms) | Extra MSAA target (MiB) | Triangle RGB MAE |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1x | analytic 1 | 0.0869 | 0.0874 | 0 | 0.5344 |
| 1x | MSAA 4 | 0.1451 | 0.0826 | 2.5 | 0.1374 |
| 2x | analytic 1 | 0.2006 | 0.0811 | 0 | 0.2640 |
| 2x | MSAA 4 | 0.4406 | 0.0882 | 10 | 0.0700 |

The reference renders at four times each output dimension and downsamples 4 × 4
samples per output pixel. MAE is the mean absolute 8-bit RGB channel difference
(0–255) in the triangle region. Its unsnapped geometry is identical in both paths.
The full-scene/edge errors in the raw report also include differences from
coordinate snapping and curve tessellation at the reference resolution; they
are not an isolated AA accuracy score.

Analytic 1x reduced GPU duration by about **40% / 54%** in these two configurations.
MSAA 4x reduced triangle-region error by about **74%** at both scales. Rounded
fills, strokes, arcs and rounded clipping already carry coverage geometry, but
`DrawList.addTriangle` remains a hard boundary at 1x. Dropping global MSAA would
therefore trade away visible custom-geometry edge quality.

The extra MSAA color target consumes `physicalWidth × physicalHeight × 4 × 4`
bytes and disappears entirely on the direct-to-drawable 1x path. At 3840 × 2160
that is about **126.6 MiB** of logical attachment storage. Actual tile/driver
allocation can differ. Drawable pools, retained backdrop targets, image residency
and 3D scene targets are excluded.

## Captures

Analytic 1x:

![Analytic 1x](evaluations/msaa-metal-m1-2026-10-27/1sample-1x.png)

MSAA 4x:

![MSAA 4x](evaluations/msaa-metal-m1-2026-10-27/4sample-1x.png)

[Supersampled reference](evaluations/msaa-metal-m1-2026-10-27/reference-1x.png),
[raw measurements](evaluations/msaa-metal-m1-2026-10-27/report.json).

## Reproduce

```sh
COVE_MSAA_EVALUATION=1 ./cove test . --filter MSAAEvaluationTests --no-parallel --testing-library swift-testing
```

`COVE_MSAA_OUTPUT` overrides the output directory; the default is
`/tmp/cove-msaa-evaluation`. This measurement is opt-in and is skipped by ordinary
CI runs. Next steps before lowering the global default are analytic boundaries
for custom meshes, text/image/backdrop coverage, and comparable Vulkan/DX12
captures and timings. Those backends have not been assessed by this Metal run.
