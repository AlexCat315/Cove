import CoveUICompose
import CoveUIRuntime
import CovePlatformCore
import Foundation
#if os(macOS) || os(Windows) || os(Linux)
private import Dispatch
#elseif os(iOS)
import UIKit
#endif

public enum AppWindowRendererState: Sendable, Equatable {
    case unconfigured, active, suspended, closed
}

public struct AppWindowMemorySnapshot: Sendable {
    public let state: AppWindowRendererState
    public let samples: UInt32
    public let hasSwapchain: Bool
    /// Owned MSAA, backdrop and effect scratch. Drawable pool/driver memory is excluded.
    public let estimatedTargetBytes: Int
}

public struct AppMemorySnapshot: Sendable {
    public let shaping: TextCacheSnapshot?
    public let textLayout: TextCacheSnapshot?
    public let cachedImageBytes: Int
    public let windows: [WindowID: AppWindowMemorySnapshot]
}

@MainActor
final class ApplicationMemoryPressure {
    #if os(macOS)
    private var source: (any DispatchSourceMemoryPressure)?
    #elseif os(iOS)
    private var observer: (any NSObjectProtocol)?
    #elseif os(Windows) || os(Linux)
    private var source: (any DispatchSourceTimer)?
    private let probe = MemoryPressureProbe()
    #endif

    init(host: SDL3PlatformHost, trim: @escaping @MainActor () -> Void) {
        #if os(macOS)
        let source = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .global())
        source.setEventHandler { [weak host] in
            host?.enqueueMainThreadWork { MainActor.assumeIsolated { trim() } }
        }
        source.resume()
        self.source = source
        #elseif os(iOS)
        observer = NotificationCenter.default.addObserver(forName: UIApplication.didReceiveMemoryWarningNotification,
            object: nil, queue: nil) { [weak host] _ in
                host?.enqueueMainThreadWork { MainActor.assumeIsolated { trim() } }
            }
        #elseif os(Windows) || os(Linux)
        let source = DispatchSource.makeTimerSource(queue: .global())
        source.schedule(deadline: .now(), repeating: .seconds(2))
        let probe = probe
        source.setEventHandler { [weak host] in
            if probe.changedToLowMemory() {
                host?.enqueueMainThreadWork { MainActor.assumeIsolated { trim() } }
            }
        }
        source.resume()
        self.source = source
        #endif
    }

    func stop() {
        #if os(macOS)
        source?.cancel()
        source = nil
        #elseif os(iOS)
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        #elseif os(Windows) || os(Linux)
        source?.cancel()
        source = nil
        probe.close()
        #endif
    }
}
