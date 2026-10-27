import CoveUIRuntime

/// Default light theme.
///
/// Neutral-zinc palette matching the reference design: a pure white window
/// with zinc control faces (the face of buttons, inputs, sidebars and chips),
/// a single blue accent ramp, and subtle borders that separate flat regions
/// rather than translucent state washes.
public enum DefaultLightTheme {
    public static let value: Theme = Theme(
        colors: ColorScheme(
            background:       Color(red: 0xFF, green: 0xFF, blue: 0xFF), // window
            surface:          Color(red: 0xF4, green: 0xF4, blue: 0xF5), // control faces / sidebar
            surfaceVariant:   Color(red: 0xE9, green: 0xE9, blue: 0xEC), // wells / hover / badges
            surfacePressed:   Color(red: 0xDD, green: 0xDD, blue: 0xE1),
            surfaceSunken:    Color(red: 0xFF, green: 0xFF, blue: 0xFF), // island bodies
            surfaceRaised:    Color(red: 0xFF, green: 0xFF, blue: 0xFF),
            surfaceFloating:  Color(red: 0xFF, green: 0xFF, blue: 0xFF), // menus / popovers
            surfaceOverlay:   Color(red: 0xFF, green: 0xFF, blue: 0xFF),

            onBackground:     Color(red: 0x18, green: 0x18, blue: 0x1B),
            onSurface:        Color(red: 0x18, green: 0x18, blue: 0x1B),
            onSurfaceVariant: Color(red: 0x52, green: 0x52, blue: 0x5B),
            onSurfaceMuted:   Color(red: 0x71, green: 0x71, blue: 0x7A),

            accent:           Color(red: 0x25, green: 0x63, blue: 0xEB),
            accentHover:      Color(red: 0x1D, green: 0x4E, blue: 0xD8),
            accentPressed:    Color(red: 0x1E, green: 0x40, blue: 0xAF),
            onAccent:         Color(red: 0xFF, green: 0xFF, blue: 0xFF),
            accentMuted:      Color(red: 0x25, green: 0x63, blue: 0xEB, alpha: 0x24),
            accentSecondary:  Color(red: 0x93, green: 0x33, blue: 0xEA),

            stateLayerHover:    Color(red: 0x18, green: 0x18, blue: 0x1B, alpha: 0x0A), // ~surfaceHover
            stateLayerPressed:  Color(red: 0x18, green: 0x18, blue: 0x1B, alpha: 0x14), // ~surfacePressed
            stateLayerSelected: Color(red: 0x25, green: 0x63, blue: 0xEB, alpha: 0x29),

            success:          Color(red: 0x16, green: 0xA3, blue: 0x4A),
            warning:          Color(red: 0xD9, green: 0x77, blue: 0x06),
            error:            Color(red: 0xDC, green: 0x26, blue: 0x26),
            info:             Color(red: 0x25, green: 0x63, blue: 0xEB),

            border:           Color(red: 0xD9, green: 0xD9, blue: 0xDE),
            borderStrong:     Color(red: 0xA5, green: 0xA5, blue: 0xAE), // border mixed 25% toward text
            divider:          Color(red: 0xE4, green: 0xE4, blue: 0xE7),
            // The outer focus ring is a translucent accent, drawn outside the
            // control's border.
            focusRing:        Color(red: 0x25, green: 0x63, blue: 0xEB, alpha: 0x8C),
            selection:        Color(red: 0x25, green: 0x63, blue: 0xEB, alpha: 0x40),
            overlay:          Color(red: 0x00, green: 0x00, blue: 0x00, alpha: 0x66)
        ),
        typography: DefaultDarkTheme.value.typography,
        spacing:    DefaultDarkTheme.value.spacing,
        radius:     DefaultDarkTheme.value.radius,
        // Not the dark theme's elevation. A black shadow over a near-white
        // canvas has far more contrast to work with than over the dark one, so
        // the same numbers that read as lift there read as dirt here: every
        // component sat in a 24pt halo of ~15% black. Light gets roughly half
        // the alpha at roughly half the spread — enough to lift, not to smudge.
        elevation: ElevationScale(
            none:   .none,
            // Selected segments, small chips: a tight 1px key shadow.
            low:    SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x1F), offsetX: 0, offsetY: 1, blur: 2),
            // Cards: a hairline key shadow.
            medium: SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x0F), offsetX: 0, offsetY: 1, blur: 3),
            // Popups, menus, tooltips: a soft 20px lift.
            high:   SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x2E), offsetX: 0, offsetY: 6, blur: 20),
            // Modals: a wide, faint ambient halo over a tighter directional key.
            floating: LayeredShadow(
                ambient: SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x1A),
                                       offsetX: 0, offsetY: 0, blur: 40),
                key:     SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x4D),
                                       offsetX: 0, offsetY: 10, blur: 16)
            )
        ),
        motion:     DefaultDarkTheme.value.motion
    )
}

public extension Theme {
    /// Project-wide light theme.
    static let defaultLight: Theme = DefaultLightTheme.value
}

