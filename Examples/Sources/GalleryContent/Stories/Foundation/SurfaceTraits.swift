import CoveUI

struct ViewportTraitsStory: View {
    var body: some View {
        SurfaceReader { surface in traits(surface) }
    }

    private func traits(_ surface: UIEnvironment) -> some View {
        StorySection("Viewport traits", "Current host surface.") {
            WidthClassReader { width in
                HeightClassReader { height in
                    OrientationReader { orientation in
                        Column(alignment: .leading, spacing: 10) {
                            Text("Viewport: \(Int(surface.viewport.width)) × \(Int(surface.viewport.height))")
                                .font(.caption).foregroundColor(.onSurfaceMuted)
                                .debugName("viewport-size")
                            Text("Width class: \(Self.name(width))")
                                .font(.body).debugName("viewport-width-class")
                            Text("Height class: \(Self.name(height))")
                                .font(.body).debugName("viewport-height-class")
                            Text("Orientation: \(Self.name(orientation))")
                                .font(.body).debugName("viewport-orientation")
                            Text("Idiom: \(Self.name(surface.idiom))")
                                .font(.body).debugName("viewport-idiom")
                            Text("Safe area: \(Int(surface.safeArea.top)) / \(Int(surface.safeArea.leading)) / \(Int(surface.safeArea.bottom)) / \(Int(surface.safeArea.trailing))")
                                .font(.caption).foregroundColor(.onSurfaceMuted)
                                .debugName("viewport-safe-area")
                            Text("Text scale: \(Int((surface.textScale.factor * 100).rounded()))%   ·   Keyboard: \(surface.keyboard.isVisible ? "visible" : "hidden")")
                                .font(.caption).foregroundColor(.onSurfaceMuted)
                                .debugName("viewport-text-scale")
                            Text("Compact < 600 · Medium 600–999 · Expanded ≥ 1000")
                                .font(.caption).foregroundColor(.onSurfaceMuted)
                        }
                    }
                }
            }
        }
    }

    private static func name(_ idiom: UIEnvironment.Idiom) -> String {
        switch idiom {
        case .phone: "phone"
        case .tablet: "tablet"
        case .desktop: "desktop"
        }
    }

    private static func name(_ width: UIEnvironment.WidthClass) -> String {
        switch width {
        case .compact: "compact"
        case .medium: "medium"
        case .expanded: "expanded"
        }
    }

    private static func name(_ height: UIEnvironment.HeightClass) -> String {
        switch height {
        case .compact: "compact"
        case .medium: "medium"
        case .expanded: "expanded"
        }
    }

    private static func name(_ orientation: UIEnvironment.Orientation) -> String {
        switch orientation {
        case .portrait: "portrait"
        case .landscape: "landscape"
        }
    }
}
