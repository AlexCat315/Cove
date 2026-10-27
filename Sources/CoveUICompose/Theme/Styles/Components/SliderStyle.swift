import CoveUIRuntime

// MARK: - Configuration

/// Snapshot of state passed to `SliderStyle.resolve` on every recompose.
public struct SliderStyleConfiguration {
    public let value: Double
    public let range: ClosedRange<Double>
    public let isPressed: Bool
    public let isHovered: Bool
    public let isFocused: Bool
    public let isEnabled: Bool
    public let theme: Theme

    public var fraction: Double {
        let span = range.upperBound - range.lowerBound
        guard span > 0 else { return 0 }
        return min(1, max(0, (value - range.lowerBound) / span))
    }
}

/// Equatable subset of the interaction flags used by built-in styles to key
/// implicit transitions, mirroring `_ButtonInteractionKey`.
public struct _SliderInteractionKey: Equatable, Sendable {
    public let isPressed: Bool
    public let isHovered: Bool
    public let isFocused: Bool
    public let isEnabled: Bool
}

public extension SliderStyleConfiguration {
    var interactionKey: _SliderInteractionKey {
        _SliderInteractionKey(
            isPressed: isPressed,
            isHovered: isHovered,
            isFocused: isFocused,
            isEnabled: isEnabled
        )
    }
}

/// Default transition for built-in `SliderStyle` interaction changes.
public extension Animation {
    static let sliderInteraction = Animation(duration: 0.12, curve: .easeInOut)
}

// MARK: - Appearance

/// Pure-data description of how a slider should be drawn for one frame.
///
/// `SliderStyle` returns this instead of a `View` because the filled-track and
/// thumb positions depend on the host's measured width, which is only known at
/// draw time. The host primitive consumes the appearance and emits the actual
/// quads.
public struct SliderAppearance: Equatable, Sendable {
    public var trackHeight: Float
    public var trackColor: Color
    public var fillColor: Color
    /// Cross-axis knob size (16 in the reference).
    public var thumbDiameter: Float
    /// Along-axis knob size; a capsule knob is longer than it is tall (20×16).
    /// Defaults to `thumbDiameter` for a circular knob.
    public var thumbLength: Float
    public var thumbColor: Color
    public var thumbBorderColor: Color?
    public var thumbBorderWidth: Float
    /// Drop shadow under the knob; `nil` for a flat knob.
    public var thumbShadow: SurfaceShadow?
    public var trackCornerRadius: Float

    public init(trackHeight: Float,
                trackColor: Color,
                fillColor: Color,
                thumbDiameter: Float,
                thumbLength: Float? = nil,
                thumbColor: Color,
                thumbBorderColor: Color? = nil,
                thumbBorderWidth: Float = 0,
                thumbShadow: SurfaceShadow? = nil,
                trackCornerRadius: Float) {
        self.trackHeight = trackHeight
        self.trackColor = trackColor
        self.fillColor = fillColor
        self.thumbDiameter = thumbDiameter
        self.thumbLength = thumbLength ?? thumbDiameter
        self.thumbColor = thumbColor
        self.thumbBorderColor = thumbBorderColor
        self.thumbBorderWidth = thumbBorderWidth
        self.thumbShadow = thumbShadow
        self.trackCornerRadius = trackCornerRadius
    }
}

// MARK: - Protocol

public protocol SliderStyle {
    func resolve(configuration: SliderStyleConfiguration) -> SliderAppearance
}

/// Type-erased `SliderStyle` ferried through the composition tree via
/// `SliderStyleEnvironment`.
public struct AnySliderStyle: @unchecked Sendable {
    public let resolve: (SliderStyleConfiguration) -> SliderAppearance

    public init<S: SliderStyle>(_ style: S) {
        self.resolve = { config in style.resolve(configuration: config) }
    }
}

public enum SliderStyleEnvironment {
    public static let key = CompositionLocal<AnySliderStyle>(
        defaultValue: AnySliderStyle(DefaultSliderStyle())
    )
}

public extension View {
    /// Override the `SliderStyle` used by every `Slider` in this subtree.
    func sliderStyle<S: SliderStyle>(_ style: S) -> some View {
        compositionLocal(SliderStyleEnvironment.key, AnySliderStyle(style))
    }
}

public extension SliderStyle where Self == DefaultSliderStyle {
    static var `default`: DefaultSliderStyle { DefaultSliderStyle() }
}

// MARK: - DefaultSliderStyle

/// Built-in style, matching the reference control: a 6-pt track with a
/// neutral empty half and an accent filled half, and a white 20×16 capsule
/// knob with a small key shadow. The knob stays white; the fill carries the
/// accent, and the track reacts to hover / press.
public struct DefaultSliderStyle: SliderStyle {
    public init() {}

    public func resolve(configuration c: SliderStyleConfiguration) -> SliderAppearance {
        let theme = c.theme
        let track = c.isEnabled ? theme.colors.border : theme.colors.border.multipliedAlpha(0.6)
        let fill: Color = c.isEnabled ? theme.colors.accent
                                      : theme.colors.borderStrong.multipliedAlpha(0.6)
        let knob: Color = c.isEnabled ? .white : Color.white.multipliedAlpha(0.7)
        // The empty track darkens slightly while the knob is held.
        let trackColor = c.isPressed ? theme.colors.borderStrong : track
        return SliderAppearance(
            trackHeight: 6,
            trackColor: trackColor,
            fillColor: fill,
            thumbDiameter: 16,
            thumbLength: 20,
            thumbColor: knob,
            // Keyboard focus draws a 2-px ring around the white capsule knob.
            thumbBorderColor: c.isFocused ? theme.colors.focusRing : nil,
            thumbBorderWidth: c.isFocused ? 2 : 0,
            thumbShadow: SurfaceShadow(color: Color(r: 0, g: 0, b: 0, a: 0.25),
                                      offsetX: 0, offsetY: 1, blur: 3),
            trackCornerRadius: 3
        )
    }
}
