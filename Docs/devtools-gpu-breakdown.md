# GPU pass and layer attribution

`TimingFrame` used to answer one question about the GPU: how long the whole frame
took. It now also answers "which passes did this frame record, how much geometry
did each carry, and which composited layer owned that geometry". This note
records what the numbers actually mean, because the three backends do not
measure the same thing and pretending they do would make the panel lie.

## What is collected

Three independent censuses are joined per frame:

| Census | Source | Cost |
| --- | --- | --- |
| Passes | `CommandBuffer::pass_reports()` in `crates/graphics/src/rhi/pass_report.rs` | Walks the recorded commands once, no allocation beyond the row vector |
| Layers | `LayerCensus` from `LayerAwareNodeRenderer::render_with_census` in `crates/core/src/scene` | One `CompositedLayer` per layer root; only collected while a DevTools session is live |
| GPU time | `GpuTimestampProfile` markers the backend already produces | Zero when no profile is attached |

The join lives in `crates/devtools/src/gpu_breakdown.rs::assemble`. It never
apportions or estimates: a pass is attributed to a layer only when their index
windows overlap, and a pass gets a `gpuMs` only when a timestamp marker names
it.

## Where GPU pass time really exists

The only backend that times individual passes is Vulkan. `encode_labels` in
`crates/graphics/src/vulkan/submission.rs` writes one timestamp pair per planned
pass, labelled with the pass's descriptor label or `renderPass#N`.

| Backend | Sampling granularity | `timingSource` |
| --- | --- | --- |
| Vulkan (Android, Linux) | One labelled pair per pass | `vulkan-timestamps` |
| Metal (macOS, iOS) | One pair per submission (`graphics submission`) | `metal-submission` |
| WebGPU (browser prototype) | One number for the whole frame | `webgpu` |
| Nothing attached | none | `cpu-only` |

On Metal a breakdown therefore has every pass row and every layer row, and
`gpuMs` is absent on all of them. That is the honest reading: the pass split is
known, the per-pass GPU cost is not measured. A panel must render the missing
value as "not measured", not as 0 ms. Adding Metal pass timing means
`MTLCounterSampleBuffer` sample points at pass boundaries; it is not implemented
and should not be faked by dividing the submission total across pass geometry.

For unnamed passes, the Vulkan labels (`renderPass#0`) cannot be matched by name.
`attribute_labels` matches them by ordinal among the unnamed passes of the same
kind. A marker that names no pass, such as a submission-level label, stays
unattached.

## How layers map to index windows

`LayerAwareNodeRenderer` appends each layer's cached `DrawList` into the frame
list, so a layer owns the contiguous index window that append created. The census
records `firstIndex`/`indexCount` around the append, plus whether the layer was
`recorded` this frame or came back from its cache (`translated` when reuse only
moved cached vertices).

Consequences a reader has to know:

* **Windows nest.** A layer root that composites its children as a cached
  container produces an outer window that contains the inner ones. A pass that
  draws the outer window therefore reports both layers. Nesting is real
  compositing, not double counting of geometry: the frame index buffer holds one
  copy.
* **A deferred container is not a layer.** On the frame a container first
  records, it has no own list and draws straight into its parent, so the census
  omits it. The same layer appears from the next frame on, when it has a cache.
* **Culled layers are reported, not dropped.** A layer root removed by culling —
  either before recording (`render`'s clip cull) or when its cached bounds sit
  outside the frame — lands in `layers` with `skipped: true` and an empty window.
  Skipped entries have no passes; they exist to explain an absent layer.
* **Every strip pass draws every visible layer.** `record_raster_pass` re-draws
  the whole batch list per `stripHeight` band, so a strip pass's `layers` list is
  the full visible set. Only backdrop and blur passes correspond to a single
  composite, and therefore to a single layer. Do not read "one pass per layer"
  into the data; the renderer never had that structure.

## Platform coverage

* Desktop (`crates/hosts/src/desktop`): census at draw time, pass reports and
  timestamp profile at submit, join at presentation. A profile is installed only
  while a DevTools session reads frames, and removed when it stops.
* Android (`crates/hosts/src/android`): same path, sharing the profile the
  `cove_android_app` timing reporter installs, so telemetry and DevTools read the
  same samples through separate drain cursors. These sources are compiled only
  for Android targets, so they are not checked by a macOS build.
* Browser (`crates/browser/src/prototype`): the prototype encodes with a flat
  node renderer, not the layer compositor, so it publishes passes and reports
  `webgpu` as its clock with an empty layer list. Layer attribution there needs a
  census in that encoder, which is not implemented.

## Reading it

`record_ui_timings` returns frames with the `gpuBreakdown` payload, so both MCP
and `cove-devtools call` see it. Each pass row carries `label`, `kind`, `draws`,
`indices`, `vertices`, `layers` (indices into `layers`) and optional `gpuMs`; each
layer row carries `nodeID`, `firstIndex`, `indexCount`, `recorded`, `translated`,
`skipped`, its bounds, and `passes` (indices into `passes`).

The Inspector performance panel renders both tables in the frame detail
(`crates/devtools/client/src/panels/performance/frames.js`), with the pass table
labelled by the same source note the payload carries.

## Tests that pin the behaviour

* `crates/graphics/tests/pass_reports.rs` — recording order, unnamed labels,
  index windows, empty buffer.
* `crates/core/src/scene/layer_aware_node_renderer.rs` tests — recorded versus
  reused versus skipped layers, and the layer window matching the frame's
  indices.
* `crates/devtools/src/gpu_breakdown.rs` tests — window overlap, skipped layers,
  Vulkan label matching including ordinals, Metal leaving pass times unmeasured.
* `crates/devtools-protocol/src/lib.rs` tests — a frame without the breakdown
  still deserializes, and a breakdown round-trips with `nodeID`/`gpuMs` naming.
