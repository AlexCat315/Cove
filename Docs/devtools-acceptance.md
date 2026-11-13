# DevTools acceptance

Acceptance runs exercise the controller and a real Gallery runtime.

## Milestone 1 — App-local capture and picking

```sh
Tools/DevTools/acceptance/m1-inspector.sh
Tools/DevTools/acceptance/m1-inspector.sh --keep
```

The harness starts an isolated controller and Gallery on macOS. The Gallery starts without taking keyboard focus. The harness selects the registered runtime by process ID, reads its framebuffer through `capture_ui`, and checks the PNG dimensions and decoded pixels. It uses `select_ui_at` to round-trip an image pixel through the Cove hit tester and verifies the node ID, path, and frame. It then checks the dimensions, pixels, and node metadata returned by `capture_ui_node`. Pointer coordinates start at the top-left of each returned image, including a node crop.

On 2026-11-13 the macOS run passed all seven checks. It decoded a 2560×1440 Gallery frame and a 48×32 node crop. Separate live runs verified Button navigation, Slider drag, sidebar scrolling, text entry, and Backspace with runtime postconditions and traces. Headless Chrome verified both Canvas2D and WebGPU frame readback and input, including Inspector interaction with a background WebGPU tab.

The harness uses Cove frame readback and scene queries. It requires a macOS GUI session to start Gallery. The Metal readback regression test also renders and reads pixels without creating a native window.

## Milestone 3: CPU profiler

Script: `Tools/DevTools/acceptance/profile.sh`

```sh
Tools/DevTools/acceptance/profile.sh              # builds, then removes its run directory on success
Tools/DevTools/acceptance/profile.sh --keep       # keeps logs and profile JSON
Tools/DevTools/acceptance/profile.sh --no-build   # reuses target/debug binaries
```

The run exercises the real in-process macOS sampler through the public controller tools. It is isolated from the user's live controller and makes no product-code changes:

- It builds `target/debug/cove-devtools` and `target/debug/gallery` with cargo unless `--no-build` is passed, then launches those binaries directly. Direct PIDs let cleanup stop exactly its own processes.
- It writes a fresh discovery file under `$TMPDIR/cove-devtools-m3.*`, exports `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_ENDPOINT` and `COVE_DEVTOOLS_TOKEN` to the controller, the gallery and every CLI call, and picks an ephemeral loopback port. Port 9229 and the user's discovery file are never used.
- It selects the runtime by the gallery PID it launched, falling back to the title `CoveUI — Component Showcase` only if the PID is absent. Runtime capability checks use that registration.
- On exit, `EXIT`/`INT`/`TERM` traps stop the gallery and controller by PID (TERM, then KILL after five seconds). Both background jobs are disowned so bash does not print job-control notices during cleanup. Logs and JSON are kept on failure and removed on success unless `--keep` is passed.
- It requires no TCC permission. Profiling is an in-process Mach sampler following the `cove-ui` thread, so the controller's `Screen Recording is off` and `Accessibility is off` warnings do not block this run.

### Exact steps

1. Preflight macOS, `arm64`/`x86_64`, `cargo`, `python3` and `jq`; create the isolated run directory; build both binaries.
2. Read `cove-devtools tools` and require `start_ui_profile`, `stop_ui_profile`, `get_ui_profile` and `query_ui_hotspots`. Every profile tool must accept `target`; `start_ui_profile` must require `hz` and `duration_ms`.
3. Start the controller:

   ```sh
   COVE_DEVTOOLS_SESSION_FILE=... COVE_DEVTOOLS_TOKEN=... \
     target/debug/cove-devtools serve --port <ephemeral>
   ```

   Poll `get_ui_session '{}'` until the discovery file exists and the controller answers with a `targets` array.
4. Start the gallery with `COVE_DEVTOOLS=1` and the same isolated environment, then poll `get_ui_session` until a runtime with the gallery PID registers.
5. Require runtime `platform = "macos"` and `capabilities` containing `cpu_profile`. Missing capability is a blocked run, never a pass.
6. Start the first profile:

   ```json
   {"target":"<runtime>","hz":500,"duration_ms":1500}
   ```

   Require `status = "running"`, `thread = "cove-ui"`, `hz = 500`, an empty `stacks` array and a numeric `samplesDropped`.
7. Poll `get_ui_profile {"target":"<runtime>"}` every 100 ms until `status = "ready"`, bounded by `ACCEPTANCE_PROFILE_TIMEOUT` (default 30 s). A transition to `idle` or an unexpected status fails the run.
8. Assert the ready profile against the real wire shape: `samplesTaken > 0`, `samplesDropped` numeric, `samplesPerSecond > 0`, `durationMs > 0`, `sampleIntervalMs > 0`, `retainedSamples > 0` and not above `samplesTaken`, `thread` equal to the real UI thread, `hz = 500`, non-empty `stacks`, non-empty labelled frames, and a stack sample sum within `retainedSamples`.
9. Query hotspots with `{"target":"<runtime>","limit":200}`. Require `status = "ready"`, matching thread/sample counters and at least one hotspot. Reject malformed entries (`selfSamples > inclusiveSamples`, inclusive above retained, `inclusiveMs < selfMs`, negative times, out-of-range self or inclusive percentages). Prefer and print a hotspot carrying `function`, `filePath` and `line`; if none resolves `file:line`, warn instead of inventing one.
10. Start a longer profile with `{"hz":500,"duration_ms":5000}`, wait until the live `samplesTaken` counter reaches 10, pause 200 ms, then call `stop_ui_profile`. Require `status = "ready"`, positive samples and stacks, and `durationMs < 5000` so the proof is an early stop rather than the bound.
11. Read `get_ui_profile` again and require the stopped profile stays `ready` with the same `samplesTaken`. Call `stop_ui_profile` a second time and require a non-zero CLI exit with error code `no_profile`. Read once more and require the rejected stop did not change the retained profile.
12. Print `RESULT PASS` and exit 0.

### Wire contracts asserted

`UiProfile` is returned directly by `start_ui_profile`, `stop_ui_profile` and `get_ui_profile` (not wrapped in `ok`/`data`). `status` is `idle`, `running` or `ready`; while running, counters are live and `stacks` is empty.

```json
{
  "status": "ready",
  "thread": "cove-ui",
  "hz": 500,
  "durationMs": 1503.954417,
  "sampleIntervalMs": 2.0,
  "samplesTaken": 510,
  "samplesDropped": 0,
  "retainedSamples": 510,
  "samplesPerSecond": 339.1060222538646,
  "stacksTruncated": false,
  "stacks": [
    {"samples": 509, "percent": 99.80392156862746,
     "frames": [{"label": "__psynch_cvwait", "function": "__psynch_cvwait", "filePath": null, "line": null}]}
  ]
}
```

`UiHotspots` is returned directly by `query_ui_hotspots`. Self time is exclusive; inclusive time counts a frame once per sample.

```json
{
  "status": "ready",
  "thread": "cove-ui",
  "hz": 500,
  "durationMs": 1503.954417,
  "sampleIntervalMs": 2.0,
  "samplesTaken": 510,
  "samplesDropped": 0,
  "retainedSamples": 510,
  "truncated": false,
  "hotspots": [
    {"label": "__psynch_cvwait", "function": "__psynch_cvwait", "filePath": null, "line": null,
     "selfSamples": 509, "inclusiveSamples": 509,
     "selfMs": 1500.9804999999983, "inclusiveMs": 1500.9804999999983,
     "selfPercent": 99.80392156862746, "inclusivePercent": 99.80392156862746}
  ]
}
```

The second stop is a controller fault. The CLI prints the JSON-RPC error object and exits non-zero; the harness reads the first string code from `data.code`, `error.code` or `code`, so the landed `no_profile` is asserted regardless of the envelope:

```json
{"code": -32000, "message": "Start a UI profile before stopping one.", "data": {"code": "no_profile"}}
```

### Pass conditions

