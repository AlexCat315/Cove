import CoveUIRuntime
import CoveUIDevTools

@MainActor
struct ApplicationCallbacks {
    let events: PlatformEventBridge
    let tick: ((Double) -> Void)?
    let ready: ((AppDisplayHandle) -> Void)?
    let diagnosticsReady: ((UIDiagnostics?) -> Void)?
}
