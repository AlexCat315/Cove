internal import CoveUICompose
internal import CoveUIRuntime

/// Session geometry survives swapchain recreation. Native updates replace
/// device facts; frame preparation replaces only viewport facts.
final class SurfaceState {
    let value: State<UIEnvironment>
    private(set) var systemBarIconStyle: Int32 = -1

    init(scale: Float) {
        var surface = UIEnvironment()
        surface.idiom = .phone
        surface.input = [.touch, .keyboard]
        surface.viewport.width = 0; surface.viewport.height = 0
        surface.viewport.scale = Self.validScale(scale)
        value = State(wrappedValue: surface)
    }

    var scale: Float { value.wrappedValue.viewport.scale }
    var logicalSize: SIMD2<Float> {
        let viewport = value.wrappedValue.viewport
        return SIMD2(viewport.width, viewport.height)
    }

    func update(idiom: UIEnvironment.Idiom, safeArea: UIEnvironment.Insets,
                keyboard: UIEnvironment.Keyboard, textScale: UIEnvironment.TextScale) -> Bool {
        var surface = value.wrappedValue
        surface.idiom = idiom
        surface.safeArea = safeArea.normalized
        surface.keyboard = .init(height: keyboard.height)
        surface.textScale = textScale
        return replace(surface)
    }

    func prepare(pixels: SIMD2<Int>, scale: Float) {
        var surface = value.wrappedValue
        surface.viewport.scale = Self.validScale(scale)
        surface.viewport.width = Float(max(0, pixels.x)) / surface.viewport.scale
        surface.viewport.height = Float(max(0, pixels.y)) / surface.viewport.scale
        _ = replace(surface)
    }

    func updateSystemBars(theme: Theme?) {
        guard let background = theme?.colors.background else { return }
        let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b
        systemBarIconStyle = luminance > 0.5 ? 1 : 0
    }

    private func replace(_ surface: UIEnvironment) -> Bool {
        guard surface != value.wrappedValue else { return false }
        value.wrappedValue = surface
        return true
    }

    private static func validScale(_ value: Float) -> Float { value.isFinite && value > 0 ? value : 1 }
}