Exit code 0 and every line below printed:

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>)
PASS  runtime advertises cpu_profile (platform macos, arm64)
PASS  sampling started: thread=cove-ui hz=500, stacks empty while running
PASS  profile ready: N samples in D ms (R/s, 0 dropped, N retained, K stacks)
PASS  real UI thread 'cove-ui' sampled at 500 Hz (interval 2.0 ms); top frame '<label>'
PASS  hotspots: H entries; top '<label>' with S self samples
PASS  source-attributed hotspot: <function> at <file>:<line> (S self samples)
PASS  early stop: N samples in D ms (bound 5000 ms, N retained, K stacks)
PASS  the stopped profile stays readable: status ready with N samples
PASS  second stop returned explicit no_profile: <message>
RESULT PASS  Milestone 3 CPU profiler: live samples, stacks, hotspots and early stop
```

The source-attribution line is replaced by `WARN` when no hotspot carries both a function and a source line. That is not a pass claim about symbols; the hotspot labels still come from live sampled addresses.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; live samples, stacks, hotspots and early stop all asserted |
| 1 | Functional failure: wrong thread/status/rate, zero samples, empty stacks or hotspots, malformed counters, early stop ignored, no_profile not returned, TCC-independent controller contract mismatch |
| 2 | Blocked: non-macOS host, unsupported architecture, missing dependency, build failure, missing profile tool, controller/gallery did not start, runtime platform not macOS, `cpu_profile` not advertised, sampler reports `unavailable` |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **`cpu_profile` not advertised**: the harness exits 2 with the runtime capabilities and points at `cargo test -p cove-devtools --test profile_tests`. On macOS this means the sampler is unavailable to the launched build (for example a non-native gallery binary or a broken `crates/profiling` platform module), not an empty profile.
- **`unavailable` from any profile tool**: blocked with the controller's reason. No samples, stacks or hotspots are synthesized.
- **No GUI session**: the gallery cannot register a runtime; blocked with the gallery log. Profiling itself does not need pixels, but the runtime must be running.
- **Port or startup race**: a failing build blocks with the cargo log; a busy ephemeral port makes the controller exit, which is reported with the controller log.
- **Thread name**: the gallery's UI thread is `cove-ui`. `ACCEPTANCE_UI_THREAD` overrides the expected name if the product renames it; a mismatch is a functional failure because sampling a different thread would not prove UI attribution.
- **Sample rate is measured, not nominal**: `samplesPerSecond` can be below the requested 500 Hz when the UI thread sleeps between wakeups. The assertion is `> 0`; the harness does not fabricate a rate or require the requested cadence.
- **Idle UI work**: an idle gallery makes `__psynch_cvwait` the top hotspot. That is honest sampling of the UI thread, not a placeholder. A `function`/`filePath`/`line` triple is not guaranteed for system frames; the harness warns rather than inventing symbols.
- **Bounds**: the wire returns at most 24 aggregated stacks and 200 hotspots; per-sample stacks are truncated at 32 frames. `stacksTruncated`/`truncated` are reported, and the harness checks the stack sample sum against `retainedSamples`, not against the full stack set.
- **Early stop timing**: the early stop is taken after the live counter reaches 10 samples plus 200 ms. Controller CLI startup, sampling and symbolization add latency, so the observed duration is typically several hundred milliseconds; the assertion is only that it is shorter than the 5,000 ms bound.
- **No product Rust or client changes**: the harness only invokes public controller tools and reads JSON.

### Current verification status

`bash -n Tools/DevTools/acceptance/profile.sh` passes. Three full runs against an isolated controller and the real gallery passed end-to-end with exit 0 and 11 checks each; the first two used `--keep`, and the third used defaults and removed its run directory afterwards. Every controller reported Screen Recording and Accessibility off; that did not affect profiling. The first run sampled 509 times in 1504.372 ms with 0 dropped, 509 retained and 11 stacks; the early stop returned 269 samples in 778.036 ms. The second run sampled 510 times in 1503.954 ms with 0 dropped, 510 retained and 2 stacks, and its source-attributed hotspot was a real symbol:

```text
core::sync::atomic::atomic_load::<usize> at .../library/core/src/sync/atomic.rs:4001 (1 self samples)
```

The third run sampled 511 times in 1501.22175 ms with 0 dropped, 511 retained and 24 stacks; the wire reported `stacksTruncated=true` and the harness printed its truncation warning. Its 65 hotspots included a source-attributed `core::ptr::copy_nonoverlapping::precondition_check` at `core/src/ub_checks.rs:68`, and the early stop returned 170 samples in 516.657 ms against the 5000 ms bound. The second stop returned `no_profile` in every run. A negative run with `ACCEPTANCE_UI_THREAD=definitely-not-cove-ui --no-build` exited 1 at the thread assertion and cleaned up both processes, proving the failure path and PID cleanup. No controller or gallery process remained afterwards.

## Memory: cache counters, render-target estimates and process RSS

Script: `Tools/DevTools/acceptance/memory.sh`

```sh
Tools/DevTools/acceptance/memory.sh              # builds, then removes its run directory on success
Tools/DevTools/acceptance/memory.sh --keep       # keeps logs and the memory JSON
Tools/DevTools/acceptance/memory.sh --no-build   # reuses target/debug binaries
```

The run reads the live host's memory snapshot through the public controller tool. It asserts the counters that already exist (logical text/image caches and per-window render-target estimates) and then requires a real process resident reading. A host that installs no memory provider, or reports no process counters, blocks the run; cache bytes are never accepted in place of RSS and the harness never invents a reading. Dart heap, GC events, retaining paths and leak tracking are not applicable to a Rust host, so the harness does not assert or simulate them.

The run is isolated from the user's live controller and makes no product-code changes:

- It builds `target/debug/cove-devtools` and `target/debug/gallery` with cargo unless `--no-build` is passed, then launches those binaries directly. Direct PIDs let cleanup stop exactly its own processes.
- It writes a fresh discovery file under `$TMPDIR/cove-devtools-memory.*`, exports `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_ENDPOINT` and `COVE_DEVTOOLS_TOKEN` to the controller, the gallery and every CLI call, and picks an ephemeral loopback port. Port 9229 and the user's discovery file (`$HOME/Library/Application Support/Cove/devtools-controller.json`) are never read or written. A guard blocks the run if the isolated session path ever resolves to that file, and the discovery file the controller writes must carry the isolated endpoint before any assertion runs.
- It selects the runtime by the gallery PID it launched, falling back to the title `CoveUI — Component Showcase` only if the PID is absent, and requires `platform = "macos"`.
- On exit, `EXIT`/`INT`/`TERM` traps stop the gallery and controller by PID (TERM, then KILL after five seconds). Both background jobs are disowned so bash does not print job-control notices during cleanup. Logs and `artifacts/memory.json` are kept on failure and removed on success unless `--keep` is passed.
- It needs no TCC permission. The controller may print `Screen Recording is off` and `Accessibility is off`; memory counters are host data, not pixels, so those warnings do not block this run.

### Exact steps

1. Preflight macOS, `cargo`, `python3` and `jq`; create the isolated run directory; build both binaries unless `--no-build`.
2. Read `cove-devtools tools` and require `query_ui_memory`. Extra required arguments beyond the optional `target` block the run, because the harness sends only `target`.
3. Start the controller:

   ```sh
   COVE_DEVTOOLS_SESSION_FILE=... COVE_DEVTOOLS_TOKEN=... \
     target/debug/cove-devtools serve --port <ephemeral>
   ```

   Poll `get_ui_session '{}'` until the discovery file exists and the controller answers with a `targets` array. Require the discovery endpoint to equal `COVE_DEVTOOLS_ENDPOINT`.
4. Start the gallery with `COVE_DEVTOOLS=1` and the same isolated environment, then poll `get_ui_session` until the runtime with the gallery PID registers.
5. Call `query_ui_memory` with `{"target":"<runtime>"}`, retrying three times only on transport-level failures. `unavailable`, `target_gone`, `target_required` and `target_unbound` block; other repeated failures fail.
6. Require `ok = true`. `data = null` blocks: the host installed no memory provider. A missing or non-object `data.counters` fails.
7. Require every counter value to be a non-negative integer (the wire contract is a `u64` map; null, negative and fractional values fail).
8. Require the existing cache counters `shapingEntries`, `shapingBytes`, `layoutEntries`, `layoutBytes`, `imageEntries`, `imageBytes`, plus `skippedParentUpdates`.
9. Normalise `data.windows` and require every window counter to be a non-negative integer. If any window counter exists, require a `targetBytes` render-target estimate; an empty map only warns, so it is never reported as a pass.
10. Require `counters.processResidentBytes` to be a positive integer. A missing value blocks with the counter list and the required per-platform sources; `0`, a negative, a fractional value or a reading below 1,048,576 bytes fails because those cannot be a live process's RSS. A resident total not above the sum of the estimated cache bytes only warns, because estimates can overstate.
11. When `counters.processVirtualBytes` is present, require a positive integer and `processVirtualBytes >= processResidentBytes`.
12. Call `query_ui_memory` a second time after 200 ms and require a positive resident total and the cache counters again, so a one-shot or malformed response cannot pass.
13. Print `RESULT PASS` and exit 0.

### Wire contract asserted

`query_ui_memory` answers the immediate-read envelope. Counters are flat `u64` maps; per-window counters are keyed `<window>.<counter>` with a `{"bytes": N}` value on the desktop host. The harness also accepts a per-window map or an array of `{id, counters}` entries, matching the client's normaliser, so the assertion is about values, not one encoding.

```json
{
  "ok": true,
  "data": {
    "counters": {
      "shapingEntries": 164, "shapingBytes": 72255,
      "layoutEntries": 299, "layoutBytes": 291886,
      "imageEntries": 100, "imageBytes": 554844,
      "skippedParentUpdates": 0,
      "processResidentBytes": 190136320,
      "processVirtualBytes": 501128593408
    },
    "windows": {
      "WindowId(1).targetBytes": {"bytes": 73728000},
      "WindowId(1).msaaLogicalBytes": {"bytes": 58982400},
      "WindowId(1).samples": {"bytes": 4}
    }
  }
}
```

`processResidentBytes` is the process's resident physical memory and `processVirtualBytes` the platform's virtual-size reading: macOS `proc_pidinfo(PROC_PIDTASKINFO)` resident and virtual size, iOS `task_info` resident and virtual size, Linux and Android `VmRSS`/`VmSize` from `/proc/self/status`, Windows `WorkingSetSize` and private commit. The counters come from `crates/hosts/src/process_memory.rs`; a target with no reader omits both counters rather than publishing `0`. The harness never derives either value from the cache counters. The `virtual >= resident` assertion holds for the macOS/Linux/Android address-space reading this harness runs against; a future Windows harness must revisit it because that platform's virtual counter is commit size, not address space.

### Pass conditions

Exit code 0 and every line below printed:

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>)
PASS  cache counters present: shapingBytes=... layoutBytes=... imageBytes=... imageEntries=... skippedParentUpdates=...
PASS  render-target estimates: N window counters, targetBytes total ...
PASS  process resident: N bytes on pid <pid>
PASS  process virtual: N bytes (>= resident N)      # only when processVirtualBytes is reported
PASS  second read still reports a positive total: resident N bytes (first read N bytes)
RESULT PASS  Memory: cache counters, render-target estimates and process RSS
```

