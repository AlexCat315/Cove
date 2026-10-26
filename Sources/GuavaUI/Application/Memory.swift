import GuavaUICompose
import GuavaUIRuntime
import GuavaPlatformCore
import Foundation
#if os(macOS)
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
        #endif
    }

    func stop() {
        #if os(macOS)
        source?.cancel()
        source = nil
        #elseif os(iOS)
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        #endif
    }
}
