import CovePlatformCore
import Foundation
import CoveUICompose
import CoveUIRuntime
import CoveUIDevTools
import CoveLogging
import NativeRHI
import PlatformShell
#if canImport(UIKit)
import UIKit
#endif

/// 高层应用宿主。隐藏窗口、设备、DrawList 绘制器、
/// `TextEnvironment`、`ViewGraph` 等装配细节。
///
/// 调用方使用方式：
///
/// ```swift
/// try Application.run(config: ApplicationConfiguration(title: "My App")) {
///     RootView()
/// }
/// ```
///
/// 主窗口与辅助窗口共享 UI 资源，各自拥有 presentation target。
/// `backend: device` 可与生产 NativeRenderer 共享设备与 viewport。
@MainActor
public final class Application {

    /// 启动主窗口并阻塞直到窗口关闭。返回值仅用于错误传递。
    ///
    /// `onTick` 会在每一帧 layout 之前调用，用于把外部子系统（例如游戏引擎
    /// `EngineHost`）按 UI 帧率推进。`deltaTime` 是上一帧到这一帧之间的秒数。
    public static func run<Root: View>(config: ApplicationConfiguration = ApplicationConfiguration(),
                                       backend: Device? = nil,
                                       events: PlatformEventBridge = PlatformEventBridge(),
                                       checkpoint: ApplicationCheckpoint? = nil,
                                       onTick: ((_ deltaTime: Double) -> Void)? = nil,
                                       onDisplayReady: ((AppDisplayHandle) -> Void)? = nil,
                                       onDiagnosticsReady: ((UIDiagnostics?) -> Void)? = nil,
                                       @ViewBuilder rootView: @escaping () -> Root) throws {
        try PlatformApplication.run {
            try runHosted(config: config, backend: backend, events: events, checkpoint: checkpoint,
                          onTick: onTick, onDisplayReady: onDisplayReady, onDiagnosticsReady: onDiagnosticsReady, rootView: rootView)
        }
    }

    private static func runHosted<Root: View>(config: ApplicationConfiguration, backend: Device?,
        events: PlatformEventBridge, checkpoint: ApplicationCheckpoint?,
        onTick: ((Double) -> Void)?, onDisplayReady: ((AppDisplayHandle) -> Void)?,
        onDiagnosticsReady: ((UIDiagnostics?) -> Void)?,
        rootView: () -> Root) throws -> Application {
        var logging = config.logging
        try logging.applyEnvironment()
        try LogSystem.bootstrap(application: Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName, configuration: logging)
        defer { LogSystem.flush() }
        LogSystem.logger("com.cove.ui.application").info("Application starting", metadata: ["title": "\(config.window.title)"])
        let runtime = try Application(config: config,
                                 backend: backend,
                                 events: events,
                                 checkpoint: checkpoint,
                                 onTick: onTick,
                                 onDisplayReady: onDisplayReady,
                                 onDiagnosticsReady: onDiagnosticsReady)
        try runtime.start(rootView: rootView())
        return runtime
    }

    private let config: ApplicationConfiguration
    private let callbacks: ApplicationCallbacks
    let checkpoint: ApplicationCheckpoint?
    let recorder = InputRecorder()
    let composition: MainComposition
    let host: SDL3PlatformHost
    private let rendering: AppRenderingContext
    private var windowRenderer: (any AppWindowRenderer)?
    private let mainWindow = MainWindowState()
    private let typography = ApplicationTypography()
    private var auxiliaryWindows: [WindowID: AuxiliaryAppWindow] = [:]
    private let metrics = FrameMetrics()
    private let performanceHUD = PerformanceHUD()
    private var runLoopError: Error?
    private let diagnostics = ApplicationDiagnostics()
    private var memoryPressure: ApplicationMemoryPressure?

    private static let forceVSyncOff = ["off", "0", "false", "no"].contains(
        ProcessInfo.processInfo.environment["COVE_VSYNC"]?.lowercased() ?? "")
    /// `COVE_PREVIEW_DEVICE=phone` opens a desktop build as a phone: a 390×844
    /// window with touch input and the safe areas of a notched phone, so mobile
    /// layouts can be built and checked without a simulator.
    private static let previewsPhone = ProcessInfo.processInfo.environment["COVE_PREVIEW_DEVICE"]?.lowercased() == "phone"
    private static let fpsLogEnabled = ProcessInfo.processInfo.environment["COVE_FPS_LOG"] == "1"
    private static let forceContinuousFrameDrive = ProcessInfo.processInfo.environment["COVEUI_FORCE_CONTINUOUS_FRAMES"] == "1"