A `WARN` replaces the render-target line when `data.windows` is empty, and the `process virtual` line is replaced by a note when the host omits virtual memory. Neither warning weakens the RSS assertion.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; cache counters, render-target estimates and a real process resident reading were asserted |
| 1 | Functional failure: malformed counters, missing cache counter, non-numeric window counter, missing `targetBytes` with window counters, resident not a positive integer, resident below the 1 MiB floor, virtual below resident, second read lost the counters |
| 2 | Blocked: non-macOS host, missing dependency, build failure, missing `query_ui_memory`, incompatible required arguments, controller/gallery did not start, runtime platform not macOS, memory read `unavailable`, `data: null`, `processResidentBytes` absent |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **No provider (`data: null`)**: the desktop host must install the memory provider in `crates/hosts/src/desktop/diagnostics/devtools.rs` backed by `Application::diagnostic_memory_snapshot`. The browser/WASM host answers `null` by design; the harness blocks instead of reading cache counters from somewhere else.
- **Missing `processResidentBytes`**: blocks with the present counter list and the per-platform RSS sources. This happens on a target whose `crates/hosts/src/process_memory.rs` backend has no reader (Wasm and other unsupported targets omit both counters), or when a host does not insert the counters into its snapshot. The harness does not fall back to `imageBytes`/`layoutBytes`/`targetBytes`. On this macOS host the desktop snapshot reports both counters.
- **`0` is never accepted**: a counter that cannot be read must be omitted or reported as `unavailable`; a zero would masquerade as a measurement, so the harness treats it as a functional failure.
- **No heap, GC or leak assertions**: Rust has no tracing GC. The separate allocation harness asserts allocator byte and block counters, but snapshot diffing, object-type attribution, retaining paths, instance tracing and leak tracking are still absent and are not approximated here. Those Dart workflows are not applicable as Dart mechanisms.
- **Plausibility floor**: the 1 MiB minimum only rejects readings that cannot be process memory. It is not a memory budget. Growth behavior belongs to the allocation harness (`allocation.sh`), not this process-memory run.
- **GUI requirement**: the gallery must register a runtime, so the run needs an unlocked GUI session even though it reads no pixels.
- **No product Rust or client changes**: the harness only calls the public controller tool, stores the response it received, and asserts on it.

### Current verification status

`bash -n Tools/DevTools/acceptance/memory.sh` passes. Two live runs used an isolated controller and the real gallery.

The first run used pre-existing debug binaries before the process counters were built:

```sh
Tools/DevTools/acceptance/memory.sh --no-build --keep
```

It wrote an isolated discovery file, refused port 9229, started the controller and gallery, selected the runtime by PID, and read the real snapshot:

```text
PASS  controller ready at http://127.0.0.1:55575 (default port 9229 not used)
PASS  runtime selected: <runtime-id> (CoveUI — Component Showcase, pid <pid>)
PASS  cache counters present: shapingBytes=72255 layoutBytes=291886 imageBytes=554844 imageEntries=100 skippedParentUpdates=0
PASS  render-target estimates: 7 window counters, targetBytes total 73728000
BLOCKED  query_ui_memory omits counters.processResidentBytes ...
```

That run exited 2 exactly as designed; it proves the cache/render-target assertions, the blocked path for a host without process counters, and that neither process remained afterwards (checked with `ps`/`pgrep`). The live response carried the seven cache/composition counters and `WindowId(1).{targetBytes, msaaLogicalBytes, msaaAllocatedBytes, samples, suspended, hasSwapchain, msaaMemoryless}`.

The second run rebuilt the current tree and exercised the full pass:

```sh
cargo build --quiet -p cove-devtools-cli --bin cove-devtools   # exit 0
cargo build --quiet -p cove-examples --bin gallery             # exit 0
Tools/DevTools/acceptance/memory.sh                            # exit 0
```

```text
PASS  controller ready at http://127.0.0.1:56097 (default port 9229 not used)
PASS  runtime selected: 7e8571b3d0f5aef1aae4cc1a968dee12a7ef49e9e5e07c58 (CoveUI — Component Showcase, pid 7142)
PASS  cache counters present: shapingBytes=72255 layoutBytes=291886 imageBytes=554844 imageEntries=100 skippedParentUpdates=0
PASS  render-target estimates: 7 window counters, targetBytes total 73728000
PASS  process resident: 190136320 bytes on pid 7142
PASS  process virtual: 501128593408 bytes (>= resident 190136320)
PASS  second read still reports a positive total: resident 190562304 bytes (first read 190136320 bytes)
RESULT PASS  Memory: cache counters, render-target estimates and process RSS
```

The run printed 7 checks and removed its run directory. The controller reported Screen Recording and Accessibility off; that did not affect the memory read. Both processes were gone afterwards. Two further runs passed after the edits above: the default run reported `processResidentBytes=190791680`, `processVirtualBytes=501130313728` and a second read of `190988288` bytes, and a final `--no-build` run reported `188710912` bytes with a second read of `188956672`. The reading tracks the process rather than a fixed literal, and no gallery process was left behind.

The failure branches were exercised separately against the same wire shape: a synthetic response with `processResidentBytes=9` failed on the 1 MiB floor with exit 1, and `processVirtualBytes` below resident failed the ordering check with exit 1. Those checks validate the assertion block; the live run above is the product proof. No controller or gallery process was left running by any run.

## Allocation: live allocator size and block tracking

Script: `Tools/DevTools/acceptance/allocation.sh`

```sh
Tools/DevTools/acceptance/allocation.sh              # builds, then removes its run directory on success
Tools/DevTools/acceptance/allocation.sh --keep       # keeps logs and the memory JSON files
Tools/DevTools/acceptance/allocation.sh --no-build   # reuses target/debug binaries
```

The run reads the live host's `query_ui_memory` snapshot and requires an allocator snapshot, not a cache or process-memory total. It then asks the runtime to resize its window and requires the cumulative allocation counters to advance, so a static or cached response cannot pass. Allocation tracking here covers **sizes and block counts only**. It does not name Rust object types, retaining paths, garbage collection or leaks, and the harness asserts none of those.

The run is isolated from the user's live controller and makes no product-code changes:

- It builds `target/debug/cove-devtools` and `target/debug/gallery` with cargo unless `--no-build` is passed, then launches those binaries directly. Direct PIDs let cleanup stop exactly its own processes.
- It writes a fresh discovery file under `$TMPDIR/cove-devtools-allocation.*`, exports `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_ENDPOINT` and `COVE_DEVTOOLS_TOKEN` to the controller, the gallery and every CLI call, and picks an ephemeral loopback port. Port 9229 and the user's discovery file (`$HOME/Library/Application Support/Cove/devtools-controller.json`) are never read or written. A guard blocks the run if the isolated session path ever resolves to that file, and the discovery file the controller writes must carry the isolated endpoint before any assertion runs.
- It starts the gallery at `1000x760` through the gallery's own `--width`/`--height` arguments and selects the runtime by the gallery PID it launched. Unlike the memory harness there is no title fallback: a second gallery with the same title is never selected.
- The allocator is platform-neutral, so the script has no macOS preflight; it needs a desktop GUI session where the gallery registers a runtime and accepts `resize_ui`. The verification below ran on macOS.
- On exit, `EXIT`/`INT`/`TERM` traps stop the gallery and controller by PID (TERM, then KILL after five seconds). Both background jobs are disowned so bash does not print job-control notices during cleanup. Logs and the JSON snapshots are kept on failure and removed on success unless `--keep` is passed.
- It needs no TCC permission. `Screen Recording is off` and `Accessibility is off` from the controller do not block this run; allocation counters are host data, not pixels.

### Exact steps

1. Preflight `cargo`, `python3` and `jq`; create the isolated run directory; build both binaries unless `--no-build`.
2. Read `cove-devtools tools` and require `query_ui_memory` and `resize_ui`. Extra required arguments beyond `target` (and `width`/`height` for `resize_ui`) block the run, because the harness sends only those.
3. Start the controller:

   ```sh
   COVE_DEVTOOLS_SESSION_FILE=... COVE_DEVTOOLS_TOKEN=... \
     target/debug/cove-devtools serve --port <ephemeral>
   ```

   Poll `get_ui_session '{}'` until the discovery file exists and the controller answers with a `targets` array. Require the discovery endpoint to equal `COVE_DEVTOOLS_ENDPOINT`.
4. Start the gallery with `COVE_DEVTOOLS=1`, the isolated environment and `--width 1000 --height 760`, then poll `get_ui_session` until the runtime with the gallery PID registers.
5. Call `query_ui_memory` with `{"target":"<runtime>"}`, retrying three times only on transport-level failures. `unavailable`, `target_gone`, `target_required` and `target_unbound` block; other repeated failures fail.
6. Require `ok = true`. `data = null` blocks: the host installed no memory provider. A missing or non-object `data.counters` fails.
7. Require every counter value to be a non-negative integer (the wire contract is a `u64` map; null, negative and fractional values fail).
8. Require `allocCurrentBytes`, `allocTotalBytes` and `allocTotalBlocks`. **A missing allocator counter blocks with exit 2** and the present counter list. The harness never falls back to `processResidentBytes`, `processVirtualBytes`, `imageBytes`, `layoutBytes`, `targetBytes` or any other cache/render total.
9. Require `allocCurrentBytes > 0`, `allocTotalBlocks > 0` and `allocTotalBytes >= allocCurrentBytes`. A present-but-zero counter fails: an instrumented live process allocates, so zero means the snapshot is not tracking and must be omitted rather than published.
10. When `allocLiveBlocks` (or `allocCurrentBlocks`) is present, require it to be a non-negative integer no larger than `allocTotalBlocks`.
11. For every reported peak counter — `allocPeak*Bytes` and `allocPeak*Blocks` — require a non-negative integer at least the matching current reading (`allocPeakBytes >= allocCurrentBytes`, `allocPeakLiveBlocks >= allocLiveBlocks`). A peak above `allocTotalBytes` only warns, because the two counters are read non-transactionally.
12. Call `resize_ui` to `780x600`. `unavailable` blocks (the counters cannot be proven live without a real operation); a target-selection error blocks; any other error fails. A successful answer is accepted in both shapes: the controller's `{ok:true,data}` envelope and the probe's raw `{"requested":true}` payload. A missing `requested: true` echo warns because the counters are the actual evidence.
13. Poll `query_ui_memory` every 250 ms for up to `ACCEPTANCE_ACTIVITY_TIMEOUT` seconds (default 20) until **both** `allocTotalBytes` and `allocTotalBlocks` are strictly greater than the baseline. A timeout fails with exit 1: counters that never move are static snapshots, not allocator readings.
14. Require the post-resize read to keep `allocCurrentBytes > 0`, `allocTotalBytes >= allocCurrentBytes` and the allocation counters.
15. Call `resize_ui` again to `1240x940` and require a final read whose `allocTotalBytes` and `allocTotalBlocks` are at least the post-resize values, so cumulative totals are monotonic. Current bytes may move either way.
16. Print `RESULT PASS` and exit 0.

