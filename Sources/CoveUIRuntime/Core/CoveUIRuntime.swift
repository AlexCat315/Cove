/// CoveUI Runtime — 占位模块
///
/// 职责：平台层、布局引擎、文字渲染、节点树、recompose 运行时。
/// 详细设计见 docs/cove-ui-blueprint.md。
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
