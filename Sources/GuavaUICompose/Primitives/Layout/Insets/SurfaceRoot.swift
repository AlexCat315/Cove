import GuavaUIRuntime

/// Installs host surface facts once at the root of a scene: safe-area insets,
/// text scale and reduced motion.
///
/// The root deliberately does **not** pad content. Modern platforms draw apps
/// edge to edge: the app paints its background behind the system bars and only
/// its interactive chrome is inset. `SafeArea` consumes the insets this root
/// publishes, so screens that own a status bar, navigation bar or keyboard get
/// the correct avoidance without a letterboxed surface.
///
/// Application hosts and device previews share this entry point so a preview
/// cannot drift from what a real surface renders.
public struct SurfaceRoot<Content: View>: View {
    public let environment: UIEnvironment
    private let content: Content

    public init(_ environment: UIEnvironment, @ViewBuilder content: () -> Content) {
        self.environment = environment
        self.content = content()
    }

    public var body: some View {
        // Each surface replaces the inherited system ramp, retaining authored
        // application scaling. Nested surfaces must not multiply host ramps.
        TextScalerReader { inherited in
            Box {
                content
                    .flex(1, shrink: 1, basis: 0)
                    .frame(minWidth: 0, minHeight: 0)
            }
            .flex(1, shrink: 1, basis: 0)
            .frame(minWidth: 0, minHeight: 0)
            .surfaceEnvironment(environment)
            .safeAreaInsets(insets)
            .reducedMotion(environment.reducesMotion)
            .textScaler(inherited.applyingSystemScale(environment.textScale))
        }
    }

    /// The safe area is persistent chrome; the keyboard overlays it while it is
    /// visible, so the two never add up.
    private var insets: EdgeInsets {
        let safe = environment.safeArea.normalized
        let keyboard = environment.keyboard.height.isFinite ? max(0, environment.keyboard.height) : 0
        return EdgeInsets(top: safe.top,
                          leading: safe.leading,
                          bottom: max(safe.bottom, keyboard),
                          trailing: safe.trailing)
    }
}
