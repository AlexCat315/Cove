# `cove-hosts` — platform coverage matrix

Source: `crates/hosts/src/` (67 files, 10,452 lines). The Swift desktop host (`Hosts/Desktop/Sources/CoveUI`,
26 files) and Android host (`Hosts/Android/Sources/`, 15 files) are deleted; only the local, untracked
`swift/` reference tree retains their paths.

## Threads

| Platform | Platform pump | Composition, layout, draw lists | GPU |
|---|---|---|---|
| macOS, Windows, Linux | main thread (`PlatformLoop`) | `cove-ui` | `cove-render-*` |
| iOS | main thread, `CADisplayLink` | main thread | `cove-render-*` |
| Android | Java main thread, `Choreographer` | same thread, through JNI | `cove-render-android-surface` |

Native web views belong to the platform thread on every desktop platform; wrap the host in `PlatformThreadWebViewHost`. `COVE_SINGLE_THREAD=1` runs the desktop UI on the main thread and `COVE_SYNC_RENDER=1` presents on the calling thread.

## Coverage matrix

| Platform | What Hosts implements | Deliberately not implemented | Named limitation |
|---|---|---|---|
| **macOS** | Full desktop host. Window lifecycle (create/resize/close/minimize/maximize/restore/hide/show), display scaling + safe area via SDL3, swapchain presentation with owned framebuffer + frosted backdrop blit, glyph-atlas upload, chrome hit-test collection, auxiliary windows, native menu **action table + presenter**, memory pressure (level-read), image transport via `cove-http` (NSURLSession, real `Content-Type`), accessibility sync | `NSApplication.mainMenu` bar installation (shell has no entry point — see limitation 6); `dispatch_source` memory-pressure source (level is read, not owned) | Menu: "the application menu bar is not installed; `cove-shell` exposes only `cove_shell_menu_popup`, so an application menu bar is not shown and the Compose menu path remains the working one." Memory: "macOS pressure level is published by the host and read by the sampler; the process does not own a `dispatch_source`." |
| **iOS** | Same desktop host code path, `#[cfg(target_os = "ios")]`-selected: touch+keyboard input capabilities, `DeviceClass::Phone` or `Tablet` from `SDL_IsTablet` (iPad reports `Platform::IPadOS`), SDL3/UIKit safe area, full window/presentation/renderer/text stack. Image transport via `cove-http` (NSURLSession, shared with macOS). Frame loop uses `cove-runtime`'s `IOSFrameDriver` (CADisplayLink) instead of the blocking loop | No iOS-specific host file: the application's Rust `main` calls `ApplicationLaunch::launch`, which enters UIKit through `SDL_RunApp`, and frames stay on the UIKit main thread while presentation runs on a render thread; `cargo xtask package ios` bundles that binary as the `.app`. Built and signed for the Simulator; not yet launched | "iOS is served by the shared desktop host with mobile environment settings; the entry point is the application binary declared with `cove_ui::application!`." |
| **Windows** | Full desktop host: window lifecycle, DPI/scale, swapchain, rendering, resources, diagnostics. Native menu install is a deliberate **no-op** (matches Swift: immersive windows draw their own title bar). Memory pressure via `CreateMemoryResourceNotification`/`QueryMemoryResourceNotification`. Image transport via `cove-http` (WinHTTP, automatic proxy, real `Content-Type`) | Native menu strip (by design, as in Swift) | None for core hosting. Menus: "Windows draws its own title bar in the client area, so a Win32 menu strip would sit below it; applications present the same menu tree inside that title bar instead." (ported verbatim in intent from the Swift comment) |
| **Linux** | Full desktop host: window lifecycle, scale, swapchain (X11/Wayland handles via `cove-shell`), rendering, resources, diagnostics. Memory pressure reads `/proc/meminfo` **and** cgroup v2 `memory.current`/`memory.max` (container limit, not just host RAM). Image transport via `cove-http`: HTTP/1.1 over `std::net`, TLS through the system OpenSSL (`libssl` 1.1.1–4.x, loaded at run time) | — | Without `libssl`, `TransportCapabilities::platform().https == false` with the limitation **"https needs the system OpenSSL (libssl), which could not be loaded"**. |
| **Android** | Full Android host (15/15 Swift files): activity-retained application, JNI bridge (all 18 `@_cdecl` entry points preserved; `cove_android_surface` / `MainActivity.surface` now carry device class, posture and display features instead of an idiom; the `Java_com_cove_probe_*` layer is Rust in `android/jni.rs`, application content arrives through `install_application_content`), surface session (swapchain, owned framebuffer, frosted-panel blit, texture teardown), content session (launch-mode selection, layer/direct rendering, input routing), frame timings + rolling GPU pass timings, device-fact surface updates (device class, fold posture and `FoldingFeature` display features from Jetpack WindowManager `androidx.window:window(-java):1.4.0`, safe area, keyboard, text scale; DevTools `set_ui_environment` overrides layer over them), system-bar icon style, memory-pressure trim, diagnostics pump. NDK symbols resolved by `dlopen`/`dlsym` (`liblog.so`, `libandroid.so`) — no new crate dependency; the Gradle runtime library depends on `androidx.window` and `androidx.core`. Logcat forwarding wired. Probe (`CoveAndroidProbe`) ported fully in-crate on `cove-graphics` only | Showcase/counter/gallery **content** is injected via `HostedSceneContent`/`ShowcaseProducts` traits — the host owns the machinery and the platform app supplies the products. Image transport via `cove-http` (HTTP/1.1 + OpenSSL); https needs `libssl.so`/`libcrypto.so` bundled from `COVE_OPENSSL_ANDROID_DIR` by `cargo xtask android-prepare` | "Android showcase and application content is supplied by the platform app through `HostedSceneContent`/`ShowcaseProducts`; `cove-hosts` carries no dependency on the example product crates." |
| **Wasm** | Nothing. `cove-http` has no backend on Wasm, so `install_platform_services()` leaves the image-transport holder unset | Everything. `cove-hosts` is **not** a dependency of `cove-browser` (the Wasm host); they are independent crates | `TransportCapabilities` = `{http: false, https: false, file: true}`; limitation = **"no HTTP stack on this target"**. `file:` still works because `ImageDataDownload` handles it without a transport. |