### Wire contract asserted

`query_ui_memory` answers the immediate-read envelope. Allocation counters are flat `u64` entries in the same `counters` map as the cache and process counters; counters the allocator does not report are omitted, never written as `0`.

```json
{
  "ok": true,
  "data": {
    "counters": {
      "allocCurrentBytes": 42138112,
      "allocPeakBytes": 58130432,
      "allocTotalBytes": 981234567,
      "allocDeallocatedBytes": 939096455,
      "allocLiveBlocks": 187245,
      "allocPeakLiveBlocks": 231040,
      "allocTotalBlocks": 4123987,
      "allocDeallocatedBlocks": 3936742,
      "allocSizeClass64Bytes": 1210368,
      "allocSizeClass1MiBBytes": 18874368,
      "processResidentBytes": 190136320,
      "processVirtualBytes": 501128593408
    },
    "windows": {"WindowId(1).targetBytes": {"bytes": 73728000}}
  }
}
```

Required counters come from `crates/alloc/src/lib.rs` (`CURRENT_BYTES_COUNTER`, `TOTAL_BYTES_COUNTER`, `TOTAL_BLOCKS_COUNTER`); the peak and live-block names are `allocPeakBytes`, `allocPeakLiveBlocks` and `allocLiveBlocks`, and the optional size-class histogram is `allocSizeClass*Bytes`. The host publishes them through its diagnostic memory snapshot only when the tracking allocator is installed; the process counters and cache counters in the same map stay independent values.

The harness treats `allocTotalBytes`/`allocTotalBlocks` as cumulative and `allocCurrentBytes`/`allocLiveBlocks` as gauges. Reallocations count a release of the old size and an allocation of the new size, so the totals advance even when live bytes stay flat; that is why the growth assertion is on the totals, not on current bytes.

### Pass conditions

