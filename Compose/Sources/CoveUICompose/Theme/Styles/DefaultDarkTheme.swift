import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Default dark theme bundled with CoveUI.
///
/// Neutral-zinc palette matching the reference design: a zinc-900 window,
/// zinc-800 control faces, a single blue accent ramp (lighter on hover than
/// at rest, as the reference), and flat zinc borders.
public enum DefaultDarkTheme {
    public static let value: Theme = Theme(
        colors: ColorScheme(
            background:       Color(red: 0x18, green: 0x18, blue: 0x1B),
            surface:          Color(red: 0x27, green: 0x27, blue: 0x2A),
            surfaceVariant:   Color(red: 0x32, green: 0x32, blue: 0x36),
            surfacePressed:   Color(red: 0x3C, green: 0x3C, blue: 0x41),
            surfaceSunken:    Color(red: 0x18, green: 0x18, blue: 0x1B),
            surfaceRaised:    Color(red: 0x1D, green: 0x1D, blue: 0x20),
            surfaceFloating:  Color(red: 0x18, green: 0x18, blue: 0x1B),
            surfaceOverlay:   Color(red: 0x18, green: 0x18, blue: 0x1B),

            onBackground:     Color(red: 0xF4, green: 0xF4, blue: 0xF5),
            onSurface:        Color(red: 0xF4, green: 0xF4, blue: 0xF5),
            onSurfaceVariant: Color(red: 0xA1, green: 0xA1, blue: 0xAA),
            onSurfaceMuted:   Color(red: 0xA1, green: 0xA1, blue: 0xAA),

            accent:           Color(red: 0x3B, green: 0x82, blue: 0xF6),
            accentHover:      Color(red: 0x60, green: 0xA5, blue: 0xFA),
            accentPressed:    Color(red: 0x25, green: 0x63, blue: 0xEB),
            onAccent:         Color(red: 0xFF, green: 0xFF, blue: 0xFF),
            accentMuted:      Color(red: 0x3B, green: 0x82, blue: 0xF6, alpha: 0x33),
            accentSecondary:  Color(red: 0xA8, green: 0x55, blue: 0xF7),

            stateLayerHover:    Color(red: 0xFF, green: 0xFF, blue: 0xFF, alpha: 0x0F),
            stateLayerPressed:  Color(red: 0xFF, green: 0xFF, blue: 0xFF, alpha: 0x1A),
            stateLayerSelected: Color(red: 0x3B, green: 0x82, blue: 0xF6, alpha: 0x38),

            success:          Color(red: 0x22, green: 0xC5, blue: 0x5E),
            warning:          Color(red: 0xF5, green: 0x9E, blue: 0x0B),
            error:            Color(red: 0xEF, green: 0x44, blue: 0x44),
            info:             Color(red: 0x3B, green: 0x82, blue: 0xF6),

            border:           Color(red: 0x3F, green: 0x3F, blue: 0x46),
            borderStrong:     Color(red: 0x6C, green: 0x6C, blue: 0x72), // border mixed 25% toward text
            divider:          Color(red: 0x27, green: 0x27, blue: 0x2A),
            focusRing:        Color(red: 0x60, green: 0xA5, blue: 0xFA, alpha: 0x99),
            selection:        Color(red: 0x3B, green: 0x82, blue: 0xF6, alpha: 0x66),
            overlay:          Color(red: 0x00, green: 0x00, blue: 0x00, alpha: 0x66)
        ),
        typography: Typography(
            display:    TextStyleToken(font: .system(size: 32, weight: .bold), lineHeight: 38, letterSpacing: -0.6),
            title:      TextStyleToken(font: .system(size: 24, weight: .bold), lineHeight: 30, letterSpacing: -0.4),
            headline:   TextStyleToken(font: .system(size: 17, weight: .semibold), lineHeight: 23, letterSpacing: -0.15),
            body:       TextStyleToken(font: .system(size: 14), lineHeight: 20),
            bodyStrong: TextStyleToken(font: .system(size: 14, weight: .semibold), lineHeight: 20),
            caption:    TextStyleToken(font: .system(size: 12), lineHeight: 17),
            label:      TextStyleToken(font: .system(size: 13, weight: .medium), lineHeight: 18),
            mono:       TextStyleToken(font: .monospaced(size: 13), lineHeight: 20)
        ),
        spacing:   SpacingScale(xs: 4, sm: 6, md: 8, lg: 12, xl: 16, xxl: 24),
        // Radii follow the reference design's 4px spacing unit: check boxes 4,
        // controls 6, popups and menus 8, modal dialogs 10.
        radius:    RadiusScale(none: 0, sm: 4, md: 6, lg: 8, xl: 10, pill: 9999),
        elevation: ElevationScale(
            none:   .none,
            // Selected segments, small chips: a tight 1px key shadow.
            low:    SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x4D), offsetX: 0, offsetY: 1, blur: 2),
            // Cards: a hairline key shadow.
            medium: SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x33), offsetX: 0, offsetY: 1, blur: 3),
            // Popups, menus, tooltips: a tight, hard 20px lift matching the
            // reference's single 18% panel shadow rather than a soft halo.
            high:   SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x2E), offsetX: 0, offsetY: 6, blur: 20),
            // Modals: a wide, faint ambient halo over a tighter directional key.
            floating: LayeredShadow(
                ambient: SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x66),
                                       offsetX: 0, offsetY: 0, blur: 40),
                key:     SurfaceShadow(color: Color(red: 0, green: 0, blue: 0, alpha: 0x8C),
                                       offsetX: 0, offsetY: 10, blur: 16)
            )
        ),
        motion: MotionScale(
            fast:           .milliseconds(80),
            standard:       .milliseconds(180),
            slow:           .milliseconds(320),
            emphasized:     .emphasized,
            standardEasing: .standard
        )
    )
}

public extension Theme {
    /// Project-wide dark theme. Value-stable across calls; safe to compare
    /// for identity within a frame.
    static let defaultDark: Theme = DefaultDarkTheme.value
}