## Screen-colour sampling

`cove-platform` holds only the contract (`ScreenColor`, `ScreenColorSample`, `ScreenColorCompletion`, `ScreenColorBackend`) and has no window-system dependency. Every native implementation lives in `cove-shell/src/color_sampler/`; all of them are asynchronous, queue their result through a thread-safe sink, wake the event loop, and let `Shell::take_deferred_callbacks()` deliver the callback on the platform thread.

| Platform | Backend | Native mechanism | Limitation |
| --- | --- | --- | --- |
| macOS | `AppKit` | `NSColorSampler` via `objc2-app-kit` + `block2` | macOS 10.15+; must start on the application's main thread, which `run_with_ui_thread` guarantees; colour converted to sRGB |
| Windows | `Win32` | `GetCursorPos` + `GetDC(NULL)` + `GetPixel` on a worker thread | Win32 has no desktop eyedropper (WinUI/Chromium `Eyedropper` only reads its own window); no colour management, alpha opaque; ESC/right-click cancel |
| Linux (X11) | `X11` | `XGetImage` root-window screenshot, then one-pixel sample on a worker thread | `libX11.so.6` resolved with `dlopen`; pixels decoded from the XImage channel masks |
| Linux (Wayland) | `XdgPortal` | `org.freedesktop.portal.Screenshot.PickColor` over the session bus | Needs a portal implementing `PickColor`; otherwise falls back to X11 (XWayland root image excludes Wayland-native windows), then `Unsupported` |
| iOS / Android / Wasm | `None` | — | The OS does not expose other apps' screen pixels; `ScreenColorSample::Unsupported`, eyedropper hidden |

Hosts install the `ColorSampler` ambient only when `Sdl3PlatformHost::screen_color_backend().is_available()`. A new pick supersedes the previous one (`Cancelled`), and `Sdl3Shell::shutdown()` cancels every in-flight worker. Native picker interaction on Windows and Linux has not been visually accepted on real hardware yet; the wire codec, pixel decoding and async queue are covered by structural tests.

## Process memory counters

