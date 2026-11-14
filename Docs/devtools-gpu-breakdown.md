# GPU pass and layer attribution

**Performance → Frames** shows CPU phases and the GPU duration reported by the runtime. Select a frame to see the recorded passes, sampled durations, draw counts and links to the layers whose indices were used. The GPU samples belong to the submission that produced them. Frame slots publish them only after retirement, including when submissions complete out of order.

| Backend | GPU clock | `timingSource` |
| --- | --- | --- |
| Metal | Command buffer duration and pass boundary counters, where available | `metal-counters` or `metal-submission` |
| Vulkan | Per-submission query pool with pairs at pass boundaries | `vulkan-timestamps` |
| DirectX 12 | Direct queue timestamps resolved into a fence-owned readback buffer | `dx12-timestamps` |
| WebGPU | `timestamp-query` for the submission and each tracked pass | `webgpu` |
| No available GPU timestamps | Pass and layer geometry | `cpu-only` |

Coverage includes the native desktop and mobile hosts and the browser prototype. Canvas2D exposes layer geometry. Hardware and browser features determine which samples are available. A missing `gpuMs` appears as “not measured”. The frame total sums measured submissions and excludes presentation and barrier-only queue handoffs. Pass intervals can overlap on the GPU and can include time spent waiting on dependencies inside the command stream.

## How layers map to draw calls

A composited layer contributes a contiguous range of indices to the Cove draw list. A pass links to a layer when one of its indexed draws overlaps that range. Nested layer ranges can overlap. A reused container contributes its cached range, and a culled layer is marked `skipped` with no range. When applying an additional rounded mask changes indices at a cache boundary, the profiler attributes the resulting range to the containing layer. Fullscreen passes that sample intermediate textures can carry zero layer links. Pass timings are durations of GPU work and no milliseconds are allocated to layers.

## Reading recorded frames

`record_ui_timings` returns lightweight frame records. Request `analyze_ui_frame` with a retained frame number to get its `gpuBreakdown`, including when CPU trace data is unavailable. The response includes up to 64 passes and 128 layers and reports their full row counts. Selected frames with loaded details preserve them when exported into a performance snapshot.

```sh
cove-devtools call record_ui_timings '{}'
cove-devtools call analyze_ui_frame '{"frame":42}'
```
