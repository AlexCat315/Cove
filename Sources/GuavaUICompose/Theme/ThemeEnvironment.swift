import GuavaUIRuntime

/// `CompositionLocal` key carrying the active `Theme` down the composition
/// tree. Always resolves: descendants without an ancestor `.theme(_:)`
/// modifier see `Theme.defaultDark`.
public enum ThemeEnvironment {
    public static let key = CompositionLocal<Theme>(defaultValue: .defaultDark)
    static let bodyStyle = CompositionLocal<TextStyleToken?>(defaultValue: nil)
    static let bodyStyleAttachment = "__theme_body_style"
}

public extension View {
    /// Provide `theme` to every descendant of this view. Nearer providers
    /// override farther ones; absent providers fall back to
    /// `Theme.defaultDark`.
    ///
    /// ```swift
    /// RootView().theme(.defaultDark)
    /// ```
    func theme(_ theme: Theme) -> some View {
        compositionLocal(ThemeEnvironment.key, theme)
            .compositionLocal(ThemeEnvironment.bodyStyle, theme.typography.body.resolvingFamily(theme.typography.family))
    }

    /// Animate a custom palette when its ordinary domain value changes. The
    /// renderer retains the previous frame; layout and input use the new theme.
    func theme<Value: Equatable>(_ theme: Theme, transition: ThemeTransition, value: Value) -> some View {
        CircularThemeHost(theme: theme, value: value, transition: transition, content: self)
    }
}

public extension Node {
    /// The `Theme` resolved for this node by walking its ancestor chain.
    /// Falls back to `Theme.defaultDark` when no provider is reachable.
    var theme: Theme {
        compositionValue(of: ThemeEnvironment.key)
    }

    /// Nearest explicitly provided theme at or below this node.
    ///
    /// Platform hosts use it for chrome outside the app's own layout — system
    /// bar icon contrast, for example — without knowing which component
    /// applied the appearance. The search is bounded: applications provide
    /// their theme near the root, so a deep scan would only cost frames.
    var surfaceTheme: Theme? {
        var level = [self]
        var depth = 0
        var budget = 512
        while !level.isEmpty, depth < 12, budget > 0 {
            var next: [Node] = []
            for node in level {
                budget -= 1
                if let theme = node.compositionValues[ThemeEnvironment.key.key] as? Theme { return theme }
                next.append(contentsOf: node.children)
            }
            level = next
            depth += 1
        }
        return nil
    }
}
