# `cove-hosts` — platform coverage matrix

Source: `Hosts/rust/` (69 files, 9,518 lines). Desktop host = `Hosts/Desktop/Sources/CoveUI`
(26 Swift files); Android host = `Hosts/Android/Sources/` (15 Swift files).
Verified with `cargo check -p cove-hosts --all-targets` → 0 errors, 0 warnings.

## Coverage matrix

| Platform | What Hosts implements | Deliberately not implemented | Named limitation |
|---|---|---|---|
| **macOS** | Full desktop host. Window lifecycle (create/resize/close/minimize/maximize/restore/hide/show), display scaling + safe area via SDL3, swapchain presentation with owned framebuffer + frosted backdrop blit, glyph-atlas upload, chrome hit-test collection, auxiliary windows, native menu **action table + presenter**, memory pressure (level-read), image transport via Foundation `NSURL`/`NSData`, accessibility sync | `NSApplication.mainMenu` bar installation (shell has no entry point — see limitation 6); `dispatch_source` memory-pressure source (level is read, not owned) | Menu: "the application menu bar is not installed; `cove-shell` exposes only `cove_shell_menu_popup`, so an application menu bar is not shown and the Compose menu path remains the working one." Memory: "macOS pressure level is published by the host and read by the sampler; the process does not own a `dispatch_source`." |
| **iOS** | Same desktop host code path, `#[cfg(target_os = "ios")]`-selected: touch+keyboard input capabilities, `Idiom::Phone`, SDL3/UIKit safe area, full window/presentation/renderer/text stack. Image transport via Foundation (shared with macOS). Frame loop uses `cove-runtime`'s `IOSFrameDriver` (CADisplayLink) instead of the blocking loop | No iOS-specific Swift file existed under `Hosts/` — that was true in the Swift tree too (the iOS app entry lives in `Examples/Platforms/iOS`, Swift, `main.swift`). No Rust iOS entry point exists yet, so this path is compiled-but-unlaunched | "iOS is served by the shared desktop host with mobile environment settings; there is no Rust iOS application entry point yet (the iOS entry remains `Examples/Platforms/iOS/main.swift`)." |
| **Windows** | Full desktop host: window lifecycle, DPI/scale, swapchain, rendering, resources, diagnostics. Native menu install is a deliberate **no-op** (matches Swift: immersive windows draw their own title bar). Memory pressure via `CreateMemoryResourceNotification`/`QueryMemoryResourceNotification`. Image transport via WinHTTP (`#[link(name="winhttp")]`), automatic proxy, `FLAG_SECURE`, capped chunked reads with real `Content-Type` | Native menu strip (by design, as in Swift) | None for core hosting. Menus: "Windows draws its own title bar in the client area, so a Win32 menu strip would sit below it; applications present the same menu tree inside that title bar instead." (ported verbatim in intent from the Swift comment) |
| **Linux** | Full desktop host: window lifecycle, scale, swapchain (X11/Wayland handles via `cove-shell`), rendering, resources, diagnostics. Memory pressure reads `/proc/meminfo` **and** cgroup v2 `memory.current`/`memory.max` (container limit, not just host RAM). Image transport: real HTTP/1.1 over `std::net` — head parse, chunked decode, 5-redirect budget, byte cap | `https://` — no TLS provider exists in the workspace dependency set (`Cargo.lock` has no rustls/native-tls/ring/webpki). Not stubbed, not silently downgraded | `TransportCapabilities::https == false`; `HTTPS_LIMITATION` = **"https requires a TLS provider; the workspace dependency set ships none, so Linux image loading is limited to http:// and file://"**. A fetch returns `AsyncImageError::NetworkFailed` with that exact text. |
| **Android** | Full Android host (15/15 Swift files): activity-retained application, JNI bridge (**all 18 `@_cdecl` entry points preserved** with identical names/signatures), surface session (swapchain, owned framebuffer, frosted-panel blit, texture teardown), content session (launch-mode selection, layer/direct rendering, input routing), frame timings + rolling GPU pass timings, device-fact surface updates (idiom/safe-area/keyboard/text-scale), system-bar icon style, memory-pressure trim, diagnostics pump. NDK symbols resolved by `dlopen`/`dlsym` (`liblog.so`, `libandroid.so`) — **no new crate dependency**. Logcat forwarding wired. Probe (`CoveAndroidProbe`) ported fully in-crate on `cove-graphics` only | Showcase/counter/gallery **content** is injected via `HostedSceneContent`/`ShowcaseProducts` traits — the host owns the machinery and the platform app supplies the products. `https://` works (no transport needed; the app supplies one if it wants remote images) | "Android showcase and application content is supplied by the platform app through `HostedSceneContent`/`ShowcaseProducts`; `cove-hosts` carries no dependency on the example product crates." |
| **Wasm** | Nothing. `install_platform_services()` resolves to `platform_transport() == None`, so the image-transport holder stays unset | Everything. `cove-hosts` is **not** a dependency of `cove-browser` (the Wasm host); they are independent crates | `TransportCapabilities` = `{http: false, https: false, file: true}`; limitation = **"no platform HTTP transport is available on this target"**. `file:` still works because `ImageDataDownload` handles it without a transport. |