Exit code 0 and every line below printed:

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>)
PASS  allocator snapshot: current N bytes, total N bytes in N blocks
PASS  peak counters reported: allocPeakBytes allocPeakLiveBlocks (>= current)   # only when reported
PASS  allocation counters advanced after resize_ui: bytes A -> B (+D), blocks C -> E (+F)
PASS  counters stayed cumulative after the second resize: total N bytes, N blocks
RESULT PASS  Allocation: live allocator size and block tracking
```

A `WARN` replaces the peak line when no peak counter is reported, and the live-block count is only asserted when the host reports it. Neither warning weakens the required snapshot or the growth assertion.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; a consistent allocator snapshot was read and its cumulative totals advanced after a real resize |
| 1 | Functional failure: non-u64 counter, present-but-zero allocation counter, total below current, live blocks above total blocks, peak below current, `resize_ui` error, counters did not advance within the timeout, counters lost or decreased on a later read |
| 2 | Blocked: missing dependency, build failure, missing `query_ui_memory`/`resize_ui`, incompatible required arguments, controller/gallery did not start, runtime did not register under the gallery PID, memory read `unavailable`, `data: null`, **missing allocation counters**, `resize_ui` unavailable |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **Allocator not installed**: blocks with exit 2, names the missing counters and lists the counters that were present. The fix is the backend allocator (`crates/alloc`) installed as the gallery process's global allocator and published through `DiagnosticMemorySnapshot`; the harness never substitutes RSS or cache bytes. A runtime that omits the counters is indistinguishable from an uninstrumented one and stays blocked.
- **Counters present but static**: a real resize must make the cumulative totals move. If they do not within 20 seconds the run fails, because publishing constant values is worse than omitting them.
- **Not object types, paths, GC or leaks**: bytes and blocks only, exactly the container's contract. There is no type attribution, retaining path, GC event or leak assertion, and growth is not by itself a leak — an unreleased block stays counted whether or not it is reachable.
- **Non-transactional snapshot**: the counters are separate atomic loads. Current bytes, peak and totals can mix instants during concurrent allocation; the harness asserts only relations that hold under this sampling (`total >= current`, peaks `>= current`, cumulative totals monotonic). It does not assert that the resize delta equals a particular byte count.
- **GUI and resize requirement**: the gallery must register and accept `resize_ui`. A headless host cannot prove the counters are live and blocks at the resize step rather than passing with a one-shot snapshot.
- **No product Rust or client changes**: the harness only calls public controller tools, stores the responses it received, and asserts on them.

### Current verification status

`bash -n Tools/DevTools/acceptance/allocation.sh` passes. The blocked path was exercised end-to-end against the current `target/debug` binaries before the allocator was wired into the host:

```sh
Tools/DevTools/acceptance/allocation.sh --no-build --keep
```

```text
PASS  controller ready at http://127.0.0.1:57351 (default port 9229 not used)
PASS  runtime selected: c7bc... (CoveUI — Component Showcase, pid 11069)
BLOCKED  query_ui_memory has no allocator snapshot for runtime ... (missing: allocCurrentBytes allocTotalBytes allocTotalBlocks). Present counters: imageBytes, imageEntries, layoutBytes, layoutEntries, processResidentBytes, processVirtualBytes, shapingBytes, shapingEntries, skippedParentUpdates.
```

That run exited 2 as designed, proving the isolation, runtime selection by PID, the missing-allocator block and the absence of any cache/RSS fallback. Both processes were gone afterwards (`ps -p 11069` empty; no controller on the isolated port).

After the allocator was wired into the gallery (`crates/alloc` as the global allocator, counters inserted by `crates/hosts/src/allocation.rs`), both builds and three live runs passed:

```sh
cargo build -p cove-devtools-cli --bin cove-devtools     # Finished dev profile, exit 0
cargo build -p cove-examples --bin gallery               # Finished dev profile, exit 0
Tools/DevTools/acceptance/allocation.sh --no-build --keep   # exit 0, 6 checks
Tools/DevTools/acceptance/allocation.sh                     # rebuilds and passes, exit 0, run directory removed
Tools/DevTools/acceptance/allocation.sh --no-build          # final run against the final script, exit 0
```

First pass (`--no-build --keep`):

```text
PASS  controller ready at http://127.0.0.1:58160 (default port 9229 not used)
PASS  runtime selected: 01bbc9c7... (CoveUI — Component Showcase, pid 12074)
PASS  allocator snapshot: current 34640587 bytes, total 172993209 bytes in 719157 blocks
live blocks: 53302 (of 719157 cumulative)
PASS  peak counters reported: allocPeakBytes allocPeakLiveBlocks (>= current)
PASS  allocation counters advanced after resize_ui: bytes 172993209 -> 214117612 (+41124403), blocks 719157 -> 790550 (+71393)
PASS  counters stayed cumulative after the second resize: total 325606492 bytes, 1612833 blocks
RESULT PASS  Allocation: live allocator size and block tracking
```

The second run, with the default build path, reported `current 34642553` bytes, `total 174691851` bytes in `720063` blocks, grew to `220008497` bytes / `794110` blocks after the first resize, and ended at `331497377` bytes / `1616393` blocks. The third (final) `--no-build` run reported a baseline of `29140806` current / `127459277` total bytes in `632483` blocks, grew to `220693667` bytes / `1114397` blocks after the first resize, and ended at `282955178` bytes / `1545306` blocks. The resize deltas are tens of megabytes and hundreds of thousands of blocks, so the counters are clearly live rather than copies of a cache or RSS total; the responses carried `processResidentBytes` and `processVirtualBytes` alongside the allocator counters, and the assertions never read those process counters. After the runs no gallery or controller process remained (`ps -p <pid>` and `pgrep -f "serve --port <port>"` were empty), and the default and final runs removed their temporary directories.

The static-counter failure branch was not observed on the live product, because the allocator counters are genuinely cumulative; it is enforced by the 20-second growth wait and exits 1 without ever accepting a one-shot snapshot. The missing-allocator block was observed end-to-end as shown above.

## Allocation sites: sampled caller stacks and fresh sampling windows

Script: `Tools/DevTools/acceptance/allocation-sites.sh`

```sh
Tools/DevTools/acceptance/allocation-sites.sh              # builds, then removes its run directory on success
Tools/DevTools/acceptance/allocation-sites.sh --keep       # keeps logs and the allocation-site JSON files
Tools/DevTools/acceptance/allocation-sites.sh --no-build   # reuses target/debug binaries
```

The run starts an isolated controller and the real gallery, requires the runtime's `allocation_sites` capability, samples at 1-in-64, drives a real `resize_ui`, and stops the sampler. It asserts the live `UiAllocationSites` contract: a `ready` stopped report, positive `sampledBytes`/`sampledBlocks`, consistent `samplesAttempted`/`samplesRetained`/`samplesDropped`/`captureFailures`, a non-empty bounded site list, real frames resolved to a function with `file:line`, no allocator-internal frame before a caller, bounded sites/frames, truncation flags that match the returned cut, and query limits that are respected.

The entire start/stop/query cycle runs twice. The second start must report a zeroed window, an immediate query must not replay the first window's attempts, and the post-stop query must serve the second window's counters. Those two independent windows are the real sampled data a baseline/diff view would compare. **This harness does not compute or assert a site diff**, and it never invents added or removed sites. It also does not claim type-aware heap attribution, retaining paths, instance tracking or leaks: every number is a cumulative sampled allocation at the configured rate, not the live heap or an object type.

### What is asserted, and what is not

- **Real samples only**: every reported address is a positive sampled return address; every site's bytes and blocks are positive; the frames are resolved by the runtime's symbolizer.
- **Caller attribution**: a site may not mix allocator-internal frames with caller frames. A stack that resolves entirely to allocator internals is a real sample but is reported with a `WARN` because it names no caller; it is never treated as caller evidence. "Allocator-internal" is the probe's own definition: `cove_alloc`, the tracking allocator, the Rust allocation shims (`__rust_alloc`, `__rg_alloc`, …) and the unwinder. Frames such as `alloc::alloc::Global::alloc_impl` or `alloc::raw_vec` are the standard-library helpers that called the tracking allocator; they remain real sampled call sites and are valid `file:line` evidence.
- **Consistent counters**: `attempted = retained + dropped + captureFailures`; `sampledBlocks = samplesRetained` (each retained sample contributes one block); `siteCount <= samplesRetained`; returned site bytes/blocks never exceed `sampledBytes`/`sampledBlocks`, and equal them when the answer is complete.
- **Bounds and truncation**: `sitesTruncated` is a boolean and equals `siteCount > returnedSites`; the returned list is exactly `min(limit, siteCount)`; sites never exceed the published `MAX_ALLOCATION_SITES`; frames never exceed the sampler's capture depth; retained samples never exceed the ring capacity.
- **Query limits**: after the stop, `query_ui_allocation_sites` is called with `limit: 8` and `limit: 1`. Both answers must carry the stopped report's counters, return the first sites of that report, and respect the cut (`8 of N`, `1 of N`, `sitesTruncated` matching).
- **States**: `idle` before any sampling (empty counters and sites), `running` while sampling, `ready` after stop, a truncated payload with `siteCount > returned`, an explicit `no_allocation_profile` error for a second stop, and `unavailable`/missing-capability paths that block.
- **Not asserted**: a baseline/diff table, added/removed sites, object types, live-heap totals, retaining paths, GC, leaks. The product has no type-aware heap, so those Flutter workflows have no analogue here; the two sampling windows are the applicable diff basis.

The run is isolated from the user's live controller and makes no product-code changes:

- It builds `target/debug/cove-devtools` and `target/debug/gallery` with cargo unless `--no-build` is passed, then launches those binaries directly. Direct PIDs let cleanup stop exactly its own processes; the user's controllers on other ports are never enumerated or signalled.
- It writes a fresh discovery file under `$TMPDIR/cove-devtools-allocation-sites.*`, exports `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_ENDPOINT` and `COVE_DEVTOOLS_TOKEN` to the controller, the gallery and every CLI call, and picks an ephemeral loopback port. Port 9229 and the user's discovery file (`$HOME/Library/Application Support/Cove/devtools-controller.json`) are never read or written. A guard blocks the run if the isolated session path ever resolves to that file, and the discovery file the controller writes must carry the isolated endpoint before any assertion runs.
- It starts the gallery at `1000x760` through the gallery's own `--width`/`--height` arguments and selects the runtime by the gallery PID it launched. There is no title fallback, so another gallery with the same title is never selected.
- On exit, `EXIT`/`INT`/`TERM` traps stop the gallery and controller by PID (TERM, then KILL after five seconds). Both background jobs are disowned. Logs and JSON responses are kept on failure and removed on success unless `--keep` is passed.
- It needs no TCC permission. `Screen Recording is off` and `Accessibility is off` from the controller do not block this run; sampled stacks are process data, not pixels.

### Exact steps

1. Preflight `cargo`, `python3` and `jq`; create the isolated run directory; build both binaries unless `--no-build`.
2. Read `cove-devtools tools` and require `start_ui_allocation_profile` (with required `rate`), `stop_ui_allocation_profile` and `query_ui_allocation_sites` (with a `limit` property), each accepting `target`. Read the published query maximum and require it to be an integer; the harness limit must fit it.
3. Start the controller:

   ```sh
   COVE_DEVTOOLS_SESSION_FILE=... COVE_DEVTOOLS_TOKEN=... \
     target/debug/cove-devtools serve --port <ephemeral>
   ```

   Poll `get_ui_session '{}'` until the discovery file exists and the controller answers with a `targets` array. Require the discovery endpoint to equal `COVE_DEVTOOLS_ENDPOINT`.
4. Start the gallery with `COVE_DEVTOOLS=1`, the isolated environment and `--width 1000 --height 760`, then poll `get_ui_session` until the runtime with the gallery PID registers. Require `allocation_sites` in its `capabilities`; a missing capability blocks with exit 2.
5. `query_ui_allocation_sites` with `limit: 8` before any start. Require `status: "idle"`, `rate: 0`, all counters zero, `siteCount: 0`, `sitesTruncated: false` and an empty `sites` array. This is the empty state; no site may be invented before sampling.
6. Window 1: `start_ui_allocation_profile` with `{"target":"<runtime>","rate":64}`. Require `status: "running"`, `rate: 64`, every counter zero, `siteCount: 0`, `sitesTruncated: false` and no sites. The zeroed start reply is the fresh-window boundary.
7. Query the running session with `limit: 4`, then call `resize_ui` to `780x600`. Poll `query_ui_allocation_sites` with `limit: 8` every 250 ms for up to `ACCEPTANCE_ACTIVITY_TIMEOUT` seconds until `samplesRetained`, `sampledBytes` and `siteCount` are positive. A `ready` status during this poll fails: sampling must still be running.
8. `stop_ui_allocation_profile`. Require `status: "ready"`, the rate, counters and site bounds above, `siteCount > 0`, and real evidence. If `symbolizationReason` is non-null the run blocks with exit 2 instead of passing unresolved addresses.
9. Query the stopped report with `limit: 8` and again with `limit: 1`. Require the same counters and truncated-report shape as the stop, the first `N` sites from the stopped report, and `sitesTruncated` matching the cut.
10. Window 2: repeat steps 6–9 with `resize_ui` to `1000x760`. Immediately after the second start, require its counters to be zero and its post-start query's `samplesAttempted` to be below window 1's completed attempts. After the second stop, the query must serve window 2's counters, not window 1's.
11. Call `stop_ui_allocation_profile` again with no active session. Require a non-zero CLI exit and error code `no_allocation_profile`.
12. Print `RESULT PASS` and exit 0.

### Wire contract asserted

The controller returns the probe payload directly for these tools (no `ok`/`data` envelope). Counters are cumulative for the sampling window. `rate` is the 1-in-N divisor. Frame `address` is the real sampled return address; `function`, `filePath` and `line` may be absent only when symbolization is unavailable, which blocks this harness.

```json
{
  "status": "ready",
  "rate": 64,
  "sampledBytes": 1011158,
  "sampledBlocks": 1024,
  "samplesAttempted": 6066,
  "samplesRetained": 1024,
  "samplesDropped": 5042,
  "captureFailures": 0,
  "sitesTruncated": true,
  "siteCount": 141,
  "symbolizationReason": null,
  "sites": [
    {
      "bytes": 846424,
      "blocks": 3,
      "framesTruncated": false,
      "frames": [
        {
          "address": 4295683520,
          "function": "<alloc::alloc::Global>::alloc_impl",
          "filePath": "/Users/dev/.../library/alloc/src/alloc.rs",
          "line": 424
        }
      ]
    }
  ]
}
```

`start_ui_allocation_profile` returns the same shape with `status: "running"` and every window counter zero. `query_ui_allocation_sites` returns the live `running` window while sampling and the last `ready` report after a stop; `limit` is clamped to the published maximum. Empty counters are never written as a substitute for a missing reading.

### Pass conditions

Exit code 0 and every line below printed (values vary with the run):

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>, platform macos)
PASS  runtime advertises allocation_sites
PASS  empty state: status idle, rate 0, no counters and no sites before the first start
PASS  window 1 start: status running at 1-in-64, every window counter zeroed
PASS  window 1 running: N sampled bytes in N blocks, A attempted / N retained / D dropped, S sites
PASS  window 1 stop: status ready, 64 of S sites (truncated=true), N bytes / N blocks, attempted=A retained=N dropped=D failures=0
PASS  window 1 frames: <function> at <file>:<line>
PASS  window 1 query: limit 8 returned 8 of S sites (truncated=true) with identical counters
PASS  window 1 query: limit 1 returned 1 site (truncated=true)
PASS  window 2 start: status running at 1-in-64, every window counter zeroed
PASS  window 2 fresh window: post-start query reports A attempts, below window 1's A
PASS  window 2 running: ...
PASS  window 2 stop: ...
PASS  window 2 frames: ...
PASS  window 2 query: ...
PASS  window 2 query: limit 1 ...
PASS  window 2 is independent: start reported zeroed counters, the preview stayed below window 1's attempts, and the post-stop query matches only window 2
PASS  error state: a stop without a session reports no_allocation_profile
RESULT PASS  Allocation sites: sampled caller stacks and fresh sampling windows
```

