import CoveUIScene
import CoveUIText
import CoveUIImaging

/// A faint upper edge and a shallow shadow: material without glass or gradients.
///
/// For surfaces that sit *in* the chrome — a panel edge, a sidebar. Surfaces
/// that float above it want `floatingSurface()` instead.
public struct SurfaceFinishModifier: ViewModifier {
    public init() {}

    public func apply(node: Node) {
        let shadow = node.theme.elevation.low
        node.shadow = shadow
        node.updateOverlayDraw(identity: node.theme.colors.border) { [weak node] list, origin in
            guard let node else { return }
            let inset = node.cornerRadius
            list.addRect(UIRect(x: Float(origin.x) + inset, y: Float(origin.y),
                                width: max(0, Float(node.frame.width) - inset * 2), height: 1),
                         color: node.theme.colors.border.multipliedAlpha(0.4 * node.opacity))
        }
    }
}

/// A solid panel floating over content — menus, popovers, tooltips, popup
/// editors: opaque fill, a crisp hairline and the two-layer lift that says it
/// is above the chrome.
///
/// Solid on purpose. Labels on a floating panel have to stay legible over
/// anything, and a live backdrop blur samples whatever happens to be behind
/// the panel, which reads as noise on fine content; the edge and the shadow
/// carry the depth instead.
/// The depth of a floating panel.
public enum FloatingSurfaceElevation: Sendable {
    /// Menus and popovers: a 20-pt lift (`elevation.high`).
    case panel
    /// Modals and sheets: the widest, deepest shadow (`elevation.floating`).
    case modal
}

public struct FloatingSurfaceModifier: ViewModifier {
    public let cornerRadius: Float
    public let elevation: FloatingSurfaceElevation

    public init(cornerRadius: Float, elevation: FloatingSurfaceElevation = .modal) {
        self.cornerRadius = max(0, cornerRadius)
        self.elevation = elevation
    }

    public func apply(node: Node) {
        let colors = node.theme.colors
        node.animatableSet(\.backgroundColor, to: colors.surfaceFloating)
        node.animatableSet(\.cornerRadius, to: cornerRadius)
        node.animatableSet(\.borderColor, to: colors.border)
        node.animatableSet(\.borderWidth, to: 1)
        switch elevation {
        case .modal:
            let floating = node.theme.elevation.floating
            node.ambientShadow = floating.ambient
            node.shadow = floating.key
        case .panel:
            node.ambientShadow = SurfaceShadow()
            node.shadow = node.theme.elevation.high
        }
    }
}

public extension View {
    func surfaceFinish() -> some View { modifier(SurfaceFinishModifier()) }

    /// Raise this surface above the chrome as a floating panel.
    func floatingSurface(_ cornerRadius: Float,
                         elevation: FloatingSurfaceElevation = .modal) -> some View {
        modifier(FloatingSurfaceModifier(cornerRadius: cornerRadius, elevation: elevation))
    }

    /// The theme-driven radius, the same way `.cornerRadius(_ token:)` takes one.
    func floatingSurface(_ cornerRadius: ThemeMetric,
                         elevation: FloatingSurfaceElevation = .modal) -> some View {
        ThemeReader { theme in self.floatingSurface(cornerRadius.resolve(theme), elevation: elevation) }
    }
}
