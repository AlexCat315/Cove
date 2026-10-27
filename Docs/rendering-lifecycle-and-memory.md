# Rendering lifecycle and memory

## Ownership

The platform host resolves native visibility, minimization and occlusion before
client ticks. Hidden windows stop presenting, wait for pending GPU work, and drop
swapchain, MSAA, backdrop and effect targets. Desired size and VSync survive;
activation requests a frame and recreates targets lazily. Focus alone does not
suspend a window. Each window owns its presentation history and scratch resources;
devices, shader pipelines, font/image residency and application state are shared.

Diffuse suppresses hidden viewport/HUD work and restarts temporal convergence on
exposure. Scene targets grow immediately and shrink after the requested area stays
at or below one quarter of capacity for one second. Resource leases preserve old
snapshots through replacement.

## Update boundaries

Body evaluation, layout invalidation and paint invalidation are separate.
Recomposition metrics do not dirty paint. Cove's keyed registrar and Swift
Observation track reads in the nearest view scope, around body or primitive
evaluation rather than recursive reconciliation. Subscriptions are replaced on
reevaluation and canceled on teardown. Mutation callbacks schedule later work.

Plain value-type `@State` invalidates its owning scope. On a parent update,
structurally equal child value inputs can preserve the existing scope, state,
bindings and callbacks without reevaluating the child's body. Closures, mutable
references and opaque values conservatively force recomposition. Changed
composition locals still invalidate consumers.

Add `@TrackStateReads` to a view struct to infer plain value-type state field
reads in its body. The compiler rewrites `model.name` and
`self.model.details.title` into typed key-path reads; the model can remain an
ordinary struct. Only changed, actually read Equatable value fields invalidate
that scope. Conditional reads are replaced on reevaluation, teardown removes
subscriptions, and DevTools reasons include the inferred field name. Normal
views retain their existing state-write behavior.

```swift
@TrackStateReads
struct ProfileView: View {
    @State var model = Profile()
    var body: some View { Text(model.name) }
}
```

Whole-value reads, helper methods and opaque/reference-bearing fields fall back
to conservative invalidation. A read such as `let snapshot = model` subscribes
to the whole value. Name shadowing disables unqualified rewriting for that name;
`self.model.field` remains explicit. Calls keep their method receiver semantics,
including writable field operations such as `model.name.append("x")`. For a
method reference, first bind the whole value in a local variable, then reference
its method; Swift key paths cannot represent methods. This is compile-time
instrumentation enabled by the view macro, not runtime inference for unannotated
Swift property access.

## Coverage and sampling

Path strokes, rounded borders and progress rings carry coverage fringes scaled to
physical pixels. `geometryAntialiasing = .analytic` also feathers rounded masks
with disjoint geometry cells and selects a 1x target for that window. Geometry
caches include this mode. Offscreen HUD and Android drawing select analytic masks.
Desktop defaults remain 4x MSAA: custom triangle meshes do not automatically carry
boundary coverage. The [Metal evaluation](msaa-evaluation.md) measures GPU time,
attachment storage and edge error; its custom-triangle result supports retaining
4x globally until those boundaries and other backends are covered.

Decoded images build alpha-weighted mip pyramids, including odd edge pixels.
Their full retained pixel payload counts toward the image cache budget. `Image`
chooses the closest resolution level from the displayed pixel size and visible
source fraction, with bilinear filtering within that level. `TextureImage` accepts
producer-owned independent texture levels. This is nearest-level LOD selection;
it does not implement trilinear filtering or native texture subresource chains.

Diffuse builds GPU levels for fixed-resolution game previews after the scene and
HUD pass. The first reduction clamps to the used region, excluding unused target
capacity. Subsequent levels have exact extents. Ordinary viewports do not allocate
a pyramid. Old snapshots retain the textures they reference.

## Budgets and diagnostics

`ApplicationConfiguration.memory.text` separately limits shaping and layout caches
(default 512 entries and 8 MiB each). Hits refresh LRU order; oversized results are
usable without retention. Trimming prevents older builds from repopulating caches.
Image-cache eviction drops cache ownership; live views and snapshots retain assets.

`AppDisplayHandle.memorySnapshot()` and DevTools `query_ui_memory` report cache
counts and estimated payload bytes, skipped parent updates, window activity,
sample count, swapchain ownership and derived target estimates. The Performance
panel refreshes these while open. These are logical ownership estimates rather
than process RSS: allocator/driver overhead, drawable pools, font atlases and
external scene textures are excluded.

`trimMemory()` drops caches and derived geometry while preserving live state and
in-flight resources. macOS pressure and iOS warnings invoke it. Windows polls its
low-memory notification and Linux samples host availability and cgroup v2 limits;
transitions into pressure enqueue cleanup on the UI loop. Android forwards
`onTrimMemory` and `onLowMemory`, excluding the visibility-only UI-hidden event.
Offscreen hosts expose their own trim hook.

## Migration

Cove 0.2 uses `Cove*` modules, `cove`, `cove.json`, `.cove`, and `COVE_*` environment
variables. Update these together when migrating an existing project. Explicit
`--sdk-root` or `COVE_SDK_ROOT` overrides may select a source checkout with a
version different from the project's fixed SDK; normal project selection still
enforces its pin. Source checkouts bootstrap native dependencies from source,
while installed SDKs require complete matching prebuilt libraries.