The number of `PASS` lines is 19. A `WARN` for a whole-stack allocator-internal site does not weaken the pass; the other assertions still require caller evidence somewhere in the report. `sitesTruncated` is `true` in the sample because each window produced more than 64 distinct sites; the harness asserts the flag's consistency, not that truncation must happen, so a quiet window with 64 or fewer sites passes with `false`.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; two real sampling windows produced bounded, symbolized, consistent sites and the stated limits/states held |
| 1 | Functional failure: wrong status/rate, zero or inconsistent counters, empty sites after a resize, a site/frame shape violation, mixed allocator frames, no resolved `file:line`, truncation or limit mismatch, no samples within the timeout, missing `no_allocation_profile` error, or a query that does not serve the stopped window |
| 2 | Blocked: missing dependency, build failure, missing allocation-site tools or arguments, missing `allocation_sites` capability, unsupported sampler, non-null `symbolizationReason`, controller/gallery did not start, runtime did not register under the gallery PID, or sampling became `unavailable` |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **Sampling unsupported**: a runtime without `allocation_sites` is normal on a browser build or a platform where `cove_alloc` cannot capture stacks. The harness blocks with exit 2 and the runtime's capability list; it never synthesizes sites. Check `cargo test -p cove-devtools --test allocation_sites` and use the native desktop gallery.
- **No symbolizer**: Linux can capture stacks but `cove-profiling` has no Linux symbolizer yet, so `symbolizationReason` is non-null and the harness blocks rather than accepting unresolved addresses. A full pass needs function and `file:line` evidence.
- **No real activity**: the resize must allocate enough for 1-in-64 sampling to retain samples. A host that cannot resize the gallery window blocks at the resize step; a resize that produces no samples fails after the activity timeout.
- **Sampler ring**: retained samples are bounded by the sampler's 1024-slot ring, and a window can drop older samples. `samplesDropped` is therefore expected and asserted to be consistent with `samplesAttempted - samplesRetained - captureFailures`; it is not data loss to hide.
- **Cumulative, not live**: bytes and blocks sum allocations that passed through the tracking allocator during the window. A freed block stays counted. There is no object type, retaining path, GC event, instance or leak assertion.
- **No diff**: the harness proves two independent windows and consistent stopped reports, but it does not render or compare a baseline/diff table. A diff view must use these same windows and label them as cumulative sampled allocations; added/removed sites cannot be inferred from object lifetimes that Rust does not expose here.
- **Whole-stack internal samples**: the probe drops allocator frames before the first caller. If every captured frame is allocator internal, the real sample is kept and the harness warns instead of fabricating a caller. Mixed internal/caller stacks fail because the filter must have removed them.
- **GUI requirement**: the gallery must register and accept `resize_ui`. The run needs a desktop session but no Screen Recording or Accessibility permission.
- **No product changes**: the harness only calls public controller tools, stores the responses it received, and asserts on them.

### Current verification status

```sh
bash -n Tools/DevTools/acceptance/allocation-sites.sh
# clean

node --check crates/devtools/client/inspector.js
node --check crates/devtools/client/tests/allocation-sites-harness.mjs
# node --check: OK

node crates/devtools/client/tests/allocation-sites-harness.mjs
# ... 10 scenarios ...
# RESULT PASS

cargo check -q -p cove-devtools-cli
# cargo check cove-devtools-cli: OK

cargo test -q -p cove-devtools --test allocation_sites
# allocation-site probe checks passed
```

Three live runs on this macOS arm64 host used an isolated controller and gallery:
```sh
Tools/DevTools/acceptance/allocation-sites.sh --no-build --keep   # exit 0, 19 checks
Tools/DevTools/acceptance/allocation-sites.sh --keep             # builds, exit 0, 19 checks
Tools/DevTools/acceptance/allocation-sites.sh --no-build         # exit 0, 19 checks, run directory removed
```

The first run printed:

```text
PASS  controller ready at http://127.0.0.1:53479 (default port 9229 not used)
PASS  runtime selected: b9be0a8c... (CoveUI — Component Showcase, pid 81650, platform macos)
PASS  runtime advertises allocation_sites
PASS  empty state: status idle, rate 0, no counters and no sites before the first start
PASS  window 1 start: status running at 1-in-64, every window counter zeroed
PASS  window 1 running: 109508 sampled bytes in 1024 blocks, 6026 attempted / 1024 retained / 5002 dropped, 136 sites
PASS  window 1 stop: status ready, 64 of 141 sites (truncated=true), 1011158 bytes / 1024 blocks, attempted=6066 retained=1024 dropped=5042 failures=0
PASS  window 1 frames: <alloc::alloc::Global>::alloc_impl at .../library/alloc/src/alloc.rs:424
PASS  window 1 query: limit 8 returned 8 of 141 sites (truncated=true) with identical counters
PASS  window 1 query: limit 1 returned 1 site (truncated=true)
PASS  window 2 start: status running at 1-in-64, every window counter zeroed
PASS  window 2 fresh window: post-start query reports 1 attempts, below window 1's 6066
PASS  window 2 running: 112881 sampled bytes in 1024 blocks, 6066 attempted / 1024 retained / 5042 dropped, 128 sites
PASS  window 2 stop: status ready, 64 of 131 sites (truncated=true), 112701 bytes / 1024 blocks, attempted=6075 retained=1024 dropped=5051 failures=0
PASS  window 2 frames: <alloc::raw_vec::RawVecInner>::with_capacity_in at .../library/alloc/src/raw_vec/mod.rs:435
PASS  window 2 query: limit 8 returned 8 of 131 sites (truncated=true) with identical counters
PASS  window 2 query: limit 1 returned 1 site (truncated=true)
PASS  window 2 is independent: start reported zeroed counters, the preview stayed below window 1's attempts, and the post-stop query matches only window 2
PASS  error state: a stop without a session reports no_allocation_profile
RESULT PASS  Allocation sites: sampled caller stacks and fresh sampling windows
```

Both windows reported 131–141 distinct sites and 64 returned sites in this run, so the bounded answer and its `sitesTruncated` flag were exercised with real data (`sitesTruncated=true`, `min(64, siteCount) = 64`). The second window's counters differ from the first independently, and the second start's post-query attempts were 1 rather than a continuation of 6066. No mixed allocator/caller stacks and no all-internal sites occurred in this run; the warnings are therefore not part of the recorded output.

The default-build run also passed 19 checks: window 1 reported 95,310 bytes / 1,024 blocks, 6,660 attempted / 1,024 retained / 5,636 dropped and 131 sites; window 2 reported 117,350 bytes / 1,024 blocks, 6,090 attempted / 1,024 retained / 5,066 dropped and 128 sites; the post-start preview was 1 attempt against window 1's 6,660. The final `--no-build` run (without `--keep`) passed the same contract with 134/132 distinct sites, 1 attempt against 6,043, and removed its run directory. Across the runs the windows reported 128–141 distinct sites, every window returned 64 of them with `sitesTruncated=true`, and both `limit: 8` and `limit: 1` returned the expected prefixes.

After all runs no gallery or controller process remained (`ps -p` empty for each launched PID and no controller on the isolated ports), and the successful no-keep run removed its temporary directory. The exit-2 branches for a missing capability, an unsupported sampler and a missing symbolizer are guarded by explicit checks but were not triggered on this macOS host, where the native sampler and DWARF symbolizer are available. The functional failure branches for zero/static counters and wrong limits are enforced by the assertions; they were not observed with real data.

## App Size: native executable sections, symbols and crate groups

Script: `Tools/DevTools/acceptance/app-size.sh`

```sh
Tools/DevTools/acceptance/app-size.sh              # builds, then removes its run directory on success
Tools/DevTools/acceptance/app-size.sh --keep       # keeps logs and the App Size JSON files
Tools/DevTools/acceptance/app-size.sh --no-build   # reuses target/debug binaries
```

The run starts an isolated controller and the real gallery, waits for the gallery's runtime, and calls `get_ui_app_size` on it. It asserts the answer against the executable that is actually running: the file on disk, its parsed object file, and the rows the controller returned. It then calls the same tool on a display or other platform target and requires the explicit `unavailable` fault, so a target without a runtime executable can never receive invented sizes.

This is the selected runtime's native executable. It is **not** Flutter or Dart AOT attribution: there is no `--analyze-size` file, no Dart snapshot, no package or asset table. The harness asserts only native sections, symbols and demangled Rust crate groups.

The run is isolated from the user's live controller and makes no product-code changes:

- It builds `target/debug/cove-devtools` and `target/debug/gallery` with cargo unless `--no-build` is passed, then launches those binaries directly. Direct PIDs let cleanup stop exactly its own processes.
- It writes a fresh discovery file under `$TMPDIR/cove-devtools-app-size.*`, exports `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_ENDPOINT` and `COVE_DEVTOOLS_TOKEN` to the controller, the gallery and every CLI call, and picks an ephemeral loopback port. Port 9229 and the user's discovery file (`$HOME/Library/Application Support/Cove/devtools-controller.json`) are never read or written. A guard blocks the run if the isolated session path ever resolves to that file, and the discovery file the controller writes must carry the isolated endpoint before any assertion runs.
- It selects the runtime by the gallery PID it launched. There is no title fallback, so another gallery window with the same title is never selected.
- On exit, `EXIT`/`INT`/`TERM` traps stop the gallery and controller by PID (TERM, then KILL after five seconds). Both background jobs are disowned. Logs and JSON responses are kept on failure and removed on success unless `--keep` is passed.
- It needs no TCC permission. `Screen Recording is off` and `Accessibility is off` from the controller do not block this run; App Size reads the executable file, not pixels or native controls.

### Exact steps

1. Preflight macOS, `cargo`, `python3` and `jq`; create the isolated run directory; build both binaries unless `--no-build`.
2. Read `cove-devtools tools` and require `get_ui_app_size` with the `sections`, `symbols` and `groups` limit properties. Extra required arguments block the run, because the harness sends only `target` plus those three optional limits.
3. Start the controller:

   ```sh
   COVE_DEVTOOLS_SESSION_FILE=... COVE_DEVTOOLS_TOKEN=... \
     target/debug/cove-devtools serve --port <ephemeral>
   ```

   Poll `get_ui_session '{}'` until the discovery file exists and the controller answers with a `targets` array. Require the discovery endpoint to equal `COVE_DEVTOOLS_ENDPOINT`.