`process_memory` is the shared reader behind the `processResidentBytes` and `processVirtualBytes` entries of `query_ui_memory`, installed by both the desktop `Application::diagnostic_memory_snapshot` and the Android memory closure. It reads the kernel once per query; a target without a reader omits the counters (`ProcessMemory` leaves the field `None`) and publishes the reason through `process_memory::unavailable_reason()` — a missing reading is never reported as zero.

| Platform | Resident | Virtual | Backend | Limitation |
| --- | --- | --- | --- | --- |
| macOS | `pti_resident_size` | `pti_virtual_size` (address space) | `proc_pidinfo(PROC_PIDTASKINFO)` | — |
| iOS | `resident_size` | `virtual_size` (address space) | `task_info(MACH_TASK_BASIC_INFO)` | iOS has no `proc_pidinfo`; the Mach task port is the public reading |
| Linux, Android | `VmRSS` | `VmSize` (address space) | `/proc/self/status` | A kernel without those fields falls back to omitting the counter |
| Windows | `WorkingSetSize` | `PagefileUsage` | `K32GetProcessMemoryInfo` + `PROCESS_MEMORY_COUNTERS` | `PROCESS_MEMORY_COUNTERS` has no address-space field; the virtual counter is private commit ("Commit size"), not reserved address space |
| Wasm, other targets | — | — | none | No process address space to sample; both counters are omitted |

`cargo test -p cove-hosts --lib` asserts a positive resident reading and `resident <= virtual` for the address-space backends. Each other backend compiles under its own `cfg`; a full `cargo check -p cove-hosts --target <triple>` additionally needs that platform's native SDK.

### Platform gate counts (`target_os = "…"` occurrences)

| Gate | Occurrences |
|---|---|
| `macos` | 16 |
| `android` | 6 |
| `windows` | 5 |
| `linux` | 3 |
| `ios` | 3 |

## Allocation counters

`allocation` adds the `cove-alloc` heap counters to the same `query_ui_memory` answer: `allocCurrentBytes`, `allocPeakBytes`, `allocTotalBytes`, `allocDeallocatedBytes`, `allocLiveBlocks`, `allocPeakLiveBlocks`, `allocTotalBlocks`, `allocDeallocatedBlocks`, plus one `allocSizeClass…LiveBytes` entry per size class (64 B, 256 B, 1 KiB, 4 KiB, 16 KiB, 64 KiB, 1 MiB, larger). The application opts in by installing `cove_alloc::TrackingAllocator` as its `#[global_allocator]`; `examples/gallery/src/bin/gallery.rs` does. A binary that does not install it publishes none of these entries — `cove_alloc::snapshot()` is `None` until the allocator has served its first allocation, so "not instrumented" is never reported as zero live bytes.

The counters are measured in the allocator from the caller's `Layout`: no per-allocation header, no call stack. They measure bytes and blocks only. Rust object types, retaining paths, garbage collection and leaks are **not** available and are not approximated: a live block is one that has not been released, whether or not it is reachable.

`cargo test -p cove-alloc` runs the lib unit tests plus two standalone binaries: `tests/tracking.rs` installs the tracking allocator as `#[global_allocator]` and asserts the counters rise, peak/current stay plausible (peak ≥ current, total ≥ current, live + released = total, size classes sum to the totals), and that realloc/dealloc accounting is exact; `tests/no_install.rs` asserts an uninstrumented process reports nothing and that one served allocation flips the counters on.

## The three behaviours that could not be ported faithfully

1. **DevTools is feature-gated.** With the `devtools` feature, a host launched with `COVE_DEVTOOLS=1` registers its runtime probe with the controller. Inspectors read diagnostics on demand. The host supplies Cove framebuffer pixels and dispatches app-local input. See [DevTools](../../devtools/README.md).

2. **macOS memory pressure is read, not owned.** The sampler polls a published pressure level (`darwin::set_level`) instead of owning a `dispatch_source` with an event handler. Linux and Windows use their real mechanisms (`/proc/meminfo` + cgroup v2; `CreateMemoryResourceNotification`). **Reason:** a `dispatch_source` event handler is an Objective-C block, which this crate does not hand-encode. All three platforms sample on a worker thread and only raise a flag that the UI thread drains, because `Sdl3PlatformHost` is `!Send` and stays on the UI thread.