    private init(config: ApplicationConfiguration, backend: Device?,
                 events: PlatformEventBridge, checkpoint: ApplicationCheckpoint?,
                 onTick: ((Double) -> Void)?, onDisplayReady: ((AppDisplayHandle) -> Void)?,
                 onDiagnosticsReady: ((UIDiagnostics?) -> Void)?) throws {
        self.config = config
        self.callbacks = ApplicationCallbacks(events: events, tick: onTick, ready: onDisplayReady, diagnosticsReady: onDiagnosticsReady)
        self.checkpoint = checkpoint
        mainWindow.isVSyncEnabled = config.rendering.vsync && !Self.forceVSyncOff
        rendering = try AppRenderingContext(device: backend, config: config)
        let portalStore = PortalStore()
        let input = PlatformInputContext()
        input.addScopedAmbient(PortalStoreAmbient(portalStore))
        // Tips belong to the window too: one tip at a time is a per-window
        // rule, and the warm window that skips the rest delay must not carry
        // over from another window.
        input.addScopedAmbient(TooltipArbitrationAmbient())
        host = SDL3PlatformHost(title: config.window.title,
            mainWindowOptions: WindowOptions(width: Self.previewsPhone ? 390 : max(1, config.window.width),
                                            height: Self.previewsPhone ? 844 : max(1, config.window.height), titleBarStyle: config.window.titleBarStyle.platformStyle,
                                            showsDockIcon: config.window.showsDockIcon), inputContext: input)
        composition = MainComposition(recomposer: host.recomposer, portalStore: portalStore)
        composition.imageAssets.limits = config.memory.images
        composition.drawList.geometryAntialiasing = config.window.rendering.geometryAntialiasing ?? config.rendering.geometryAntialiasing
        configureHostFrameDrive()
    }

    private func configureHostFrameDrive() {
        applyFrameRatePolicy()
    }

    /// Apply the configured frame-drive policy. Also used to restore the policy
    /// after a mirror session forces continuous presentation.
    private func applyFrameRatePolicy() {
        if let targetFrameRate = config.frames.validatedTargetRate {
            host.setTargetFrameRate(targetFrameRate)
            return
        }

        if Self.forceContinuousFrameDrive {
            host.setFrameRateMode(.displayRefresh)
            return
        }

        switch config.frames.policy {
        case .continuous:
            host.setFrameRateMode(.displayRefresh)
        case .eventDriven:
            host.setFrameRateMode(.eventDriven)
        }
    }