4. Start the gallery with `COVE_DEVTOOLS=1` and the isolated environment, then poll `get_ui_session` until a runtime with the gallery PID registers. Require `platform: "macos"`.
5. Call `get_ui_app_size` with `{"target":"<runtime>","sections":3,"symbols":5,"groups":3}`, retrying three times only on transport-level failures. `unavailable`, `target_gone`, `target_required` and `target_unbound` block; other repeated failures fail.
6. Require `available: true`, `measurement: "native_executable"`, non-empty `label` and `attribution`, and `target`/`pid` equal to the selected runtime and its gallery PID.
7. Require an absolute `executable` path that exists, canonicalizes to the same file as the launched gallery binary, and whose on-disk size equals `file_bytes` byte for byte. Require `file_bytes > 0`.
8. Require `format: "MachO"`, `kind: "Executable"`, the architecture of the host (`Aarch64` on arm64, `X86_64` on x86_64), and a non-empty `symbol_size_basis`.
9. Require non-empty `sections`, `symbols` and `groups`; every row a well-formed object with non-negative integer `size`/`bytes` and `address`; the section rows include the `__TEXT` segment and at least one section with both a positive size and a positive address; all three lists are ordered by descending size; every group name is demangled (no `_ZN`/`_R`/`<` prefix); group byte and symbol sums do not exceed the symbol totals.
10. Require `sections_returned`/`symbols_returned`/`groups_returned` to equal the returned array lengths and be at most the requested limits, the reported `*_count` to be at least the returned rows, and each `*_truncated` flag to equal `count > returned`. At least one list must be truncated at `3/5/3`, which proves the bound is enforced rather than ignored.
11. Call `get_ui_app_size` again with the controller maxima `{"sections":100,"symbols":200,"groups":100}`. Require the same executable, PID, target and totals; the same truncation consistency at the new limits; returned sizes to add up to `section_bytes`/`symbol_bytes` when the list is complete and not to exceed them when it is truncated; the same top section, symbol and group rows; at least one Rust-mangled symbol (`__ZN…`/`__R…` on Mach-O) demangled to a `::` path; and at least one group named after a real gallery/Cove crate.
12. Re-read `get_ui_session`, pick a `display` target (falling back to a window, webdriver, portal or process target), and call `get_ui_app_size` on it. Require a non-zero CLI exit, error code `unavailable`, the message naming the missing runtime executable, and no `file_bytes`, `sections`, `symbols`, `groups` or `available` field in the answer.
13. Print `RESULT PASS` and exit 0.

### Wire contract asserted

`get_ui_app_size` returns the analysis directly (no `ok`/`data` envelope). The rows are bounded by the requested limits; `*_count` is the total before truncation. On Mach-O, `raw` carries the leading object-file underscore (`__R…`, `__ZN…`), while `name` is the demangled Rust name.

```json
{
  "available": true,
  "measurement": "native_executable",
  "label": "Native executable of the selected runtime",
  "attribution": "This measurement covers the selected runtime's own executable file. ...",
  "target": "<runtime>",
  "pid": 4242,
  "executable": "/path/target/debug/gallery",
  "format": "MachO",
  "kind": "Executable",
  "architecture": "Aarch64",
  "file_bytes": 114517928,
  "section_bytes": 45271911,
  "section_count": 38,
  "sections_returned": 3,
  "sections_truncated": true,
  "sections": [
    {"name": "__text", "segment": "__TEXT", "kind": "Text", "size": 31747324, "address": 4294974080}
  ],
  "symbol_bytes": 31786276,
  "symbol_size_basis": "Symbol sizes come from the object symbol table. ...",
  "symbol_count": 206056,
  "symbols_returned": 5,
  "symbols_truncated": true,
  "symbols": [
    {"name": "cove_core::…", "raw": "__R…", "kind": "Text", "size": 28688, "address": 4295000000, "inferred_size": true}
  ],
  "group_count": 19764,
  "groups_returned": 3,
  "groups_truncated": true,
  "groups": [
    {"name": "cove_core", "bytes": 2006408, "symbols": 7994}
  ]
}
```

The `sections`, `symbols` and `groups` arrays above are abbreviated to one row each; the controller returns `sections_returned`/`symbols_returned`/`groups_returned` rows.

A non-runtime target answers a JSON-RPC fault, which the CLI prints as the error object and exits non-zero:

```json
{
  "code": -32000,
  "data": {"code": "unavailable"},
  "message": "App Size measures the native executable of a running Cove runtime. The selected target is a platform window, display or browser session, which has no runtime executable."
}
```

The truncation flag equals `*_count > *_returned` for every list. It is not a guess: with limits `L`, `returned = min(count, L)`, so the equivalence holds in both truncated and complete cases. A truncated list may only add up to a value at most the reported total; a complete list must add up exactly.

### Pass conditions

Exit code 0 and every line below printed (14 checks in the verified run):

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>)
PASS  native executable measurement: MachO Aarch64 Executable
PASS  executable: <path> (<N> bytes on disk, matches the launched gallery)
PASS  object file: MachO Executable Aarch64 with a documented symbol size basis
PASS  sections: 3 returned, largest __text 31747324 bytes at 0x100001a80
PASS  symbols: 5 returned of 206056, 31786276 bytes in the symbol table, largest <name> ... bytes
PASS  groups: 3 returned of 19764, largest core 6211224 bytes across 57097 symbols
PASS  bounded rows at 3/5/3: sections 3/38 truncated=true; symbols 5/206056 truncated=true; groups 3/19764 truncated=true
PASS  totals are stable across limits: 114517928 file bytes, 38 sections, 206056 symbols, 19764 groups
PASS  top rows are deterministic across limits: __text / <symbol> / core
PASS  demangled names: __R… -> <demangled path>
PASS  crate groups name real Rust crates: cove_ui_compose
PASS  non-runtime display target display:1 -> unavailable: ...
RESULT PASS  App Size: native executable sections, symbols and crate groups
```

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; the executable on disk matched the response and the bounded rows were consistent at two limits |
| 1 | Functional failure: wrong executable/PID/target, `file_bytes` mismatch, malformed row, missing `__TEXT` section, non-demangled group, inconsistent truncation flag/count, unstable totals or top rows, display target returning data or size fields |
| 2 | Blocked: non-macOS host, missing dependency, build failure, missing `get_ui_app_size` or its limit arguments, controller/gallery did not start, runtime did not register under the gallery PID, executable analysis `unavailable`, no display/platform target for the unavailable assertion |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **No local executable**: a runtime on another host, a mobile/WebDriver runtime, a display, a window or a browser session answers `unavailable` with the reason. The harness blocks on the runtime read and asserts the explicit fault for the platform target. It never falls back to RSS, allocator counters or a guessed binary.
- **Stripped or unreadable binary**: a runtime whose executable has no symbol table returns fewer or no symbol/group rows. The harness fails the non-empty rows and demangling assertions instead of fabricating attribution; a stripped release build needs its own documented expectation.
- **Not Dart/Flutter attribution**: there is no treemap, dominator tree, call graph, AOT snapshot, package or asset data here. Only the native executable's sections, symbols and Rust crate groups are asserted, and no diff is asserted at the wire level.
- **Host architecture**: the architecture assertion assumes a native build of the gallery. Running the controller under Rosetta against an arm64 binary would report a mismatch rather than silently pass.
- **GUI requirement**: the gallery must register a runtime and a display or window target must be listed for the unavailable assertion. A headless session blocks at that step rather than skipping it.
- **No product Rust or client changes**: the harness only calls public controller tools, stores the responses it received, and asserts on them.

### Current verification status

`bash -n Tools/DevTools/acceptance/app-size.sh` passes. The script was run end-to-end four times against isolated controllers and the real gallery on this macOS host: once with the default build path, three times with `--no-build` (the last two used the final script). All four printed 14 checks and `RESULT PASS` with exit 0.

```sh
cargo build -p cove-devtools-cli --bin cove-devtools   # exit 0
cargo build -p cove-examples --bin gallery             # exit 0
Tools/DevTools/acceptance/app-size.sh                  # exit 0, run directory removed
Tools/DevTools/acceptance/app-size.sh --no-build --keep # exit 0, artifacts kept
```

Final `--no-build --keep` run (isolated controller on `127.0.0.1:58108`, gallery pid 45445):

```text
PASS  executable: /Users/cyke666/dev/Cove/target/debug/gallery (114517928 bytes on disk, matches the launched gallery)
PASS  sections: 3 returned, largest __text 31747324 bytes at 0x100001a80
PASS  symbols: 5 returned of 206056, 31786276 bytes in the symbol table, largest taffy::compute::grid::track_sizing::resolve_intrinsic_track_sizes::... 28688 bytes
PASS  groups: 3 returned of 19764, largest core 6211224 bytes across 57097 symbols
PASS  bounded rows at 3/5/3: sections 3/38 truncated=true; symbols 5/206056 truncated=true; groups 3/19764 truncated=true
PASS  totals are stable across limits: 114517928 file bytes, 38 sections, 206056 symbols, 19764 groups
PASS  demangled names: __RINvNtNtNtCs1H1hfzekmJm_5taffy7compute4grid12track_sizing29resolve_intrinsic_track_sizesNtNtNtNt... -> taffy::compute::grid::track_sizing::resolve_intrinsic_track_sizes::<cove_core::scene::layout_node::engine::LayoutTree, ...>
PASS  crate groups name real Rust crates: cove_ui_compose
PASS  non-runtime display target display:1 -> unavailable: App Size measures the native executable of a running Cove runtime. The selected target is a platform window, display or browser session, which has no runtime executable.
RESULT PASS
```

The first live attempt exposed a harness bug rather than a product bug: Mach-O prefixes mangled symbols with an extra underscore (`__RNv…`), so a `^_ZN|^_R` demangling check matched nothing even though the product had demangled every Rust name. The check now accepts `^__?(ZN|R)`, requires a `::` path in the demangled name, and bans the same prefixes on group names. The next attempt failed on the CLI's error shape: `cove-devtools call` prints the JSON-RPC error object directly (top-level `code`/`data.code`/`message`), not the envelope, so the assertion now accepts both shapes. The reruns passed.

The display assertion ran against the real macOS target list (`display:1`). The error carried `data.code: "unavailable"`, the message above, and no size fields. The controller had Screen Recording and Accessibility off; that did not affect App Size, which reads the executable file.

The client-side App Size view was exercised separately with its existing fake-controller harness, which is unaffected by this script:

```sh
node crates/devtools/client/tests/app-size-harness.mjs   # RESULT PASS
```

It covered success, unavailable, parse/read error, both truncation caps, malformed payload, empty payload, remote target, and diff snapshots. That harness uses a fake controller; the run above is the live product proof.

After every run no gallery or controller process remained (`ps` showed none; the isolated port was closed), and the user's live discovery file kept its earlier timestamp and `http://127.0.0.1:9229` endpoint. The failure branches not observed live — `file_bytes` mismatch, malformed rows, inconsistent truncation — are asserted by jq expressions on the live response; the live values shown above exercised the true side of each branch.

