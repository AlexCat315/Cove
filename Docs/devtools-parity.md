# Flutter DevTools 3.50 parity

Audit snapshot: 2026-11-13 04:28 CST, working tree at `095b5923` plus the uncommitted M1–M3 and debugger work. Line numbers are from that snapshot; the debugger implementation landed while this audit ran, so its client line numbers were still moving (see [Concurrent work](#concurrent-work)).

This is the goal's recorded matrix. A row is `implemented + verified` only when the repository, its tests, or an isolated live controller run supports it. Nothing here treats a UI shell, a schema, or a synthetic payload as a completed feature.

Milestone mapping: **M1 Inspector** is verified through the tree/details path, with app-local capture and picking tracked in the acceptance log; **M2 Performance** is verified for frames, traces, and frame attribution; **M3 CPU profiler** is verified for real macOS sampling, ranked stacks, and hotspots. Everything else is tracked below. For `implemented + verified` rows, the controller/runtime path is what was verified; client DOM rendering is called out separately where a harness exercised it.

Status legend:

- **implemented + verified** — code paths exist and were exercised by tests and/or a live controller session.
- **implemented using the runtime framebuffer** — code exists but the macOS privacy/developer authorization needed to exercise it is not granted on this host.
- **needs runtime work** — the capability is applicable but the runtime cannot produce honest data yet.
- **proven not applicable** — the exact Flutter/Dart mechanism has no Rust analogue; any user-facing substitute is tracked separately.

## Summary matrix

| Flutter DevTools 3.50 surface | Cove DevTools equivalent | Status |
| --- | --- | --- |
| Inspector widget tree, details, semantics, source | Cove node tree, Properties/Render/Layout/Source/Semantics panels | implemented + verified |
| Inspector Select Widget mode and node screenshot | `select_ui_at`, `capture_ui_node` | implemented using the runtime framebuffer |
| Inspector debug painting: guidelines | `set_ui_debug_paint` + `SceneEditor` 1px live-node outlines | implemented + verified (toggle/DrawList; native visual acceptance pending) |
| Inspector debug painting: baselines | `set_ui_baselines` + `SceneEditor` 1px lines from each node's real `TextLayoutResult` | implemented + verified (toggle/DrawList; native visual acceptance pending) |
| Inspector debug painting: repaint rainbow | `set_ui_repaint_highlight` + per-node paint generations from real renderer entries | implemented + verified (toggle/DrawList; native visual acceptance pending) |
| Inspector debug painting: oversized images | `set_ui_oversized_images` + `SceneEditor` 2px outlines from each image's recorded source/display pixel sizes | implemented + verified (toggle/DrawList; native visual acceptance pending) |
| Inspector slow animations (Flutter `timeDilation`) | `set_ui_animation_speed` + global core animation time scale applied to each host's scheduler delta | implemented + verified (runtime/probe/controller; client control tracked separately) |
| Inspector flex/layout explorer with editable flex properties | editable grow/shrink/justify/align via `edit_ui_style`; read-only layout panel | implemented + verified (basis/fit and per-child constraint explorer still missing) |
| Performance frames chart and jank view | `record_ui_timings` + client frames chart | implemented + verified |
| Performance Frame analysis tab | `analyze_ui_frame` phase attribution from retained trace spans | implemented + verified |
| Performance timeline events + enhanced tracing | `start_ui_trace` / `stop_ui_trace` CPU span timeline and flame chart | implemented + verified (CPU spans only) |
| CPU profiler sampling, ranked stacks, hotspots, Call tree, Bottom-up, Method table, flame chart | `cove-profiling` + `start/stop/get_ui_profile` + `query_ui_hotspots` + client sub-views | implemented + verified |
| CPU profiler call tree, bottom-up, method table, flame chart, filters | Call tree / Bottom-up / Flame chart / Methods views with function filter; no library/Thread filters | implemented + verified |
| Memory cache/render counters + process RSS/virtual + allocator tracking | shaping/layout/image counters, per-window target/MSAA bytes, `processResidentBytes`/`processVirtualBytes`, `alloc*` counters and size classes | implemented + verified |
| Memory allocation-site sampling + baseline diff | opt-in `cove_alloc` sampled stacks + `cove-profiling` symbolization; start/stop/query tools; client site table, snapshot/import baseline and per-site diff | implemented + verified (cumulative sampled allocations, not live heap) |
| Memory heap object graph, heap Diff Snapshots, Trace Instances, leak tracking | none | needs runtime work; Dart GC/leak mechanisms proven not applicable |
| Network HTTP/HTTPS | in-process `cove-http` recorder with `set_ui_network_recording` pause/resume and `clear_ui_network` | implemented + verified |
| Network WebSocket, all traffic | none | needs runtime work |
| Source-level debugger (breakpoints, stepping, variables, console, exceptions) | DAP/LLDB native debugger (controller tools + client UI) | implemented using the runtime framebuffer (real attach) |
| Logging view (stdout/stderr, framework, GC, custom) | `query_ui_logs` records/filters + real `clear_ui_logs` replay-history clear | implemented + verified (host logs); GC events proven not applicable |
| App Size: native executable sections/symbols/groups + treemap/diff | `get_ui_app_size` from the runtime PID; `object` sections/symbols; demangled crate groups; client treemap and snapshot diff | implemented + verified (dominator tree/call graph still missing) |
| App Size: Dart AOT/package/class attribution | none | proven not applicable (Cove has no Dart AOT output) |
| Deep Links validator | `get_ui_deep_links` reads Android manifest/assetlinks and Apple plists/entitlements/AASA under the project root | implemented + verified (gallery has none; fixture projects cover links and failures) |
| Provider-like state inspection | opted-in state registry (`query_ui_state`) | implemented + verified |
| DevTools extension host (third-party UI from packages) | none | needs architecture work; pub/Dart extension mechanism proven not applicable |
| Hot reload / hot restart | none | needs runtime work; structural blocker |

## Inspector

Flutter DevTools 3.50 exposes a widget tree, widget/render details, a flex explorer with editable properties, select-widget mode, screenshots, and debug-paint toggles (guidelines, baselines, repaint rainbow, oversized images, slow animations) ([Inspector docs](https://docs.flutter.dev/tools/devtools/inspector)).

Cove's Inspector is a live Cove node tree, not a Flutter widget/render-element tree:

- Tree, search, role/text/source filters, properties, render inventory, layout, source, and semantics: `crates/devtools/client/index.html:135-264`, `crates/devtools/client/inspector.js:1057` (`loadTree`), `1579-1627` (properties/semantics), `1764` (layout), `1834` (source).
- Node tools and temporary overrides: `crates/devtools-cli/src/tools.rs:187-205` (`inspect_ui_tree`, `select_ui_node`, `set_ui_environment`, `edit_ui_style`). `edit_ui_style` now supports background/foreground/padding and flexGrow/flexShrink/justifyContent/alignItems, with real layout invalidation and per-field restoration on clear/undo/redo/reset; per-child basis/fit and constraint graphs are not exposed.
- Live check: the gallery returned a 1,884-node tree, 1,133,706 bytes, through the browser client's `/rpc` path. The browser transport is `crates/devtools/client/inspector.js:442-465`; the controller handles it at `crates/devtools-cli/src/http.rs:187-214`.
- MCP/CLI caveat: the full tree through the agent toolbox exceeds the 512 KiB result budget (`crates/agent/src/tool.rs:150`, `208-230`); a live `cove-devtools call inspect_ui_tree` returns the 512 KiB refusal. Use the real paged `query_ui_tree {node_id?, depth, limit}` tool instead: depth 2/limit 200 returns 2 nodes/491 bytes, depth 16/limit 10 returns 10 nodes/2917 bytes on the gallery, with `nodeCount`/`truncated` and stale/invalid-range errors. `query_ui_nodes` remains the flat-node search tool.
- Select-at-point and node capture use the Cove framebuffer. The controller maps image pixels to the runtime viewport, and the runtime performs hit testing. See the [acceptance log](devtools-acceptance.md).
- Guidelines are implemented: `set_ui_debug_paint {guidelines}` drives `SceneEditor::draw_overlay` to outline every visible live node frame with a capped 1px amber border, clipped by the root/each node's clip chain, then draws the selection border on top. Backend tests assert 4→20→4 rects on toggle, clipping, live frame movement and a 512-outline cap; the controller fake-RPC test asserts the exact probe command; the client harness toggles the control and handles capability loss/target switch. The on-screen pixels were not captured because Screen Recording is off on this host.
- Baselines are implemented from real text layout: `set_ui_baselines {enabled}` drives `SceneEditor::draw_overlay` to draw a 1px line at `frame.y + line.baseline_y` for every line in the node's cached `LayoutNode::text_measure().result.lines`, with the measured line width clamped to the frame. Only nodes with a measured `TextLayoutResult` contribute, so non-text nodes get no invented baseline. The walk is bounded (512 lines, 4096 visited nodes), skips detached/invisible/transparent/zero-area nodes, and is clipped by the root viewport and each node's clipping ancestors. Baselines are a separate layer from `debug_paint` guidelines and `animation_scale`; tests in `crates/devtools/tests/baseline_tests.rs` cover toggle, non-text/invisible/zero-area/detached skipping, live frame movement, width clamping, root/ancestor clipping, both caps, and that guidelines/selection still render. The controller fake-RPC test asserts `set_ui_baselines` sends `ProbeCommand::Baselines` and gates on the `baselines` capability; the probe advertises it, and `RuntimeProbe::disconnect` clears it.
- Oversized images are implemented from the image primitive's own paint data. On every paint, `Image` records `CoveUI.image.sourcePixels` (the decoded asset or registered texture extent, never the authored point size), `CoveUI.image.displayPixels` (the painted rect scaled by the draw list's pixel scale, so `.fit` letterboxing is excluded) and `CoveUI.image.vector`. `set_ui_oversized_images {enabled}` drives `SceneEditor::draw_overlay` to draw a 2px magenta border around each visible node whose real source area is more than 4× its display area. Nodes without recorded metrics, vector rasters chosen at the draw size, and non-finite/negative/zero display extents are skipped; the walk is bounded (512 outlines, 4096 visited nodes) and clipped by the root viewport and each node's clipping ancestors. It is a separate layer from guidelines, baselines, selection and `animation_scale`; tests in `crates/devtools/tests/oversized_image_tests.rs` cover the threshold boundary, vector/unknown/invalid/zero-display skipping, root/ancestor clipping, the cap, toggle/disconnect and overlay independence, while `crates/compose/tests/image_pixel_metrics_tests.rs` pins the recorded metadata and its clearing on an unknown source. The controller fake-RPC test asserts `set_ui_oversized_images` sends `ProbeCommand::OversizedImages`, gates on the `oversized_images` capability and returns the applied state. The on-screen pixels were not captured because Screen Recording is off on this host.
- Repaint highlighting is implemented from real paint execution: each node carries a monotonic repaint epoch bumped by `NodeRenderer` and by the layer renderer's actual `begin` paint path, while a layer-cache hit (`reuse`) does not bump. `set_ui_repaint_highlight {enabled}` draws a 2px border colored from that epoch, so a repainting node cycles color and a cached node stays put. The walk is capped/clipped and independent of guidelines/baselines/oversized images/selection; `crates/devtools/tests/repaint_highlight_tests.rs` and the renderer unit tests in `crates/core/src/scene/node_renderer.rs`/`layer_aware_node_renderer.rs` cover real renderer bumps, cache-hit stability, toggle, clipping, cap and disconnect, and the controller fake-RPC test asserts `set_ui_repaint_highlight` gates on `repaint_highlight`. On-screen pixels were not captured because Screen Recording is off.
- Slow animations are implemented independently of debug paint: `set_ui_animation_speed {scale}` stores a process-wide scale in `cove_core::scene::animation` (clamped to 0.05–10, returned as applied) and every host that ticks the animator scales only its scheduler delta, so frame pacing, input and diagnostics stay on wall-clock time. The probe advertises `animation_speed` and resets the scale to 1.0 when DevTools disconnects.
- The Flutter widget/render-element object model and its flex explorer are **proven not applicable**; the analogue is Cove's node/component tree. Editable flex grow/shrink/justify/align is implemented and verified; per-child basis/fit and a constraint graph remain missing (runtime work).
- Flutter's widget-creation instrumentation and package-directory tree filtering have no direct equivalent; Cove shows a source location only when the node record publishes one (`crates/devtools/client/inspector.js:1834`), and the tree is not filtered by project/package directory.

## Performance

Flutter DevTools 3.50 has a frames chart with UI/raster work and jank highlighting, a Frame analysis tab with debugging hints, a timeline events viewer with enhanced tracing (widget builds, layouts, paints, layer toggles), and snapshot import/export ([Performance docs](https://docs.flutter.dev/tools/devtools/performance)).

Cove implements the measurable subset:

- Frame records carry `recompositionMs`, `layoutMs`, `drawMs`, `renderThreadMs`, `submitted`, `presented`, `totalMs`, node/batch counts, and an owned CPU timeline window (`crates/devtools-protocol/src/lib.rs:138-152`). The client renders the chart at `crates/devtools/client/inspector.js:3304` and a frame table/analysis block at `3473`.
- Attribution is real: `FrameWindow` closes the sequence range a frame owns, and `FrameAnalysis::for_frame` classifies only retained spans (`crates/devtools/src/frame_analysis.rs:1-17`, `227-240`; probe wiring `crates/devtools/src/runtime_probe.rs:233-238`). Frames with no window, no attributable spans, or expired spans return `unavailable` with the reason, never a guessed phase (`crates/devtools/src/frame_analysis.rs:144-160`; tests `crates/devtools/tests/frame_analysis_tests.rs`).
- Live run: a 2.75 s trace around a real `resize_ui` produced 756 spans (731 component, 19 frame, 3 draw, 2 layout, 1 recomposition). Frame 20 owned sequences 14–755; `analyze_ui_frame` returned `dominantPhase: "recomposition"` with 741 spans, 96.5% of attributed CPU, plus a `draw` and a `layout` share. This is the end-to-end controller path, not a fixture.
- An idle trace (first live run) produced 0 spans and frames with `timelineStart == timelineEnd == 0`; the tool correctly had nothing to attribute. That is expected: Cove emits frames on demand, and Frame analysis requires an operation with retained spans.
- GPU execution and physical display refresh are not measured; the budget is a user-selected 60/90/120 Hz because the runtime does not publish its refresh rate (`crates/devtools/client/index.html:367`; `crates/devtools/README.md:52`; `Docs/devtools-redesign.md:33`). Flutter's UI/raster thread split is Flutter-specific and **proven not applicable**; Cove's phases are its own.
- Flutter's Frame analysis hints (shader compilation, expensive ops) and enhanced-tracing toggles are not implemented. Cove records component/layout/draw spans during a capture but has no "track builds/layouts/paints" controls or layer-disable experiments.
- The client-local snapshot export/import exists (`crates/devtools/client/index.html:383-390`; snapshot state in `crates/devtools/client/inspector.js:2852`, `3229`), but the audit did not exercise it in a browser.

## CPU profiler

Flutter DevTools 3.50 records VM CPU samples with low/medium/high rates (1,000/4,000/20,000 Hz) and presents Call tree, Bottom up, Method table, and Flame chart views ([CPU profiler docs](https://docs.flutter.dev/tools/devtools/cpu-profiler)).

Cove's profiler is a real in-process macOS sampler:

- `cove-profiling` follows one named thread using Mach `thread_get_state` from the current task (no `task_for_pid`), walks the frame-pointer chain, and symbolizes with `addr2line` over DWARF, split-DWARF objects, or dSYM (`crates/profiling/src/lib.rs:1-35`).
- Bounds: 1–1,000 Hz, 1–10,000 ms, 200 hotspots; the worker publishes counters every 4 ms and stops itself at the bound (`crates/devtools/src/profile/native.rs:20-36`). `start_ui_profile` / `stop_ui_profile` / `get_ui_profile` / `query_ui_hotspots` are registered at `crates/agent/src/ui_tools.rs:220-255` and dispatched at `crates/devtools-cli/src/controller.rs:692-706`.
- Live run: `cpu_profile` was advertised; `start_ui_profile {hz:500, duration_ms:800}` sampled the `cove-ui` thread and returned 282 samples in 803 ms with 0 dropped, 2 aggregated stacks, and hotspots. The second hotspot was a real symbol: `<core::cell::RefCell<cove_hosts::desktop::diagnostics::session::ApplicationDiagnostics>>::borrow_mut`, `/Users/cyke666/.rustup/toolchains/1.99.1-aarch64-apple-darwin/lib/rustlib/src/rust/library/core/src/cell.rs:1224`. The top hotspot was `__psynch_cvwait` because the UI thread was idle; that is honest sampling, not a placeholder.
- Tests pin the wire shape and a real sampler run: `crates/devtools/tests/profile_tests.rs:18-230`, including `profile::native::tests::a_real_sampler_run_resolves_the_busy_threads_function_and_line`.
- Client CPU Profile panel: `crates/devtools/client/index.html:395`; Call tree, Bottom-up, Flame chart, Methods and Stacks sub-views with a shared function filter and sortable method columns (`crates/devtools/client/inspector.js` CPU Profile section), plus rate/duration/start/stop controls and honest running/empty/unavailable/unsupported/truncated states. The fake-CDP harness is `crates/devtools/client/tests/profile-harness.mjs`; `Tools/DevTools/acceptance/profile.sh` drives the real sampler and passes.
- Verified live: `profile.sh --no-build` returned 506 samples in 1503 ms at 500 Hz, `thread=cove-ui`, 0 dropped, 1 stack, a real hotspot, early stop at 263 samples, and an explicit `no_profile` on the second stop. The client harness exercised call-tree aggregation, bottom-up, flame-chart caps, filter, selection sync, methods sorting, idle defaults, running/empty/unsupported/truncated states.
- Remaining gaps vs Flutter: no caller/callee graph beyond the call tree, library/UserTag filters, thread/isolate selection, or full method-table source drill-down. Only the UI thread is sampled.
- Platform gaps are explicit: the sampler is compiled only for macOS arm64/x86_64 (`crates/profiling/src/platform/mod.rs:1-24`). Linux/Windows return `unavailable` with a reason (`crates/profiling/src/platform/unsupported.rs:8-55`); Android/iOS fall into the same unsupported branch. The browser build has its own always-unavailable profiler (`crates/devtools/src/profile/unavailable.rs:1-53`). Tests assert that unsupported platforms never advertise `cpu_profile` and never synthesize samples (`crates/devtools/tests/profile_tests.rs:235-290`).

## Memory

Flutter DevTools 3.50 charts Dart/Flutter heap, native memory, RSS and GC; it adds Profile Memory, Diff Snapshots, Trace Instances, and leak/retaining-path investigation ([Memory docs](https://docs.flutter.dev/tools/devtools/memory)).

Cove's Memory screen is deliberately a different measurement:

- Provider shape: `DiagnosticMemorySnapshot` (`crates/devtools/src/inspection/diagnostics.rs:57`), optional host provider (`270-272`), served by `query_ui_memory` (`791-794`; schema `crates/agent/src/ui_tools.rs:133`).
- Desktop installs shaping/text-layout/image cache counters and per-window render-target counters (`crates/hosts/src/desktop/diagnostics/devtools.rs:147`; snapshot assembly `crates/hosts/src/desktop/application/runtime.rs:1639-1704`). Android wires a memory callback through its diagnostics integration (`crates/hosts/src/android/integration/scene.rs:286-360`).
- Live run: the gallery returned `shapingBytes`, `layoutBytes`, `imageBytes`, entry counts, `skippedParentUpdates`, per-window `targetBytes`, `msaaLogicalBytes`, `msaaAllocatedBytes`, `samples`, `suspended`, `hasSwapchain`, and the real process counters `processResidentBytes` / `processVirtualBytes`. Cache and render-target values remain estimates; RSS/virtual are host process readings.
- `crates/hosts/src/process_memory.rs` reads macOS `proc_pidinfo(PROC_PIDTASKINFO)`, iOS `task_info(MACH_TASK_BASIC_INFO)`, Linux/Android `/proc/self/status`, Windows `K32GetProcessMemoryInfo`, and omits the counters honestly on WASM. Windows `processVirtualBytes` is private commit, not reserved address space (`crates/hosts/src/PLATFORM_COVERAGE.md`).
- The client labels process RSS/virtual separately and states the scope explicitly: real host process readings plus estimated cache/render values; still no heap objects, GC generations, retaining paths or leak tracking (`crates/devtools/client/inspector.js` Memory section).
- The browser probe installs no memory provider (`crates/browser/src/prototype/prototype.rs:131-164`), so `query_ui_memory` returns `null` there and the client shows the unavailable state. No heap or RSS fallback is substituted.
- Acceptance: `Tools/DevTools/acceptance/memory.sh` passed live with resident 189480960 bytes, virtual 501126529024 bytes (>= resident), all cache counters and 7 window counters.
- VRAM: Cove reports MSAA logical/allocated target bytes and target bytes; it does not query driver/GPU memory. This is a VRAM estimate, not measured VRAM.
- Allocation tracking: `crates/alloc` installs a real `TrackingAllocator` in the gallery and publishes current/peak/total/deallocated bytes, live/peak/total/deallocated blocks and size-class live bytes. `crates/hosts/src/allocation.rs` inserts the `alloc*` counters into `query_ui_memory` only when the allocator is installed. This is size/block tracking, not a type-aware object graph.
- Acceptance: `Tools/DevTools/acceptance/memory.sh` passed live with RSS/virtual and cache counters; `Tools/DevTools/acceptance/allocation.sh` passed with allocator totals growing from 181,426,954 to 214,951,493 bytes after a real `resize_ui`, then to 326,441,533 bytes after a second resize.
- Allocation-site profiling is implemented: `cove_alloc` samples every Nth allocation into a fixed ring without allocating; `start_ui_allocation_profile`, `stop_ui_allocation_profile` and `query_ui_allocation_sites` symbolize raw PCs with `cove-profiling`, filter allocator frames, and return bounded sites with bytes/blocks, attempted/retained/dropped, truncation and unsymbolized-address reasons. Live gallery: 120,507 sampled bytes / 1,024 sampled blocks in 6,040 attempts, 134 unique sites (64 returned), real `file:line` frames. It is cumulative sampled allocation, not live heap, object types or retaining paths.
- Heap/GC/leaks: Rust has no tracing GC, so Dart heap, GC events, retaining paths, and Dart leak tracking are **proven not applicable** as mechanisms. Type-aware heap object graphs, snapshot/diff and instance tracing are still runtime work; the allocator counters do not substitute for them.

## Network

Flutter DevTools 3.50 records HTTP, HTTPS, and WebSocket traffic, including `dart:io`, `dio`, and `http_profile` packages; it has pause/resume, filters, timing, headers, and bodies ([Network docs](https://docs.flutter.dev/tools/devtools/network)).

Cove records only what its own HTTP client sends:

- The recorder sits on `cove_http::Client::send`, is enabled by default in debug builds, keeps 256 records, truncates bodies at 16 KiB, and redacts credentials in headers (`crates/http/src/recording.rs:1-18`, `88-110`; schema `crates/agent/src/ui_tools.rs:139`; wire `crates/devtools/src/inspection/diagnostics.rs:1441-1471`).
- Test: a real local HTTP server returned 201/`pong`; the DevTools wire carried status, body, byte counts, and `<redacted>` Authorization (`crates/devtools/tests/network_tests.rs:44-120`).
- Client: Network view at `crates/devtools/client/index.html:300-326`, filters/search/state/follow at `319-325`, renderer at `crates/devtools/client/inspector.js:2452`. The working tree also wires the Pause/Resume toggle and Clear button through these tools, reads `enabled` from the query for the paused banner, and passes its fake-controller harness (`node crates/devtools/client/tests/network-harness.mjs`, 12 scenarios including a pre-paused runtime, target switches, disconnect and capability-absent targets). Real-controller browser acceptance for those controls has not been run. Flutter's "record from app startup" workflow is not available because the app registers its probe when it starts.
- WebSocket is absent: no WebSocket dependency or code exists in `crates/http`, and the recorder only sees `cove-http` requests. Other network stacks, OS traffic, and browser `fetch` are not recorded. This needs runtime work.
- Controls act on that process-wide recorder, not a client-side table. `set_ui_network_recording {enabled}` sends `network_recording`: pausing stops new records and keeps the retained history, resuming records new requests, and the reply carries the state `cove_http::recording::is_enabled()` actually holds. `clear_ui_network` sends `clear_network` and reports `cleared` (removed records, at most the 256-record bound) plus `limit`; the enabled state is unchanged. `query_ui_network` also reports the live `enabled` flag so a client never infers recording state from the table. The probe advertises `network_control`, and the controller refuses both tools on a runtime that does not advertise it; probe teardown restores the build's default recording state so a closed session cannot leave the process silently unrecorded (`crates/devtools/src/runtime_probe.rs`, `crates/devtools-cli/src/controller.rs`, `crates/devtools-cli/src/tools.rs`).
- Control tests: `crates/devtools/tests/network_control_tests.rs` sends real loopback requests through `Client::send`; a paused probe records nothing while preserving earlier records, resume records the next request, 300 `recording::begin` calls stay at the 256 bound, clear reports 256 removed then retains a later request, and probe teardown restores the build default. Controller fake-runtime tests cover both tool dispatches, the `network_control` gate (`unavailable`) and schema errors (`invalid_request`).
- Live run: the idle gallery made no HTTP calls, so `query_ui_network` reported 0 records and `limit: 256`; the tests are the verification for recorded requests. An isolated live gallery control run returned `{"enabled":false}`, then `{"enabled":true}` with the retained count unchanged, and `clear_ui_network` returned `{"cleared":0,"limit":256}` followed by an empty `query_ui_network`; because the gallery issues no HTTP, the recording tests prove pause/resume on real traffic.

## Debugger

Flutter DevTools 3.50 is a source-level Dart VM debugger: breakpoints, call stack, variables, stepping, evaluate, console, and exception modes ([Debugger docs](https://docs.flutter.dev/tools/devtools/debugger)).

Current Cove state at the audit snapshot:

- The client Debugger UI/JS is present: `crates/devtools/client/index.html:127-128`, `419-531` provides Attach/Detach, Pause/Continue, stepping, threads, stack, breakpoints, source, scopes, console, and evaluate; `crates/devtools/client/inspector.js:6466-8795` normalizes state and has explicit unavailable/not-attached rendering.
- The controller and DAP client are now implemented: `get_ui_debugger`, `attach_ui_debugger`, `detach_ui_debugger`, `set_ui_breakpoint`, `clear_ui_breakpoint`, `control_ui_debugger`, `get_ui_debug_stack`, `read_ui_debug_variables`, and `evaluate_ui_debug` are published (`crates/devtools-cli/src/tools.rs:211-219`) and dispatched (`crates/devtools-cli/src/controller.rs:707-873`). Sessions are keyed by runtime PID and owned by the controller (`crates/devtools-cli/src/debugger.rs:1-40`), not by the UI probe. The `crates/debugger` crate wraps `lldb-dap` framing, process cleanup, and DAP state; adapter discovery, version checks, attach, control, and cleanup run with hard timeouts and process-group cleanup (`crates/debugger/src/process.rs`). A real attach-denied test (`a_real_attach_is_denied_without_developer_mode_and_leaves_no_debugserver`) exists for that cleanup but was not run because it would prompt.
- Live controller check (no attach attempted): `get_ui_debugger` returned the discovered adapter (`/Library/Developer/CommandLineTools/usr/bin/lldb-dap`, LLVM 21.0.0, liblldb lldb-2103.0.34.407), `available: true`, `developer_mode: false`, `session: null`, and the warning "Developer mode is disabled, so macOS can deny task_for_pid attach...". `get_ui_debug_stack`, `control_ui_debugger` (pause), and `read_ui_debug_variables` all returned `no_session` with "Attach the UI debugger to this runtime first." No paused state was invented.
- Debugger tests: the crate's 13 mock tests and 6 framing tests pass, including attach refusal → `unavailable` and session closed, "paused state only comes from stopped events", a step without a stopped event is a timeout rather than a fake pause, and detach does not terminate the debuggee. The real attach-denied test was not run because it would exercise the TCC/authorization prompt on this host.
- The client Debugger module (`crates/devtools/client/inspector.js`, `emptyDebugger` through `resetDebugger`) calls the specific tools via `debuggerCall`: `attach_ui_debugger`, `detach_ui_debugger`, `control_ui_debugger`, `get_ui_debug_stack`, `read_ui_debug_variables`, `set_ui_breakpoint`/`clear_ui_breakpoint`, and `evaluate_ui_debug`. A headless-Chrome fake-controller harness (`crates/devtools/client/tests/debugger-harness.mjs`) exercised the real client against the controller response shapes and passed all eight scenarios: a DOM audit, unavailable adapter, developer-mode warning, refused attach, attach timeout, a narrow-window layout check, the full attach/pause/step/continue/threads/stack/scopes/variables/evaluate/breakpoints/detach flow, and all six rail views rendering without a page exception. It asserts no paused state appears without a stopped response. Real `lldb-dap` attach is the only unverified part.
- Adapter feasibility is established: `xcrun --find lldb-dap` → `/Library/Developer/CommandLineTools/usr/bin/lldb-dap` (LLVM 21.0.0, liblldb lldb-2103.0.34.407); DAP `initialize` works. DAP is the right analogue to the Dart VM service for native Rust frames.
- Attach is currently authorization-blocked. `DevToolsSecurity -status` reports "Developer mode is currently disabled." `codesign -dv target/debug/gallery` reports `flags=0x20002(adhoc,linker-signed)`, `Signature=adhoc`, `TeamIdentifier=not set`, and no entitlements; `DevToolsSecurity` disabled plus no `get-task-allow` means `task_for_pid` cannot attach without an authorization prompt/developer-mode change.
- Symbols: `target/debug/gallery` has no dSYM and no `__DWARF` section (`otool -l`), while split-DWARF `.rcgu.o` files remain under `target/debug/deps`. Any native debugger must resolve split DWARF or the project must emit a dSYM; LLDB can use the object files, but this must be verified on a disposable process.
- Rust vs Dart model: native frames, scopes, and LLDB expressions replace Dart isolates, `toString()` evaluation, async stacks, and Dart VM breakpoints. The Dart VM debugger model is **proven not applicable**; the user-facing parity target is source-level native debugging.
- Design constraint now satisfied by the controller: a paused app cannot poll `RuntimeProbe`, so the session is controller-owned and keyed by PID, and every attach failure returns explicit `unavailable`/`timeout` with the adapter's reason. Attach itself is still unverified on this host because authorization is disabled.

## Logs

Flutter DevTools 3.50 shows GC events, framework events, stdout/stderr, and custom logs ([Logging docs](https://docs.flutter.dev/tools/devtools/logging)).

- Cove serves the runtime's recent `cove_logging` records, capped at 100 per answer, with `dropped` and sequence fields (`crates/devtools/src/runtime_probe.rs:243-258`; tool `crates/devtools-cli/src/tools.rs:206`).
- Client: Logs view `crates/devtools/client/index.html:328-357`; search, minimum level, follow, refresh; renderer `crates/devtools/client/inspector.js:2615`, `2674`.
- Live run: 3 records, 0 dropped. `cove_logging` captures process stdout/stderr sources (`crates/logging/src/formatting.rs:40-45`).
- Dart GC and Flutter framework events are **proven not applicable**; host/application logs are covered. `clear_ui_logs` clears the runtime's actual replay history and the cleared-window dropped count while keeping sequences monotonic; the client Clear button waits for the acknowledgement, starts a fresh follow window, and shows a cleared/empty state. A live gallery run cleared 3 records (`{"cleared":3,"dropped":0,"firstAvailableSequence":null}`) and the next query returned 0 records. Flutter's GC-only filters and framework event categories have no equivalent (and no need for GC).

## App Size

Flutter DevTools 3.50 analyzes `--analyze-size` files with treemap, table, dominator tree, call graph, and a diff tab ([App Size docs](https://docs.flutter.dev/tools/devtools/app-size)).

Cove implements the applicable native-executable slice:

- `get_ui_app_size` resolves the selected runtime's local PID to its executable (macOS `proc_pidpath`, Linux `/proc/<pid>/exe`, Windows `QueryFullProcessImageNameW`; remote/device/display targets and unresolvable PIDs return `unavailable`). There is no arbitrary path input.
- The controller parses the file with `object`, demangles Rust symbols with `rustc-demangle`, and returns bounded total bytes, top sections (name/kind/size/address), top symbols, and top crate/group sums with explicit truncation. Dart AOT package/library/class/function attribution is **proven not applicable** because Cove has no Dart AOT output.
- The client App Size view labels this as the running native executable, not Flutter/Dart AOT, and renders sections/symbols/groups with refresh and explicit empty/error/unavailable/truncation states. It also has a proportional squarified treemap for groups/sections (deterministic, click-to-focus rows, labels/tooltips) and a snapshot baseline diff (save/export/import, added/removed/changed bytes and percentages, timestamps, truncation notices, explicit no-baseline state).
- Live verification: the gallery's executable returned 20 sections (e.g. `__TEXT,__text` 31,747,324 bytes), 20 symbols and 20 groups; top groups included `core`, `alloc`, `cove_ui_compose`. `Tools/DevTools/acceptance/app-size.sh` passed 14 checks live (`file_bytes=114,517,928`, real sections/symbols/groups, consistent truncation, display target unavailable). Tests parse the test executable, reject invalid files, resolve the current PID where the OS allows, and pin controller registration/dispatch; the client fake-CDP harness covers treemap, focus, diff add/remove/change, malformed snapshots, truncation and unavailable states.
- Still missing: dominator tree, call graph, build-artifact comparison, and Dart AOT/package attribution (the latter is **proven not applicable**).

## Deep Links

Flutter DevTools 3.50 validates a Flutter project's deep-link setup (website association, manifest, universal links) and gives fix instructions ([Deep Links docs](https://docs.flutter.dev/tools/devtools/deep-links)).

Cove validates the platform configuration that turns a URL into an app launch, using its own project layout rather than Flutter's Dart/pub layout:

- `get_ui_deep_links {root}` takes one canonical absolute project root and reads only allowlisted files beneath it: `Platforms/Android/app/src/main/AndroidManifest.xml`, the macOS `Platforms/macOS/Info.plist`, `Info.plist`/`*.entitlements` under `Platforms/macOS` and `Platforms/iOS` (bounded scan), plus `assetlinks.json` and `apple-app-site-association` at the known `.well-known`/platform paths (`crates/devtools-cli/src/deep_links.rs`). The root is canonicalized; non-absolute roots, `..` components and any file whose canonical target leaves the root are rejected with `invalid_request`. Each file is limited to 4 MiB, and only regular files are read.
- Android is parsed with `quick-xml`: every intent filter that declares a scheme reports its activity, schemes, hosts, paths, VIEW action, BROWSABLE category, `autoVerify` and line. Issues cover malformed XML, invalid scheme/host, a scheme without a host, a missing VIEW action or BROWSABLE category, missing `autoVerify` for web hosts, duplicate filters and a missing `assetlinks.json` for a web host.
- Apple is parsed with `plist` (XML and binary property lists): `CFBundleURLTypes`/`CFBundleURLSchemes` supply URL schemes; `com.apple.developer.associated-domains` supplies `applinks:`, `webcredentials:`, `appclips:` and `activitycontinuation:` domains. Issues cover malformed or non-dictionary plists, invalid schemes/domains, duplicate schemes/entitlements and a missing `apple-app-site-association` for an applink domain.
- The association files themselves are validated as real JSON: Digital Asset Links statements need `delegate_permission/common.handle_all_urls` plus an `android_app` target with `package_name` and a SHA-256 fingerprint; AASA needs a known section and non-empty `applinks.details` `appID`/`appIDs` entries.
- Results are `{root, files, android:{configured,schemes,hosts,links,issues}, apple:{configured,schemes,domains,associatedDomains,issues}, summary}`. Every issue carries `severity`, `code`, `message`, `file`, `location` (`file:line` when the parser has one), and a concrete `fix`. Missing platform files set `configured:false` and produce no issue; the empty result is `"No deep links configured."`.
- Tests in `crates/devtools-cli/src/deep_links.rs` cover a valid Android+Apple project, iOS nested layout discovery, missing BROWSABLE/VIEW/`autoVerify`, scheme without host, duplicate links/schemes/domains, legitimate cross-platform scheme/domain reuse, malformed manifest/plist/associations, traversal and symlink escapes, a file root, and an empty project. The controller tests publish the tool and call it against `examples/gallery`.
- Live gallery call through `cove-devtools serve` + `cove-devtools call`: `configured:false`, empty schemes/hosts/domains, no issues, files `AndroidManifest.xml` and `macOS/Info.plist`, summary `"No deep links configured."` (the manifest has only MAIN/LAUNCHER and the plist has no URL types). `cargo test -p cove-devtools-cli` passed 68/68.
- Limits: validation is local configuration only. It does not fetch the live `/.well-known` files, match `assetlinks` package names to the Gradle application ID, verify AASA `appID` team/bundle IDs, or deliver URLs at runtime. Mobile hosts still receive URLs through their platform launch paths; Linux/Windows have no equivalent association-file format.


## Provider and DevTools extensions

Flutter DevTools extensions dynamically load third-party tooling from pub packages (for example Provider), and the Provider extension makes app state visible in DevTools ([Extensions docs](https://docs.flutter.dev/tools/devtools/extensions)).

- Cove has no extension host and no third-party UI loading. DevTools extensions as a Dart/pub mechanism are **proven not applicable**; a Rust extension ABI would be a new architecture (versioned UI/plugin boundary, trust model), not a port.
- The closest existing capability is the opted-in state registry: `query_ui_state` lists only registered metadata and reads summaries for explicit ids (`crates/agent/src/ui_tools.rs:178-192`; dispatch `crates/devtools/src/inspection/diagnostics.rs:860-880`). Tests cover that summaries are not evaluated eagerly and only requested ids are read (`crates/devtools/tests/diagnostics_tests.rs`, `state_metadata_does_not_evaluate_values_and_reads_only_requested_ids`). This is app-declared state, not a provider graph.
- Provider-specific provider values, dependencies, and `ChangeNotifier` relationships would need library-specific integration; Cove's inspector shows semantics/properties and the state registry instead. Status: state inspection implemented + verified; extension host and provider graph need architecture work.

## Hot reload and hot restart

Flutter hot reload injects updated Dart kernel code into the running VM, rebuilds the widget tree, and preserves app state; hot restart re-runs the program from the start. Native code changes require a full restart ([Hot reload docs](https://docs.flutter.dev/tools/hot-reload)).

Cove has neither. There is no `reload`, `restart`, or app-lifecycle command in the controller tool list (`crates/devtools-cli/src/tools.rs:184-220`); the gallery is launched externally and the controller only discovers/attaches to it.

### Time-boxed read-only feasibility probe

Probe steps (no product edits):

- `rust-toolchain.toml:1-4`: pinned toolchain `1.99.1`, profile `minimal`, components rustfmt/clippy.
- `otool -L target/debug/gallery`: only Apple system frameworks (WebKit, JavaScriptCore, Security, Foundation, QuartzCore, AppKit, AVFoundation, AudioToolbox, CoreAudio, CoreFoundation, CoreGraphics, ...). No project dylib.
- `examples/gallery/Cargo.toml`: a normal `bin`/`lib` package dependency graph; `examples/gallery/src/bin/gallery.rs` is a three-line `main` calling `cove_examples::run()`, and `examples/gallery/src/lib.rs:21` installs the app with `cove_hosts::application!(launch = demo::application)`. All Cove crates are statically linked.
- The only app-side `cdylib` target in the workspace is the browser reactor (`crates/browser/Cargo.toml:10`); it is the WASM build, not the desktop app.
- No hot-reload/patching dependency is present (`rg` over `Cargo.lock`/manifests found no `subsecond`, `hot-lib-reloader`, or equivalent). `libloading` appears only transitively; the in-tree `dlopen` uses are for X11, Vulkan, WebKit, and Android bridge libraries, not app code.
- App state is in-process: the probe owns the tree/timeline/profile and `RuntimeProbe::disconnect` resets editor state, captures, and the profiler (`crates/devtools/src/runtime_probe.rs:260-265`). There is no state serialization/restore or app relaunch path.
- `hot-lib-reloader` requires the reloadable code to live in a `dylib`, with `#[unsafe(no_mangle)]` non-generic functions and stable type layouts; changing signatures/types can crash, and macOS requires signing the reloadable library ([hot-lib-reloader README](https://github.com/rksm/hot-lib-reloader-rs)). Cove's UI/app code is generic Rust across many crates, so this is an architectural split, not a drop-in.
- `subsecond` patches hot function call sites and requires marking entry points with `subsecond::call`/`subsecond::current`; it is not a transparent reload of an existing static binary ([Subsecond README](https://github.com/DioxusLabs/dioxus/blob/main/packages/subsecond/README.md)).
- The current gallery is ad-hoc/linker-signed with no entitlements; a dylib or in-memory patching approach would also need its signing/entitlement story resolved on macOS.

**Blocker:** there is no code-replacement boundary in the running app. Rust has no stable ABI for arbitrary generic UI code, the desktop app is one static Mach-O, state is owned by the process, and the controller does not own app launch. A DevTools-only hot reload cannot be implemented honestly. Options, none implemented:

1. Dev-only process restart with explicit state snapshot/restore. This is hot-restart-like, loses native/engine state, and requires serializable app state that does not exist.
2. A dylib/patch-boundary spike (`hot-lib-reloader` or `subsecond`) on a disposable example: split app code into a reloadable library, define a stable state handoff, and measure whether the Cove UI tree can be rebuilt without restarting the host. This is the only path to state-preserving code reload, and it is a significant architecture change.
3. Browser-only fast reload: rebuild the WASM reactor and reload the page. That restarts the module and loses state; it is not Flutter-equivalent hot reload.

## Flutter-only mechanisms (proven not applicable)

These are Dart/Flutter mechanisms, recorded so they are not tracked as missing Cove features:

- Dart VM isolate debugging, `toString()` evaluation, async stacks, and Dart breakpoints. (Native DAP/LLDB is the applicable substitute and is tracked above.)
- Dart heap, GC events, retaining paths, and Dart leak tracking. (Rust allocator size/block tracking is implemented; a type-aware heap object graph and retaining paths are not.)
- Dart kernel hot reload and `--pause-isolates-on-start`.
- Flutter widget/render-element tree and flex object model. (Cove's node/component tree is the substitute.)
- Flutter AOT size attribution. (Native binary size analysis is applicable and missing.)
- DevTools extensions distributed as pub packages.
- Flutter UI/raster thread split and Flutter shader-compilation attribution. (Cove measures its own CPU phases; GPU/refresh are explicitly out of scope.)

## Platform gaps (explicit)

| Capability | macOS | Linux | Windows | Android | iOS | Web/WASM |
| --- | --- | --- | --- | --- | --- | --- |
| Inspector tree/query | verified live | shared Rust path, not exercised here | shared Rust path, not exercised here | shared Rust path, not exercised here | shared Rust path, not exercised here | browser probe path, not exercised here |
| App frame capture / pick | Metal framebuffer | DX12 or Vulkan framebuffer | Vulkan framebuffer | Metal framebuffer | Vulkan framebuffer | WebGPU or Canvas2D canvas |
| CPU sampler | verified (arm64) | unavailable (`crates/profiling/src/platform/unsupported.rs:8-55`) | unavailable (`crates/profiling/src/platform/unsupported.rs:8-55`) | unavailable | unavailable | unavailable (`profile/unavailable.rs`) |
| Memory counters | verified live | desktop provider, not exercised here | desktop provider, not exercised here | callback wired (`crates/hosts/src/android/integration/scene.rs:286-360`), not exercised | not verified | no provider (returns `null`) |
| HTTP recording | verified by test | shared path | shared path | shared path | shared path | `cove-http` only; browser `fetch` not recorded |
| WebSocket | absent | absent | absent | absent | absent | absent |
| Heap/GC/leak profiling | absent | absent | absent | absent | absent | absent |
| Debugger | implemented; attach authorization-blocked | controller path, not exercised | controller path, not exercised | no local DAP target | no local DAP target | no local DAP target |
| App Size | native executable verified | implemented, not run here | implemented, not run here | unavailable (device PID) | unavailable | absent |
| Deep Links validator | implemented (project files) | not applicable (no platform file) | not applicable (no platform file) | implemented (project files) | implemented (project files) | not applicable (no platform file) |
| Hot reload/restart | absent | absent | absent | absent | absent | absent |

The app-local capture path uses the runtime surface on each platform.

## Verification performed

- `cargo build -p cove-devtools-cli --bins` and `cargo build -p cove-examples --bin gallery`: success, `Finished dev profile ... in 15.54s`; warnings were existing `cove-shell` CVDisplayLink deprecations.
- `cargo test -p cove-devtools-protocol -p cove-devtools -p cove-devtools-cli`: **81 passed, 0 failed** (12 frame-analysis unit, 6 diagnostic presentation, 12 diagnostics, 5 frame-analysis integration, 7 inspection geometry, 1 network, 3 profile, 2 tree summary, 27 controller, 6 protocol), plus 0 doc-tests. The run included the real macOS sampler test.
- `cargo check -p cove-debugger`: success. `cargo test -p cove-debugger tests::mock::` 13 passed; `cargo test -p cove-debugger framing::` 6 passed. The real attach-denied test was not run (TCC prompt).
- `node crates/devtools/client/tests/debugger-harness.mjs`: **RESULT PASS**, eight scenarios (DOM audit, unavailable, developer-mode warning, refused attach, attach timeout, narrow-window layout, full paused flow, all rail views), using a fake controller and headless Chrome; it starts no Rust process and attaches to nothing.
- Isolated live controller + gallery (ephemeral port, temporary discovery file, both processes killed after):
  - tree: 1,884 nodes; capabilities `tree, diagnostics, style, trace, timing, frame_analysis, network, inspect_nodes, resize, environment, content_geometry, cpu_profile`.
  - debugger: adapter discovered, `available: true`, `developer_mode: false`, `session: null`; stack/control/variables before attach return `no_session`/"Attach the UI debugger to this runtime first."
  - trace: 756 spans; analysis of frame 20: 741 spans, dominant `recomposition` at 96.5%.
  - profile: 282 samples/803 ms at 500 Hz, 2 stacks, real `file:line` hotspot.
  - memory: cache counters and per-window target/MSAA estimates; logs: 3 records; network: 0 records (no requests).
  - client tree transport: 1,133,706-byte `/rpc` result; the full-tree CLI path refuses it at the 512 KiB agent-tool budget, while `query_ui_tree` paged calls stay under it.
- Not run: real attach (would prompt/authorize against the user's session), real-controller browser DOM rendering for Inspector/Performance/Memory/Network (the Debugger harness renders all rail views against a fake controller), and Android/iOS/Windows/Linux/Web devices.

### Concurrent work

The Debugger landed during this audit: the `crates/debugger` DAP crate compiles, has mock/framing tests, and is now a `cove-devtools-cli` dependency; the controller publishes and dispatches the debugger tools, and the client UI/JS exists. The client harness passes; only a real attach-denied run against a disposable process remains. The devtools workspace suite (M1–M3 plus the debugger controller tests) now passes 81/81, so the debugger work did not regress the earlier milestones.

## Remaining blockers and next decisions

1. **Debugger**: verify attach denial plus a stopped-stack/variable read against a disposable process. Resolve macOS `get-task-allow`/developer-mode authorization and split-DWARF symbol lookup.
2. **Inspector capture**: Run `Tools/DevTools/acceptance/m1-inspector.sh` to check live frame capture and picking.
3. **Performance**: add call tree/bottom-up/method table/flame chart to CPU profiles; add frame-analysis hints and enhanced-tracing controls if Flutter parity is required.
4. **Memory**: add type-aware heap object snapshot/diff and instance tracing if desired; allocator size/block tracking, RSS/virtual, allocation-site sampling and allocation-site baseline diff are now real. Keep the UI explicit that cache counters are estimates and GC is absent.
5. **Network**: WebSocket and non-`cove-http` traffic; pause/resume.
6. **App Size**: add dominator tree/call graph and build-artifact comparison over the real native section/symbol data. Deep-link validation now covers local project configuration; provider/extension gaps remain separate.
7. **Hot reload**: decide whether to fund a dylib/patch-boundary spike; do not schedule it as a DevTools-only task.
8. **Agent tooling**: the full `inspect_ui_tree` still exceeds 512 KiB on the gallery; use the real paged `query_ui_tree` (verified) or `query_ui_nodes`.