3. **Android showcase/gallery/counter content is injected, not linked.** `HostedScene` takes a `Box<dyn HostedSceneContent>` and `ShowcaseContent` takes `&mut dyn ShowcaseProducts`; every host-side behaviour is ported (launch-mode selection, direct-vs-layers rendering, checkpoint decode plus the `CHECKPOINT_CORRUPT` error, page reporting, per-frame typography metrics, atlas-full re-prepare). **Reason:** the Swift originals imported `CounterExample` and `GalleryContent`, which are not dependencies of `cove-hosts`. Inventing placeholder content would have been a stub, so the products are a seam. The seam is genuinely shared — the desktop host uses the same `AdaptiveApplicationContent` wrapper.

## Known wiring gaps (called out, not hidden)

6. ~~**`install_platform_services()` is callable but never called.**~~ **CLOSED.** Both host boot paths now install the platform services:
   - desktop: `Application::run` in `desktop/application/runtime.rs`, before the run loop, logging the capability report;
   - Android: `cove_android_create_application` in `android/sources/cove_android_app/bridge.rs`, before the first frame.
   Remote `http`/`https` image loading is therefore live on macOS, iOS, Windows and Linux (Linux `http` only, see deviation 1). A host that embeds `cove-hosts` differently must still call `install_platform_services()` itself.

7. **The macOS application menu bar is not installed.** `cove-shell` exposes only `cove_shell_menu_popup` (context popups). `NativeMenuBar::snapshot()` already produces the tree an `NSApplication.mainMenu` install needs, and `native_menu_bridge` routes chosen ids back to authored closures — but there is no shell entry point to hand the bar to.

## Mobile parity additions (2026-11-09)

These close implementation gaps on both mobile targets; structural tests pass, **device acceptance is still outstanding** (see `Docs/gallery-audit.md`).

| Area | iOS | Android |
|---|---|---|
| Keyboard inset | `UIKeyboardWillShow/WillHide` observers in `shell/native/apple.m` cache the frame, wake the SDL loop, and `Sdl3PlatformHost::keyboard_height()` publishes it into `Environment.keyboard` every frame | Already supplied by `SurfaceMetrics` / `WindowInsets.Type.ime()`; now reachable through the same shared `Shell` API |
| Explicit soft keyboard | `Shell::{show_soft_keyboard,hide_soft_keyboard,keyboard_height}` + `cove_shell_keyboard` (SDL text-input start/stop) | Java `InputMethodManager.showSoftInput/hideSoftInput` driven by `inputSnapshot`; unchanged, no platform omission |
| Persistent app storage | Sandbox `HOME/Library/Application Support` | `COVE_APP_STORAGE_DIR` injected by the `createApplication` JNI entry, then the parent of `PACKAGE_RESOURCE_BUNDLE_PATH` (`filesDir`); `$HOME/.local/share` is gone |
| Process services | All launchers return `ProcessError::Unavailable` | Same guard; no fork/exec through `cfg(unix)` |
| Bundle metadata | `UISupportedInterfaceOrientations` + `~ipad` added to `bundle_ios` | Orientation already handled by `MainActivity.setRequestedOrientation` |

Still open, reported rather than hidden: no iOS-specific host or `UITextInput` (SDL3 UIKit has no `SDL_SendEditingText` path in this tree, so native composition preview is unverified); Android's generic `ClipboardHolder` backend is not wired to a JNI clipboard (only IME context-menu copy/cut/paste works); multi-pointer gestures beyond the primary pointer; Storage Access Framework / runtime permissions; bundled OpenSSL for Android https; native WebView on both mobile targets; application foreground/background notifications; and screen-colour sampling, which the OS does not expose on either target.

## Swift-source coverage

| Area | Swift files | Rust files |
|---|---|---|
| Android app (application, bridge, content, frame, surface, validation) | 11 | 16 |
| Android probe | 6 | 7 |
| Desktop application | 9 | 7 |
| Desktop diagnostics | 5 | 7 |
| Desktop display | 2 | 3 |
| Desktop rendering | 1 | 2 |
| Desktop resources support | 2 | 1 |
| Desktop window | 6 | 6 |
| Android contract + integration (already migrated) | — | 11 |
| Host-installed services (new: image transports) | — | 6 |
| Crate roots (`lib.rs`, `android/mod.rs`, `desktop/mod.rs`) | — | 3 |
| **Total** | **43 pending + 10 migrated** | **69** |