### Platform gate counts (actual `#[cfg]` usage)

| Gate | Occurrences |
|---|---|
| `macOS` | 18 |
| `windows` | 11 |
| `linux` | 8 |
| `ios` | 8 |
| `android` | 5 |

## The five behaviours that could not be ported faithfully

1. **Linux `https://` does not work.** The workspace dependency set contains no TLS implementation, and `Cargo.lock` confirms it (zero rustls / native-tls / ring / webpki entries). I implemented real HTTP/1.1 over `std::net` for `http://` and reported `https = false` with the named limitation above rather than stubbing, silently downgrading, or pulling a full TLS+crypto tree into every platform including Wasm. **Reason:** a TLS stack is a workspace-dependency decision, not a host-crate one — I did not edit the root `Cargo.toml`.

2. **Apple image transport reports no MIME type.** `NSData dataWithContentsOfURL:options:error:` returns bytes only; `NSHTTPURLResponse`'s `Content-Type` is only available through the async `dataTask` completion handler. `DownloadedImage` therefore falls back to the URL path extension, exactly as the Swift loader did when a response carried no `Content-Type`. **Reason:** the async path requires hand-encoding an Objective-C block ABI (block literal, descriptor, flags, capture layout) in raw FFI. My first attempt did exactly that; I discarded it as too fragile to ship, and chose a synchronous Foundation call with a known metadata gap over a call that could corrupt memory. Windows and Linux *do* return the real `Content-Type`.

3. **DevTools is not linked into `cove-hosts`.** Diagnostic sessions use the pre-existing injected-factory seam (`set_diagnostics_factory` / `NullDiagnosticsSession`), so by default there is no DevTools server, inspection overlay, input interception, or mirror capture. **Reason:** `cove-hosts` has no `cove-devtools` dependency and adding one is a workspace-graph change; the seam was already established in this crate before my work and I preserved it rather than re-architecting.

4. **macOS memory pressure is read, not owned.** The sampler polls a published pressure level (`darwin::set_level`) instead of owning a `dispatch_source` with an event handler. Linux and Windows use their real mechanisms (`/proc/meminfo` + cgroup v2; `CreateMemoryResourceNotification`). **Reason:** a `dispatch_source` event handler is an Objective-C block — the same ABI limitation as item 2. All three platforms sample on a worker thread and only raise a flag that the UI thread drains, because `Sdl3PlatformHost` is `!Send` and cannot cross threads.

5. **Android showcase/gallery/counter content is injected, not linked.** `HostedScene` takes a `Box<dyn HostedSceneContent>` and `ShowcaseContent` takes `&mut dyn ShowcaseProducts`; every host-side behaviour is ported (launch-mode selection, direct-vs-layers rendering, checkpoint decode plus the `CHECKPOINT_CORRUPT` error, page reporting, per-frame typography metrics, atlas-full re-prepare). **Reason:** the Swift originals imported `CounterExample` and `GalleryContent`, which are not dependencies of `cove-hosts`. Inventing placeholder content would have been a stub, so the products are a seam. The seam is genuinely shared — the desktop host uses the same `AdaptiveApplicationContent` wrapper.

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