## Paged tree: `query_ui_tree` under the agent result budget

Script: `Tools/DevTools/acceptance/tree-page.sh`

```sh
Tools/DevTools/acceptance/tree-page.sh            # builds both binaries, removes its run directory on success
Tools/DevTools/acceptance/tree-page.sh --no-build --keep
```

The browser client reads the full `inspect_ui_tree` summary, but the agent/MCP toolbox refuses any JSON tool result above 512 KiB. On a live gallery the full summary is ~1.1 MB, so `cove-devtools call inspect_ui_tree` fails with the toolbox's `AgentError::InvalidResult`. This harness proves the bounded query is the agent's way in.

### Exact steps

1. Preflight `cargo`, `python3`, `jq`, `curl`; build `target/debug/cove-devtools` and `target/debug/gallery` unless `--no-build`.
2. Require `query_ui_tree` and `inspect_ui_tree` in `cove-devtools tools`, with `query_ui_tree` accepting `depth` and `limit` and requiring no argument the harness does not send.
3. Allocate an ephemeral loopback port, refusing 9229, and a random token. Export `COVE_DEVTOOLS_SESSION_FILE`, `COVE_DEVTOOLS_TOKEN` and `COVE_DEVTOOLS_ENDPOINT` to the controller, the gallery and every CLI call. The run owns its discovery file under `$TMPDIR/cove-devtools-tree-page.*`; the user's live controller is never read or written.
4. Start the controller with `cove-devtools serve --port <ephemeral>`, then the gallery with `COVE_DEVTOOLS=1`. Select the runtime whose `pid` is exactly this run's gallery PID.
5. Fetch the live full tree through the controller's own `/rpc` route as ground truth (`inspect_ui_tree`), flatten it once (id, zero-based depth, absolute frame, live child IDs) and require it to exceed 524288 compact bytes.
6. Call `inspect_ui_tree` through `cove-devtools call`; require a non-zero exit and the 512 KiB/result-limit refusal.
7. Call `query_ui_tree` with `depth: 2, limit: 200`, then `depth: 1, limit: 50`, then `depth: 16, limit: 10` (the limit-cap case):
   - valid JSON, a non-empty `root.id`, an explicit numeric `nodeCount` and a boolean `truncated`;
   - the page root is the live scene root;
   - every returned ID exists in the live full tree, with no duplicates;
   - every returned node keeps the live node's `absoluteFrame`, and its child list is a prefix of the live child list;
   - the page holds at most `limit` nodes and at most `depth` levels (the page root is level 1);
   - `nodeCount` equals the number of nodes in the returned subtree;
   - the returned prefix is `min(limit, live nodes within depth)`, and `truncated` is true exactly when the depth, the node limit or the byte budget omitted live nodes;
   - the compact response is under 524288 bytes.
8. Call `query_ui_tree` with `node_id` set to a real depth-1 child (`depth: 2, limit: 20`); require that ID as the page root, live IDs only, and the same frame/edge agreement.
9. Call it with a nonexistent node ID and require `stale_node` both through the CLI and over `/rpc`. Call it with `depth: 10000` and `limit: 0`: the CLI schema refuses them before the controller, and `/rpc` answers `invalid_request` naming the field.
10. Print `RESULT PASS` and exit 0. Traps stop the gallery and controller by PID (TERM, then KILL after five seconds).

### Wire contract asserted

`query_ui_tree` arguments: `target` (optional), `node_id` (optional live element ID), `depth` (1-16, default 4), `limit` (1-2000, default 500).

```json
{
  "root": {
    "id": "1",
    "frame": {"x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0},
    "absoluteFrame": {"x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0},
    "children": []
  },
  "depth": 2,
  "limit": 200,
  "nodeCount": 2,
  "truncated": true
}
```

`nodeCount` counts the live nodes actually returned. `truncated` is true when the depth, the node limit or the controller's serialised-page byte budget omitted live nodes. Children omitted by any bound are absent; no node is synthesized. Optional fields (`viewTag`, `debugName`, `layoutRole`, `semanticRole`, `source`, `children`) are omitted when absent.

Faults arrive in both shapes, depending on the caller:

```json
{"error": {"code": -32000, "message": "No live node has element ID ...", "data": {"code": "stale_node"}}}
{"error": {"code": -32000, "message": "$.depth: must match exactly one allowed schema (matched 0)", "data": {"code": "invalid_request"}}}
```

The controller validates the tool schema before dispatch, so out-of-range `depth`/`limit` answer the schema message rather than the inspector's own text; the code is still `invalid_request`.

### Pass conditions

Exit code 0 and every line below printed (11 checks in the verified run):

```text
PASS  controller ready at http://127.0.0.1:<ephemeral> (default port 9229 not used)
PASS  runtime selected: <id> (CoveUI — Component Showcase, pid <pid>)
PASS  live full tree exceeds the agent budget: <N> bytes > 524288
PASS  unpaged inspect_ui_tree refused by the agent budget: Tool returned invalid JSON/media or exceeded the result limit (JSON: 512 KiB; images: 1 MiB encoded).
PASS  depth-2-limit-200: 2 nodes, 2 of 2 levels, truncated=true, 491 bytes (< 524288), root 1
PASS  depth-1-limit-50: 1 nodes, 1 of 1 levels, truncated=true, 174 bytes (< 524288), root 1
PASS  depth-16-limit-10: 10 nodes, 10 of 16 levels, truncated=true, 2917 bytes (< 524288), root 1
PASS  subtree query rooted at real node 2: 2 nodes
PASS  nonexistent node <id> -> controller [stale_node]: No live node has element ID <id>.
PASS  depth 10000 -> controller [invalid_request]: $.depth: must match exactly one allowed schema (matched 0)
PASS  limit 0 -> controller [invalid_request]: $.limit: must match exactly one allowed schema (matched 0)
RESULT PASS  Paged tree: query_ui_tree under the agent budget, 11 checks
```

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Pass; paged results were real, bounded, under 512 KiB, and invalid requests were refused |
| 1 | Functional failure: missing page fields, non-live IDs, changed frames/edges, wrong node count, wrong truncation flag, over-budget page, unpaged tool unexpectedly succeeding, wrong fault code |
| 2 | Blocked: missing dependency, missing tool or schema, build failure, controller/gallery did not start, no runtime registered, full tree does not exceed the agent budget |
| 130/143 | Interrupted; processes were stopped by the traps |

### Blockers and limitations

- **GUI runtime required**: a headless session registers no gallery runtime, so the harness blocks instead of inventing a tree.
- **Overflow premise**: the live full tree must exceed 524288 bytes. A small app or display would fail the premise assertion; the gallery is ~1.1 MB.
- **Depth semantics**: `depth` counts node levels including the page root. The gallery's top is a single chain, so depth 2 holds 2 nodes and depth 1 holds 1; the depth-16/limit-10 call exercises the node cap, and all three must report `truncated=true`.
- **Byte-budget trimming not observed live**: the controller shrinks pages above its 384 KiB serialised ceiling. These bounded calls stay under 3 KiB, so the trimming loop is asserted only by the contract, not triggered here. A wide tree at `limit: 2000` would exercise it.
- **Schema-first errors**: the CLI and controller validate the tool schema before inspector dispatch, so the harness asserts the explicit `invalid_request` code and field name rather than the inspector's wording.
- **No product or client edits**: the harness only calls public controller routes and the CLI toolbox, and stores the responses it received.

### Current verification status

`bash -n Tools/DevTools/acceptance/tree-page.sh` passes. The script was run end-to-end five times against isolated controllers and the real gallery on this macOS host; the run before the last used the default build path, and the last used `--no-build`; both removed their run directory.

```sh
cargo build -p cove-devtools-cli --bin cove-devtools   # exit 0
cargo build -p cove-examples --bin gallery             # exit 0
Tools/DevTools/acceptance/tree-page.sh --no-build      # exit 0, 11 checks
Tools/DevTools/acceptance/tree-page.sh                 # exit 0, 11 checks, run directory removed
```

Final run (`--no-build`, isolated controller on `127.0.0.1:57645`, gallery pid 57695, runtime `6b8ebc4c609403f7e477a2f75064068f6ab4edfb8bd09b25`):

```text
Live tree: 1884 nodes, max depth 45, root 1, compact JSON 1135829 bytes
PASS  live full tree exceeds the agent budget: 1135829 bytes > 524288
PASS  depth-2-limit-200: 2 nodes, 2 of 2 levels, truncated=true, 491 bytes
PASS  depth-1-limit-50: 1 nodes, 1 of 1 levels, truncated=true, 174 bytes
PASS  depth-16-limit-10: 10 nodes, 10 of 16 levels, truncated=true, 2917 bytes
PASS  subtree query rooted at real node 2: 2 nodes
RESULT PASS
```

After the run no gallery or controller from the harness remained (`ps` showed none on its port), and the user's live discovery file kept its earlier timestamp and endpoint.

The first two live attempts exposed harness bugs, not product bugs: `depth` counts levels (depth 2 returned root plus one child, not three zero-based levels), and depth 30 is outside the tool's 16-level schema. The harness now computes live prefix sizes with the level-counting semantics and uses depth 16 / limit 10 for the limit-cap call. Both fixes were re-run to the pass above.
