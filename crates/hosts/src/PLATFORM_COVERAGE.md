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
| **iOS** | Same desktop host code path, `#[cfg(target_os = "ios")]`-selected: touch+keyboard input capabilities, `Idiom::Phone`, SDL3/UIKit safe area, full window/presentation/renderer/text stack. Image transport via `cove-http` (NSURLSession, shared with macOS). Frame loop uses `cove-runtime`'s `IOSFrameDriver` (CADisplayLink) instead of the blocking loop | No iOS-specific host file: the application's Rust `main` calls `ApplicationLaunch::launch`, which enters UIKit through `SDL_RunApp`, and frames stay on the UIKit main thread while presentation runs on a render thread; `cargo xtask package ios` bundles that binary as the `.app`. Built and signed for the Simulator; not yet launched | "iOS is served by the shared desktop host with mobile environment settings; the entry point is the application binary declared with `cove_ui::application!`." |
| **Windows** | Full desktop host: window lifecycle, DPI/scale, swapchain, rendering, resources, diagnostics. Native menu install is a deliberate **no-op** (matches Swift: immersive windows draw their own title bar). Memory pressure via `CreateMemoryResourceNotification`/`QueryMemoryResourceNotification`. Image transport via `cove-http` (WinHTTP, automatic proxy, real `Content-Type`) | Native menu strip (by design, as in Swift) | None for core hosting. Menus: "Windows draws its own title bar in the client area, so a Win32 menu strip would sit below it; applications present the same menu tree inside that title bar instead." (ported verbatim in intent from the Swift comment) |
| **Linux** | Full desktop host: window lifecycle, scale, swapchain (X11/Wayland handles via `cove-shell`), rendering, resources, diagnostics. Memory pressure reads `/proc/meminfo` **and** cgroup v2 `memory.current`/`memory.max` (container limit, not just host RAM). Image transport via `cove-http`: HTTP/1.1 over `std::net`, TLS through the system OpenSSL (`libssl` 1.1.1–4.x, loaded at run time) | — | Without `libssl`, `TransportCapabilities::platform().https == false` with the limitation **"https needs the system OpenSSL (libssl), which could not be loaded"**. |
| **Android** | Full Android host (15/15 Swift files): activity-retained application, JNI bridge (**all 18 `@_cdecl` entry points preserved** with identical names/signatures; the `Java_com_cove_probe_*` layer is Rust in `android/jni.rs`, application content arrives through `install_application_content`), surface session (swapchain, owned framebuffer, frosted-panel blit, texture teardown), content session (launch-mode selection, layer/direct rendering, input routing), frame timings + rolling GPU pass timings, device-fact surface updates (idiom/safe-area/keyboard/text-scale), system-bar icon style, memory-pressure trim, diagnostics pump. NDK symbols resolved by `dlopen`/`dlsym` (`liblog.so`, `libandroid.so`) — **no new crate dependency**. Logcat forwarding wired. Probe (`CoveAndroidProbe`) ported fully in-crate on `cove-graphics` only | Showcase/counter/gallery **content** is injected via `HostedSceneContent`/`ShowcaseProducts` traits — the host owns the machinery and the platform app supplies the products. Image transport via `cove-http` (HTTP/1.1 + OpenSSL); https needs `libssl.so`/`libcrypto.so` bundled from `COVE_OPENSSL_ANDROID_DIR` by `cargo xtask android-prepare` | "Android showcase and application content is supplied by the platform app through `HostedSceneContent`/`ShowcaseProducts`; `cove-hosts` carries no dependency on the example product crates." |
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

### Platform gate counts (`target_os = "…"` occurrences)

| Gate | Occurrences |
|---|---|
| `macos` | 16 |
| `android` | 6 |
| `windows` | 5 |
| `linux` | 3 |
| `ios` | 3 |

## The three behaviours that could not be ported faithfully

1. **DevTools is not linked into `cove-hosts`.** Diagnostic sessions use the pre-existing injected-factory seam (`set_diagnostics_factory` / `NullDiagnosticsSession`), so by default there is no DevTools server, inspection overlay, input interception, or mirror capture. **Reason:** `cove-hosts` has no `cove-devtools` dependency and adding one is a workspace-graph change; the seam was already established in this crate before my work and I preserved it rather than re-architecting.

2. **macOS memory pressure is read, not owned.** The sampler polls a published pressure level (`darwin::set_level`) instead of owning a `dispatch_source` with an event handler. Linux and Windows use their real mechanisms (`/proc/meminfo` + cgroup v2; `CreateMemoryResourceNotification`). **Reason:** a `dispatch_source` event handler is an Objective-C block, which this crate does not hand-encode. All three platforms sample on a worker thread and only raise a flag that the UI thread drains, because `Sdl3PlatformHost` is `!Send` and stays on the UI thread.

3. **Android showcase/gallery/counter content is injected, not linked.** `HostedScene` takes a `Box<dyn HostedSceneContent>` and `ShowcaseContent` takes `&mut dyn ShowcaseProducts`; every host-side behaviour is ported (launch-mode selection, direct-vs-layers rendering, checkpoint decode plus the `CHECKPOINT_CORRUPT` error, page reporting, per-frame typography metrics, atlas-full re-prepare). **Reason:** the Swift originals imported `CounterExample` and `GalleryContent`, which are not dependencies of `cove-hosts`. Inventing placeholder content would have been a stub, so the products are a seam. The seam is genuinely shared — the desktop host uses the same `AdaptiveApplicationContent` wrapper.

## Known wiring gaps (called out, not hidden)

6. ~~**`install_platform_services()` is callable but never called.**~~ **CLOSED.** Both host boot paths now install the platform services:
   - desktop: `Application::run` in `desktop/application/runtime.rs`, before the run loop, logging the capability report;
   - Android: `cove_android_create_application` in `android/sources/cove_android_app/bridge.rs`, before the first frame.
   Remote `http`/`https` image loading is therefore live on macOS, iOS, Windows and Linux (Linux `http` only, see deviation 1). A host that embeds `cove-hosts` differently must still call `install_platform_services()` itself.

7. **The macOS application menu bar is not installed.** `cove-shell` exposes only `cove_shell_menu_popup` (context popups). `NativeMenuBar::snapshot()` already produces the tree an `NSApplication.mainMenu` install needs, and `native_menu_bridge` routes chosen ids back to authored closures — but there is no shell entry point to hand the bar to.

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
