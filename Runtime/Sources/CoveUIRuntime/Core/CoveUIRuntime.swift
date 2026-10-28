import CoveUIScene
import CoveUIGraphics
import CoveUIImaging
import CoveUIText
/// Native application assembly and frame scheduling.
@_exported import Foundation
#if os(Android)
@_exported import Android
#endif
@_exported import CoveUICore

// Preserve the desktop API while using the same implementations on Wasm.
typealias UIShader = CoveUICore.UIShader

public enum CoveUIRuntime {
    public static let version = "0.1.0-wip"
}
