# Rendering lifecycle and memory

## Ownership

`PlatformShell.WindowHandle` reports native visibility, minimization and occlusion.
`SDL3PlatformHost` resolves these into each session's `isPresentationActive` and
notifies presentation owners outside GPU frame recording. Programmatic hide is
synchronous; native changes are reconciled after polling events, before ticking
clients. Focus alone does not suspend a window.

`NativeAppWindowRenderer` owns a window's swapchain and derived targets. Its states
are unconfigured, active, suspended and closed. Suspension waits for pending work
through `SwapchainResource.close()` and drops target ownership. Pending display
requests cannot recreate these resources. The native window and latest desired
size/VSync remain; activation requests a frame and lazily recreates the swapchain.
Each window has its own presentation renderer, including reveal history and blur
scratch. The device, shader/pipeline resources, font/image residency and application
state outlive suspension; per-window transient resources do not.

The host does not initialize/draw suspended windows or spin retries for them.
An inactive window does not force display-refresh fallback on active siblings.

## Update boundaries

Body evaluation, layout invalidation and paint invalidation are separate.
Recomposition metrics must not dirty rendered content. Nodes invalidate their
paint when visual identity changes; structure and layout retain their own paths.

Guava's keyed registrar and Swift Observation both use the nearest view scope.
Swift observation wraps body/primitive evaluation, not recursive reconciliation:
wrapping the whole tree would accidentally subscribe parents to child reads.
Subscriptions are replaced on reevaluation and canceled on teardown. Mutation
callbacks schedule a later recomposition; they do not evaluate a body in `willSet`.

Plain value-type `@State` remains scope-granular. The framework does not infer
field reads from reflection or skip arbitrary closures/reference objects by a
guessed fingerprint. Existing state and binding semantics remain supported.

## Budgets and diagnostics

`ApplicationConfiguration.memory.text` limits shaping and layout caches separately
(default 512 entries and 8 MiB each). Cache hits refresh LRU order; oversized
results are usable without being retained. Trimming also prevents an older
in-progress build from repopulating the cache.

`memory.images` configures the existing image cache. Its byte limit is a ceiling,
not an eager allocation. Cache eviction does not release assets still owned by
live nodes or in-flight snapshots.

`AppDisplayHandle.memorySnapshot()` reports text-cache payload estimates, CPU
image-cache bytes and each initialized window's state, sample count, swapchain
ownership and estimated MSAA/backdrop/effect-scratch bytes. These are logical ownership metrics,
not `phys_footprint`: allocator overhead, driver allocation, drawable pools, font
atlases and externally owned scene textures are excluded. Do not derive CPU usage
by subtracting these estimates from an OS process measurement.

`trimMemory()` drops text/image cache ownership and derived layer/frame geometry.
macOS warning/critical pressure and iOS memory warnings enqueue the same operation
on the UI loop. Other hosts can invoke the explicit hook. Live view/state and
in-flight resources remain valid. Offscreen hosts expose their own `trimMemory()`.

## Quality and integration

Application rendering remains the default. `config.window.rendering` and
`display.openWindow(rendering:)` optionally override samples/clear color per window.
The default remains 4x MSAA. No new geometry-AA mode, shader ABI or texture-mipmap
contract is introduced by this repair. Those require separate image/clip/path
coverage and cross-backend pixel validation, especially for cropped engine textures
and glyph atlases; a global sampler change cannot supply missing mip contents.

Studio consumes visibility before ticking, suppresses hidden viewport/HUD work,
and restarts temporal convergence on exposure. Scene targets grow immediately and
shrink when their quantized requested area stays at or below one quarter of the
allocation for one second. Old surface leases remain readable after replacement.

## Regression coverage

- Root updates with unchanged paint reuse geometry with metrics enabled; changed
  output and theme updates still repaint.
- Swift-observed sibling properties only invalidate their readers; repeated writes,
  conditional dependencies, reader closures and scope teardown are covered.
- Dynamic text/width streams stay bounded, LRU hits survive, oversize results bypass
  retention and trimming wins over an older build.
- Hidden multi-window sessions cannot render pending frames; siblings continue.
  A Metal test exercises 100 suspend/resize/resume cycles with owned-target snapshots.
- Offscreen snapshots preserve geometry and atlas update delivery across reuse/trim.
- Studio's viewport gate suppresses forced hidden frames and restores TAA convergence;
  target sizing tests cover settled shrink, resize changes and immediate growth.

Run package checks through `guava test`, selecting this checkout with `--sdk-root`.
Changes to Swift sources do not by themselves validate Windows/Linux/Android/iOS
runtime behavior; record host-specific results separately.