    private func start<Root: View>(rootView: Root) throws {
        let services = ApplicationServices()
        defer {
            #if !os(iOS)
            services.restore()
            #endif
        }

        let queryMemory: @MainActor @Sendable () -> UIDiagnosticMemorySnapshot = { [weak self] in
            self?.diagnosticMemorySnapshot() ?? UIDiagnosticMemorySnapshot(counters: [:], windows: [:])
        }
        if let devConfig = config.diagnostics.devTools {
            let dev = DevTools(config: devConfig,
                               tree: composition.tree,
                               invalidationLog: host.invalidationLog,
                               renderTree: composition.graph.renderTree)
            try rendering.attachFrameTap(to: dev)
            dev.stateRegistry = composition.graph.stateRegistry
            dev.memorySnapshot = queryMemory
            configureCheckpoint(dev)
            dev.server.host.execute = { [weak self] operation in
                self?.host.enqueueMainThreadWork {
                    MainActor.assumeIsolated {
                        operation()
                    }
                }
            }
            dev.inputDelivery = { [weak self] event in
                self?.host.mainSession?.injectEvent(event)
            }
            dev.onMirrorStart = { [weak self] in
                guard let self else { return }
                // Force continuous presentation and keep the frame loop driving
                // even when the window is occluded / backgrounded, so the mirror
                // keeps receiving offscreen frames. The swapchain is still
                // released on occlusion (see `onWindowPresentationChanged`), so
                // this does not keep GPU presentation memory resident —
                // background mirroring stays memory-efficient.
                self.host.setFrameRateMode(.displayRefresh)
                if let id = self.host.mainSession?.id {
                    self.host.setWindowKeepRendering(id, true)
                }
            }
            dev.onMirrorStop = { [weak self] in
                guard let self else { return }
                if let id = self.host.mainSession?.id {
                    self.host.setWindowKeepRendering(id, false)
                }
                self.applyFrameRatePolicy()
            }
            dev.server.host.window.resize = { [weak self] size in
                guard let host = self?.host, let window = host.mainWindow else { return "No main window" }
                // A maximized native window can ignore explicit size requests.
                host.restoreWindow(window)
                host.setWindowSize(window, width: Float(size.width), height: Float(size.height))
                host.requestDisplay()
                return nil
            }
            dev.onSelectionChanged = { [weak self] in
                self?.host.requestDisplay()
            }
            do {
                try dev.start()
                self.diagnostics.devTools = dev
            } catch {
                LogSystem.logger("com.cove.ui.app").warning("DevTools server failed to start: \(error)")
            }
        }

        if callbacks.diagnosticsReady != nil {
            diagnostics.queries = UIDiagnostics(tree: composition.tree, state: composition.graph.stateRegistry)
            diagnostics.queries?.memory = queryMemory
            callbacks.diagnosticsReady?(diagnostics.queries)
        }

        ImageAssetRegistryHolder.current = composition.imageAssets
        AssetDropRegistryHolder.current = composition.assetDropRegistry
        UIWorkSchedulerHolder.enqueue = { [weak host] work in
            host?.enqueueMainThreadWork(work)
        }

        // 把进程级 holder 接到主窗口的 input context 上，使 Compose / Workspace
        // primitives 能直接读到 interaction / focus /
        // pointer-capture / clipboard。
        InteractionRegistryHolder.current = host.interactions
        FocusChainHolder.current = host.focusChain
        PointerCaptureHolder.current = host.pointerCapture
        ClipboardHolder.read = { SDL3Clipboard.read() }
        ClipboardHolder.write = { SDL3Clipboard.write($0) }
        #if os(macOS)
        // AppKit presents native menus; other hosts use Compose menus.
        NativeMenuPresenter.present = { snapshot, point in
            NativeMenuPopover.popUp(snapshot, at: point)
        }
        #endif

        host.onInit = { [weak self] native, w, h in
            guard let self else { return }
            self.host.mainSession?.inputInterceptor = { [weak self] event in
                MainActor.assumeIsolated { self?.diagnostics.devTools?.interceptInput(event) ?? false }
            }
            if self.checkpoint != nil {
                self.host.mainSession?.inputObserver = { [weak self] event in
                    MainActor.assumeIsolated { self?.recorder.record(event) }
                }
            }
            do {
                try self.handleInit(native: native, widthPx: w, heightPx: h, rootView: rootView)
            } catch {
                self.failRunLoop(error, context: "main window initialization")
            }
        }
        host.onResize = { [weak self] w, h in
            guard let self else { return }
            do {
                try self.handleResize(widthPx: w, heightPx: h)
            } catch {
                self.failRunLoop(error, context: "main window resize")
            }
        }
        host.onEvent = { [weak self] event in
            self?.callbacks.events.publish(event)
        }
        host.onBeforeCommit = { [weak self] deltaTime in
            self?.handleFramePreparation(deltaTime: deltaTime)
        }
        host.onFrame = { [weak self] _ in
            self?.handleFrame() ?? false
        }
        host.onWindowTeardown = { [weak self] windowID in
            self?.releaseGPUSurface(for: windowID)
        }
        host.onWindowPresentationChanged = { [weak self] windowID, active in
            guard let self else { return }
            do {
                if self.host.mainSession?.id == windowID {
                    try self.windowRenderer?.setPresentationActive(active)
                    self.callbacks.events.publish(active ? .windowExposed : .windowOccluded)
                    // When the window goes off-screen we already release the
                    // swapchain / MSAA / framebuffer. Also drop the heavy CPU
                    // caches (glyph atlas, image assets, render tree) so
                    // backgrounding actually reclaims memory. Skip this while
                    // mirroring so the offscreen capture path keeps its shared
                    // texture uploads warm.
                    if !active, !(self.diagnostics.devTools?.mirrorIsActive ?? false) {
                        self.trimMemory()
                    }
                } else {
                    try self.auxiliaryWindows[windowID]?.setPresentationActive(active)
                }
            } catch {
                self.failRunLoop(error, context: "window presentation transition")
            }
        }
        host.onTeardown = { [weak self] in
            self?.memoryPressure?.stop()
            self?.memoryPressure = nil
            self?.callbacks.diagnosticsReady?(nil)
            self?.diagnostics.queries?.resetCaptures()
            self?.diagnostics.queries = nil
            self?.diagnostics.devTools?.stop()
            self?.diagnostics.devTools = nil
            self?.releaseGPUSurfaces()
            #if os(iOS)
            services.restore()
            #endif
        }

        let displayHandle = AppDisplayHandle { [weak host] in
            host?.wakeEventLoop()
        }
        AppDisplayHandleHolder.current = displayHandle
        displayHandle.installMemoryControls(snapshot: { [weak self] in self?.memorySnapshot() },
            trim: { [weak self] in self?.trimMemory() })
        memoryPressure = ApplicationMemoryPressure(host: host) { [weak self] in self?.trimMemory() }
        host.externalDisplayRequestDrain = {
            displayHandle.drainDisplayRequest()
        }
        displayHandle.installFolderDialogControl { [weak self] defaultPath, accept in
            guard let self else { accept(nil); return }
            self.host.showOpenFolderDialog(windowID: self.host.mainSession?.id,
                                           defaultPath: defaultPath,
                                           accept: accept)
        }
        displayHandle.installFileDialogControl { [weak self] defaultPath, filters, allowsMultiple, accept in
            guard let self else { accept([]); return }
            self.host.showOpenFileDialog(windowID: self.host.mainSession?.id,
                                         defaultPath: defaultPath,
                                         filters: filters,
                                         allowsMultiple: allowsMultiple,
                                         accept: accept)
        }
        displayHandle.installCloseControls(
            setCloseInterceptor: { [weak self] handler in
                self?.host.windowCloseInterceptor = handler
            },
            quit: { [weak self] in
                self?.host.stop()
            },
            mainWindowID: { [weak self] in
                self?.host.mainSession?.id
            }
        )
        displayHandle.installAuxiliaryWindowControls(
            open: { [weak self] request in
                self?.openAuxiliaryWindow(request)
            },
            close: { [weak self] windowID in
                self?.closeAuxiliaryWindow(windowID)
            },
            isOpen: { [weak self] windowID in
                self?.isAuxiliaryWindowOpen(windowID) ?? false
            }
        )
        displayHandle.installPresentationControls(PresentationControls(
            setRuntimeTargetFrameRate: { [weak self] framesPerSecond in
                self?.host.setTargetFrameRate(framesPerSecond)
            },
            setRuntimeFrameRateMode: { [weak self] mode in
                self?.host.setFrameRateMode(mode)
            },
            setRuntimeVSyncEnabled: { [weak self] enabled in
                self?.setVSyncEnabled(enabled)
            },
            currentRuntimeDisplayRefreshRate: { [weak self] in
                self?.host.currentDisplayRefreshRate()
            }
        ))
        displayHandle.installMenuControl { NativeMenuInstaller.install($0) }
        displayHandle.installWindowControls(main: MainWindowControls(
            minimizeMainWindowAction: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.minimizeWindow(id)
            },
            maximizeMainWindowAction: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.maximizeWindow(id)
            },
            restoreMainWindowAction: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.restoreWindow(id)
            },
            hideMainWindowAction: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.hideWindow(id)
            },
            showMainWindowAction: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.showWindow(id)
            },
            closeMainWindowAction: { [weak self] in
                guard let self, let id = self.host.mainSession?.id else { return }
                self.host.closeWindow(id)
            },
            mainWindowMaximizedQuery: { [weak self] in
                guard let id = self?.host.mainSession?.id else { return false }
                return self?.host.isWindowMaximized(id) ?? false
            },
            setMainWindowChromeHitTestAction: { [weak self] hitTest in
                guard let id = self?.host.mainSession?.id else { return }
                self?.host.setWindowChromeHitTest(id, hitTest)
            }
        ), windows: WindowControls(
            minimizeWindowByIDAction: { [weak self] id in
                self?.host.minimizeWindow(id)
            },
            maximizeWindowByIDAction: { [weak self] id in
                self?.host.maximizeWindow(id)
            },
            restoreWindowByIDAction: { [weak self] id in
                self?.host.restoreWindow(id)
            },
            closeWindowByIDAction: { [weak self] id in
                self?.host.closeWindow(id)
                self?.auxiliaryWindows.removeValue(forKey: id)
            },
            windowMaximizedByIDQuery: { [weak self] id in
                self?.host.isWindowMaximized(id) ?? false
            },
            showWindowSystemMenuAction: { [weak self] id, x, y in
                self?.host.showWindowSystemMenu(id, x: x, y: y)
            }
        ))
        callbacks.ready?(displayHandle)
        host.run(tree: composition.tree)
        if let runLoopError { throw runLoopError }
    }

    // MARK: - Lifecycle callbacks

    private func failRunLoop(_ error: Error, context: String) {
        guard runLoopError == nil else { return }
        runLoopError = error
        LogSystem.logger("com.cove.ui.app").error("\(context) failed: \(error)")
        host.stop()
    }

    /// Release every GPU surface (main window + auxiliary windows) before the host
    /// shell destroys its windows and quits SDL. A swapchain built from a
    /// Wayland window holds the `wl_display`/`wl_surface`; releasing it after
    /// `SDL_Quit` has disconnected Wayland dereferences freed proxies and crashes
    /// in libwayland-client. Wired to `host.onTeardown`, which fires after the run
    /// loop exits but before `shell.shutdown()`.
    private func memorySnapshot() -> AppMemorySnapshot {
        var windows = auxiliaryWindows.compactMapValues { $0.memorySnapshot }
        if let id = host.mainSession?.id, let renderer = windowRenderer { windows[id] = renderer.memorySnapshot }
        let text = TextEnvironmentHolder.current
        return AppMemorySnapshot(shaping: text?.shapingCacheSnapshot, textLayout: text?.layoutCacheSnapshot,
            cachedImageBytes: composition.imageAssets.cachedBytes, windows: windows)
    }

    private func diagnosticMemorySnapshot() -> UIDiagnosticMemorySnapshot {
        let value = memorySnapshot()
        return UIDiagnosticMemorySnapshot(counters: [
            "shapingEntries": value.shaping?.entries ?? 0,
            "shapingBytes": value.shaping?.estimatedBytes ?? 0,
            "layoutEntries": value.textLayout?.entries ?? 0,
            "layoutBytes": value.textLayout?.estimatedBytes ?? 0,
            "imageEntries": composition.imageAssets.cachedCount,
            "imageBytes": value.cachedImageBytes,
            "skippedParentUpdates": composition.graph.skippedParentUpdates
        ], windows: Dictionary(uniqueKeysWithValues: value.windows.map { id, window in
            var counters = ["samples": Int(window.samples),
                "hasSwapchain": window.hasSwapchain ? 1 : 0,
                "suspended": window.state == .suspended ? 1 : 0,
                "targetBytes": window.estimatedTargetBytes,
                "msaaLogicalBytes": window.msaaLogicalBytes,
                "msaaMemoryless": window.msaaMemoryless ? 1 : 0]
            if let allocated = window.msaaAllocatedBytes { counters["msaaAllocatedBytes"] = allocated }
            return (String(describing: id), counters)
        }))
    }

    private func trimMemory() {
        TextEnvironmentHolder.current?.trimTextCaches()
        composition.imageAssets.clear()
        composition.graph.renderTree.trimCaches()
        composition.drawList.reset()
        for window in auxiliaryWindows.values { window.trimMemory() }
    }

    private func releaseGPUSurfaces() {
        releaseMainWindowRenderer()
        for window in auxiliaryWindows.values { window.releaseGPUSurface() }
    }

    private func releaseMainWindowRenderer() {
        do { try windowRenderer?.close() }
        catch {
            runLoopError = runLoopError ?? error
            LogSystem.logger("com.cove.ui.app").error("Main window GPU cleanup failed: \(error)")
        }
        windowRenderer = nil
    }

    private func releaseGPUSurface(for windowID: WindowID) {
        if host.mainSession?.id == windowID { releaseMainWindowRenderer() }
        else { auxiliaryWindows[windowID]?.releaseGPUSurface() }
    }

    private func handleInit<Root: View>(native: NativeRenderSurface,
                                        widthPx: UInt32,
                                        heightPx: UInt32,
                                        rootView: Root) throws {
        mainWindow.drawableW = widthPx
        mainWindow.drawableH = heightPx
        mainWindow.logicalW = host.logicalSize.width
        mainWindow.logicalH = host.logicalSize.height

        let renderer = try rendering.makeWindow(settings: AppWindowRenderSettings(config: config, window: config.window.rendering))
        try renderer.configure(native: native, size: SIMD2(widthPx, heightPx), vsync: mainWindow.isVSyncEnabled)
        windowRenderer = renderer

        updateSurfaceEnvironment()
        configureTextEnvironment(scale: host.contentScaleFactor)

        if !mainWindow.didInstallRoot {
            withWindowChromeContext(host.mainSession?.id) {
                composition.graph.install(root: AdaptiveApplicationContent(content: rootView, environment: composition.environment)
                    .textScaler(config.typography.scaler)
                    // One selection for the whole window: every label is copyable
                    // without each screen opting in. A subtree that should stay
                    // inert opts out with `.textSelection(false)`. The local is
                    // applied outside it so the selection menu can reach the portal.
                    .textSelection()
                    .compositionLocal(PortalStoreEnvironment.key, composition.portalStore))
            }
            composition.graph.computeLayout(width: Float(mainWindow.logicalW), height: Float(mainWindow.logicalH))
            syncMainWindowChromeHitTest()
            mainWindow.didInstallRoot = true
        }

        try uploadAtlasIfNeeded()
        mainWindow.lastFrameTime = TimingTrace.now()

        // Request an initial frame to ensure surfaces like viewport have
        // time to initialize before the first render.
        host.requestDisplay()
    }

    private func handleResize(widthPx: UInt32, heightPx: UInt32) throws {
        mainWindow.drawableW = widthPx
        mainWindow.drawableH = heightPx
        mainWindow.logicalW = host.logicalSize.width
        mainWindow.logicalH = host.logicalSize.height
        try windowRenderer?.resize(size: SIMD2(widthPx, heightPx), vsync: mainWindow.isVSyncEnabled)
        updateSurfaceEnvironment()
        configureTextEnvironment(scale: host.contentScaleFactor)
        try uploadAtlasIfNeeded()
    }

    private func setVSyncEnabled(_ enabled: Bool) {
        let enabled = enabled && !Self.forceVSyncOff
        let changed = mainWindow.isVSyncEnabled != enabled
        if changed {
            mainWindow.isVSyncEnabled = enabled
            reconfigureMainSurfaceForCurrentPresentMode()
            for window in auxiliaryWindows.values {
                window.setVSyncEnabled(mainWindow.isVSyncEnabled)
            }
        }
        // VSync controls the surface present mode only. Frame cadence is owned
        // by `ApplicationConfiguration.frameDrivePolicy` or an explicit runtime frame-rate
        // override; tying it to VSync makes input/update pacing change when the
        // user only asked to change presentation.
        host.requestDisplay()
    }

    private func reconfigureMainSurfaceForCurrentPresentMode() {
        do { try windowRenderer?.resize(size: SIMD2(mainWindow.drawableW, mainWindow.drawableH), vsync: mainWindow.isVSyncEnabled) }
        catch { LogSystem.logger("com.cove.ui.app").warning("Main window present mode update failed: \(error)") }
    }

    private func syncMainWindowChromeHitTest() {
        guard let root = composition.tree.root,
              let id = host.mainSession?.id else { return }

        guard let next = mainWindow.mainWindowChromeHitTestCollector.collect(root: root) else {
            if mainWindow.lastMainWindowChromeHitTest != nil {
                host.setWindowChromeHitTest(id, nil)
                mainWindow.lastMainWindowChromeHitTest = nil
            }
            return
        }

        guard next != mainWindow.lastMainWindowChromeHitTest else { return }
        host.setWindowChromeHitTest(id, next)
        mainWindow.lastMainWindowChromeHitTest = next
    }

    private func handleFrame() -> Bool {
        guard let windowRenderer, windowRenderer.isConfigured, let root = composition.tree.root else { return false }

        let frameStart = TimingTrace.now()

        // Everything between the layout span and the draw span used to be
        // unattributed frame time: it landed in `totalMs` but in no timeline
        // phase, so a slow environment / chrome / accessibility pass looked
        // like slow rendering. Each gets its own span.
        let environmentTrace = composition.tree.timeline.begin()
        updateSurfaceEnvironment()
        configureTextEnvironment(scale: host.contentScaleFactor)
        composition.tree.timeline.end(environmentTrace, phase: "frame", name: "Surface + text environment")

        let layoutStart = TimingTrace.now()
        _ = composition.graph.computeLayoutIfNeeded(width: Float(mainWindow.logicalW), height: Float(mainWindow.logicalH))

        let chromeTrace = composition.tree.timeline.begin()
        syncMainWindowChromeHitTest()
        composition.tree.timeline.end(chromeTrace, phase: "frame", name: "Window chrome hit test")

        if let session = host.mainSession {
            let accessibilityTrace = composition.tree.timeline.begin()
            NativeAccessibility.synchronize(host: host, session: session, root: root,
                                            timeline: composition.tree.timeline)
            composition.tree.timeline.end(accessibilityTrace, phase: "frame", name: "Accessibility sync")
        }
        let layoutEnd = TimingTrace.now()

        composition.drawList.reset()
        // Edges are snapped to the physical pixel grid as they are emitted; see
        // `DrawList.pixelScale`. Set before drawing, not at record time.
        composition.drawList.pixelScale = Float(host.contentScaleFactor)
        let drawTrace = composition.tree.timeline.begin()
        if composition.useLegacyRenderer {
            composition.nodeRenderer.render(root: root, into: composition.drawList)
        } else {
            composition.layerRenderer.render(tree: composition.graph.renderTree, into: composition.drawList)
        }
        if config.diagnostics.showsPerformanceHUD { performanceHUD.draw(into: composition.drawList) }
        drawDevToolsOverlay(into: composition.drawList)
        let drawEnd = TimingTrace.now()
        composition.tree.timeline.end(drawTrace, phase: "draw", name: "Encode draw list")

        // One walk per frame rather than one per reporting branch, and
        // memoised on the root's structural version, so a steady frame is O(1).
        let countTrace = composition.tree.timeline.begin()
        let frameNodeCount = composition.tree.nodeCount
        composition.tree.timeline.end(countTrace, phase: "frame", name: "Node count")

        do {
            if typography.atlas?.isDirty == true {
                try uploadAtlasIfNeeded(force: true)
            }
        } catch {
            failRunLoop(error, context: "Font atlas upload")
            return false
        }

        if let dev = diagnostics.devTools {
            if root.isDirty || root.renderDirty {
                dev.notifyTreeChanged()
            }
        }

        do {
            guard try windowRenderer.draw(list: composition.drawList, logical: SIMD2(Float(mainWindow.logicalW), Float(mainWindow.logicalH))) else {
                diagnostics.devTools?.mirrorCapture(drawList: composition.drawList, widthPx: mainWindow.drawableW, heightPx: mainWindow.drawableH,
                    logical: (Float(mainWindow.logicalW), Float(mainWindow.logicalH)))
                let frameEnd = TimingTrace.now()
                diagnostics.devTools?.timing.record(layoutMs: (layoutEnd - layoutStart) * 1000,
                    drawMs: (drawEnd - layoutEnd) * 1000, presentMs: 0,
                    totalMs: (frameEnd - frameStart) * 1000, nodeCount: frameNodeCount,
                    batchCount: composition.drawList.batches.count, presented: false)
                diagnostics.devTools?.notifyFrameFinished()
                host.requestDisplay()
                return false
            }
            if let dev = diagnostics.devTools {
                // Keep synchronous mirror readback behind the primary
                // swapchain submission. Doing it before acquisition can make
                // the native surface unavailable on subsequent frames.
                dev.mirrorCapture(
                    drawList: composition.drawList,
                    widthPx: mainWindow.drawableW,
                    heightPx: mainWindow.drawableH,
                    logical: (Float(mainWindow.logicalW), Float(mainWindow.logicalH))
                )
            }
            if config.frames.policy == .continuous || Self.forceContinuousFrameDrive {
                // Legacy behavior: keep re-rendering at display rate. Under
                // `.eventDriven` the loop's needsDisplay machinery (input,
                // recompose, markRenderDirty, animations, requestDisplay)
                // schedules the next frame instead.
                host.requestDisplay()
            }
            performanceHUD.presented()
            let presentEnd = TimingTrace.now()

            if Self.fpsLogEnabled {
                metrics.record(layoutMs: (layoutEnd - layoutStart) * 1000,
                                drawMs: (drawEnd - layoutEnd) * 1000,
                                presentMs: (presentEnd - drawEnd) * 1000)
            }

            if let dev = diagnostics.devTools {
                dev.timing.record(
                    layoutMs: (layoutEnd - layoutStart) * 1000,
                    drawMs: (drawEnd - layoutEnd) * 1000,
                    presentMs: (presentEnd - drawEnd) * 1000,
                    totalMs: (presentEnd - frameStart) * 1000,
                    nodeCount: frameNodeCount,
                    batchCount: composition.drawList.batches.count
                )
                dev.notifyFrameFinished()
                // Mirror needs a steady frame stream even when the UI is
                // idle. Force the next frame so FrameTap keeps producing.
                if dev.mirrorIsActive {
                    host.requestDisplay()
                }
            }
            return true
        } catch {
            failRunLoop(error, context: "Frame rendering")
            return false
        }
    }

    private func handleFramePreparation(deltaTime: Double) {
        performanceHUD.tick()
        let delta = max(0, deltaTime)
        mainWindow.lastFrameTime = TimingTrace.now()
        let tickStart = TimingTrace.now()
        callbacks.tick?(delta)
        if Self.fpsLogEnabled {
            metrics.fpsTickAccum += (TimingTrace.now() - tickStart) * 1000
        }
        syncAuxiliaryWindows()
    }

    private func drawDevToolsOverlay(into list: DrawList) {
        diagnostics.devTools?.drawInspectionOverlay(into: list)
    }

    // MARK: - Text environment
    private func updateSurfaceEnvironment() {
        var environment = UIEnvironment()
        environment.viewport.width = Float(host.logicalSize.width)
        environment.viewport.height = Float(host.logicalSize.height)
        environment.viewport.scale = host.contentScaleFactor
        let insets = host.safeArea
        environment.safeArea.top = insets.top
        environment.safeArea.bottom = insets.bottom
        environment.safeArea.leading = insets.leading
        environment.safeArea.trailing = insets.trailing
        #if os(iOS)
        environment.input = [.touch, .keyboard]
        environment.idiom = UIDevice.current.userInterfaceIdiom == .pad ? .tablet : .phone
        // UIFontMetrics is the platform's own accessibility ramp; one sample
        // keeps the whole point-based typography scale in proportion.
        let reference: CGFloat = 17
        environment.textScale = .init(Float(UIFontMetrics.default.scaledValue(for: reference) / reference))
        #elseif os(Android)
        environment.input = [.touch, .keyboard]
        #else
        if Self.previewsPhone {
            environment.input = [.touch, .keyboard]
            environment.idiom = .phone
            environment.safeArea.top = max(insets.top, 47)
            environment.safeArea.bottom = max(insets.bottom, 34)
        } else {
            environment.idiom = .desktop
        }
        #endif
        if environment != composition.environment.wrappedValue { composition.environment.wrappedValue = environment }
    }

    private func configureTextEnvironment(scale requestedScale: Float) {
        let scale = max(1, requestedScale)
        guard typography.atlas == nil || abs(scale - typography.scale) >= 0.01 else { return }
        typography.scale = scale

        let atlasEdge = max(1024, Int((1024 * scale).rounded(.up)))
        let env = TextEnvironment.bootstrapped(
            atlasTextureID: typography.textureID,
            primaryFontName: config.typography.primaryFontName,
            defaultFont: Font.system(size: config.typography.fontSize),
            defaultLineHeight: config.typography.lineHeight,
            defaultColor: .white,
            rasterScale: scale,
            atlasEdge: atlasEdge,
            cacheLimits: config.memory.text
        )
        typography.atlas = env.atlas
        TextEnvironmentHolder.current = env
        ContentScaleHolder.current = scale
    }

    private func uploadAtlasIfNeeded(force: Bool = false) throws {
        guard let fontAtlas = typography.atlas, force || fontAtlas.isDirty else { return }
        try rendering.uploadFontAtlas(fontAtlas, textureID: typography.textureID)
    }

    private func openAuxiliaryWindow(_ request: AppAuxiliaryWindowRequest) -> WindowID? {
        syncAuxiliaryWindows()

        do {
            let windowTree = NodeTree()
            let recomposer = Recomposer()
            let portalStore = PortalStore()
            let inputContext = PlatformInputContext()
            inputContext.addScopedAmbient(PortalStoreAmbient(portalStore))
            inputContext.addScopedAmbient(TooltipArbitrationAmbient())
            let session = try host.openWindow(
                title: request.title,
                tree: windowTree,
                recomposer: recomposer,
                inputContext: inputContext,
                options: WindowOptions(width: request.width,
                                       height: request.height,
                                       titleBarStyle: config.window.titleBarStyle.platformStyle)
            )
            let window = AuxiliaryAppWindow(session: session,
                                            rootView: request.rootView,
                                            portalStore: portalStore,
                                            rendering: rendering,
                                            config: config,
                                            windowRendering: request.rendering,
                                            isVSyncEnabled: mainWindow.isVSyncEnabled,
                                            useLegacyRenderer: composition.useLegacyRenderer,
                                            setWindowChromeHitTest: { [weak self] id, hitTest in
                                                self?.host.setWindowChromeHitTest(id, hitTest)
                                            })
            auxiliaryWindows[session.id] = window
            let windowID = session.id

            session.onInit = { [weak self, weak window] native, widthPx, heightPx in
                guard let self, let window else { return }
                do {
                    try window.handleInit(
                        native: native,
                        widthPx: widthPx,
                        heightPx: heightPx,
                        configureTextEnvironment: { scale in
                            self.configureTextEnvironment(scale: scale)
                        },
                        uploadAtlasIfNeeded: { force in
                            try self.uploadAtlasIfNeeded(force: force)
                        }
                    )
                } catch {
                    self.failAuxiliaryWindow(windowID, error: error, context: "initialization")
                }
            }
            session.onResize = { [weak self, weak window] widthPx, heightPx in
                guard let self, let window else { return }
                do {
                    try window.handleResize(
                        widthPx: widthPx,
                        heightPx: heightPx,
                        configureTextEnvironment: { scale in
                            self.configureTextEnvironment(scale: scale)
                        },
                        uploadAtlasIfNeeded: { force in
                            try self.uploadAtlasIfNeeded(force: force)
                        }
                    )
                } catch {
                    self.failAuxiliaryWindow(windowID, error: error, context: "resize")
                }
            }
            session.onFrame = { [weak self, weak window] _ in
                guard let self, let window else { return false }
                return window.handleFrame(
                    host: self.host,
                    configureTextEnvironment: { scale in
                        self.configureTextEnvironment(scale: scale)
                    },
                    uploadAtlasIfNeeded: { force in
                        try self.uploadAtlasIfNeeded(force: force)
                    }
                )
            }
            session.requestDisplay()
            return session.id
        } catch {
            LogSystem.logger("com.cove.ui.app").warning("Auxiliary window open failed: \(error)")
            return nil
        }
    }

    private func closeAuxiliaryWindow(_ windowID: WindowID) {
        host.closeWindow(windowID)
        auxiliaryWindows.removeValue(forKey: windowID)
    }

    private func failAuxiliaryWindow(_ windowID: WindowID,
                                     error: Error,
                                     context: String) {
        LogSystem.logger("com.cove.ui.app")
            .error("auxiliary window \(context) failed: \(error)")
        host.enqueueMainThreadWork { [weak self] in
            self?.closeAuxiliaryWindow(windowID)
        }
    }

    private func isAuxiliaryWindowOpen(_ windowID: WindowID) -> Bool {
        syncAuxiliaryWindows()
        return auxiliaryWindows[windowID] != nil && host.session(for: windowID) != nil
    }

    private func syncAuxiliaryWindows() {
        let liveWindowIDs = Set(host.windowIDs)
        auxiliaryWindows = auxiliaryWindows.filter { liveWindowIDs.contains($0.key) }
    }
}

