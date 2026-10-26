internal import Foundation
internal import CoveUIRuntime

enum BuiltinButtonChromeKind: Hashable {
    case primary
    case secondary
    case ghost
    case destructive
    case toggle(minWidth: Float, height: Float)
}

struct BuiltinButtonChrome: _PrimitiveView {
    let kind: BuiltinButtonChromeKind
    let isEnabled: Bool
    let isSelected: Bool
    let foreground: SemanticColorRef
    let label: any View
    private let metrics: BuiltinButtonChromeMetrics

    init(kind: BuiltinButtonChromeKind,
         configuration: ButtonStyleConfiguration,
         foreground: SemanticColorRef) {
        self.kind = kind
        self.isEnabled = configuration.isEnabled
        self.isSelected = configuration.isSelected
        self.foreground = foreground
        self.label = configuration.label
        self.metrics = kind.metrics(in: configuration.theme, size: configuration.controlSize, isIconOnly: configuration.isIconOnly, input: configuration.input)
    }

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = false
        return node
    }

    func _updateNode(_ node: Node) {
        node.attachments[Self.stateKey] = BuiltinButtonChromeState(
            kind: kind,
            isEnabled: isEnabled,
            isSelected: isSelected,
            metrics: metrics
        )
        applyBuiltinButtonChrome(to: node, animated: false)
    }

    func _makeLayoutNode() -> LayoutNode? {
        LayoutNode()
    }

    func _updateLayout(_ layout: LayoutNode) {
        layout.flexDirection = .row
        layout.alignItems = .center
        layout.justifyContent = .center
        // The host owns the hit region. Built-in chrome should occupy that
        // same region when a form/toolbar gives the button extra width, while
        // retaining its intrinsic label width when the host is unconstrained.
        layout.flexGrow = 1
        layout.flexShrink = 1
        layout.minHeight = metrics.height
        layout.minWidth = metrics.minWidth
        layout.setPadding(metrics.verticalPadding, edge: .top)
        layout.setPadding(metrics.horizontalPadding, edge: .left)
        layout.setPadding(metrics.verticalPadding, edge: .bottom)
        layout.setPadding(metrics.horizontalPadding, edge: .right)
    }

    func _children(for node: Node) -> [any View] {
        [
            AnyView(label)
                .font(SemanticFontRef.label)
                .foregroundColor(foreground)
        ]
    }

    static let stateKey = "__builtin_button_chrome_state"
}

private struct BuiltinButtonChromeState {
    let kind: BuiltinButtonChromeKind
    let isEnabled: Bool
    let isSelected: Bool
    let metrics: BuiltinButtonChromeMetrics
}

private struct BuiltinButtonChromeMetrics {
    let height: Float
    let minWidth: Float?
    let horizontalPadding: Float
    let radius: Float
    let verticalPadding: Float
}

private struct BuiltinButtonChromeValues {
    let background: Color
    let border: Color
    let borderWidth: Float
    let radius: Float
    let opacity: Float
    let scale: Float
}

private extension BuiltinButtonChromeKind {
    func metrics(in theme: Theme, size: ControlSize, isIconOnly: Bool, input: UIEnvironment.Input) -> BuiltinButtonChromeMetrics {
        let touch = input.contains(.touch)
        let height = touch ? size.touchHeight : size.buttonHeight
        let lineHeight = theme.typography.label.lineHeight
        switch self {
        case .primary, .secondary, .ghost, .destructive:
            return BuiltinButtonChromeMetrics(height: height,
                                              minWidth: isIconOnly ? height : nil,
                                              horizontalPadding: isIconOnly ? max(2, (height - lineHeight) / 2) : touch ? size.touchPadding : size.horizontalPadding,
                                              radius: touch ? max(12, theme.radius.md) : theme.radius.md,
                                              verticalPadding: max(touch ? 4 : 2, (height - lineHeight) / (touch ? 4 : 2)))
        case .toggle(let minWidth, let authoredHeight):
            let height = touch ? max(height, authoredHeight) : authoredHeight
            return BuiltinButtonChromeMetrics(height: height,
                                              minWidth: minWidth,
                                              horizontalPadding: touch ? 12 : 7,
                                              radius: touch ? max(10, theme.radius.sm) : theme.radius.sm,
                                              verticalPadding: max(touch ? 4 : 2, (height - lineHeight) / (touch ? 4 : 2)))
        }
    }
}

func updateBuiltinButtonChromeDescendants(of buttonNode: Node, animated: Bool) {
    for child in buttonNode.children {
        updateBuiltinButtonChromeDescendants(child, animated: animated)
    }
}

private func updateBuiltinButtonChromeDescendants(_ node: Node, animated: Bool) {
    if node.attachments[BuiltinButtonChrome.stateKey] is BuiltinButtonChromeState {
        applyBuiltinButtonChrome(to: node, animated: animated)
    }
    for child in node.children {
        updateBuiltinButtonChromeDescendants(child, animated: animated)
    }
}

private func applyBuiltinButtonChrome(to chromeNode: Node, animated: Bool) {
    let animated = animated && !chromeNode.prefersReducedMotion
    guard let state = chromeNode.attachments[BuiltinButtonChrome.stateKey] as? BuiltinButtonChromeState else {
        return
    }
    let buttonNode = nearestButtonHostAncestor(of: chromeNode)
    let values = builtinButtonChromeValues(state: state,
                                           chromeNode: chromeNode,
                                           buttonNode: buttonNode)
    let apply = {
        chromeNode.animatableSet(\.backgroundColor, to: values.background)
        chromeNode.animatableSet(\.borderColor, to: values.border)
        chromeNode.animatableSet(\.borderWidth, to: values.borderWidth)
        chromeNode.animatableSet(\.cornerRadius, to: values.radius)
        chromeNode.animatableSet(\.opacity, to: values.opacity)
    }
    if animated {
        withAnimation(.semantic(.snappy, in: chromeNode.theme), apply)
    } else {
        apply()
    }
    chromeNode.transform.anchor = CGPoint(x: 0.5, y: 0.5)
    let transform = {
        chromeNode.animatableSet(propertyKey: "button.pressScale", current: chromeNode.transform.scale,
                                to: values.scale) { [weak chromeNode] in chromeNode?.transform.scale = max(0, $0) }
    }
    if animated {
        withAnimation(.spring(response: chromeNode.theme.motion.fast.seconds, dampingFraction: 0.9,
                              allowsOvershoot: true), transform)
    } else { transform() }
}

private func builtinButtonChromeValues(state: BuiltinButtonChromeState,
                                       chromeNode: Node,
                                       buttonNode: Node?) -> BuiltinButtonChromeValues {
    let theme = chromeNode.theme
    let pressed = state.isEnabled && (buttonNode?.attachments[ButtonHost.pressedKey] as? Bool == true)
    let hovered = state.isEnabled && (buttonNode?.attachments[ButtonHost.hoveredKey] as? Bool == true)
    let focused = state.isEnabled && FocusChainHolder.current?.isFocusVisible == true
        && buttonNode.map { FocusChainHolder.current?.focused === $0 } == true
    let metrics = state.metrics
    let clear = Color.clear
    let background: Color
    let border: Color
    let borderWidth: Float

    switch state.kind {
    case .primary:
        if !state.isEnabled {
            background = theme.colors.surfaceVariant
        } else if pressed {
            background = theme.colors.accentPressed
        } else if hovered {
            background = theme.colors.accentHover
        } else {
            background = theme.colors.accent
        }
        border = focused ? theme.colors.focusRing : clear
        borderWidth = focused ? 2 : 0
    case .secondary:
        // A translucent ink wash, so the button stands out from whatever surface
        // it sits on instead of matching a card of the same tone.
        let ink = theme.colors.onSurface
        if !state.isEnabled {
            background = ink.multipliedAlpha(0.04)
        } else if pressed {
            background = ink.multipliedAlpha(0.16)
        } else if hovered {
            background = ink.multipliedAlpha(0.12)
        } else {
            background = ink.multipliedAlpha(0.08)
        }
        // The tonal fill already reads as a button; an outline only adds noise.
        border = focused ? theme.colors.focusRing : clear
        borderWidth = focused ? 2 : 0
    case .ghost:
        if pressed {
            background = theme.colors.stateLayerPressed
        } else if hovered {
            background = theme.colors.stateLayerHover
        } else {
            background = clear
        }
        border = focused ? theme.colors.focusRing : clear
        borderWidth = focused ? 2 : 0
    case .destructive:
        let error = theme.colors.error
        if !state.isEnabled {
            background = theme.colors.surfaceVariant
        } else if pressed {
            background = error.composited(over: theme.colors.stateLayerPressed)
        } else if hovered {
            background = error.composited(over: theme.colors.stateLayerHover)
        } else {
            background = error
        }
        border = focused ? theme.colors.focusRing : clear
        borderWidth = focused ? 2 : 0
    case .toggle:
        // Selection still conveys the current mode when its command cannot
        // be repeated (e.g. Play while already playing). Keep the paired
        // accent/onAccent colors even when disabled.
        if state.isSelected {
            if pressed {
                background = theme.colors.accentPressed
            } else if hovered {
                background = theme.colors.accentHover
            } else {
                background = theme.colors.accent
            }
        } else if pressed {
            background = theme.colors.stateLayerPressed
        } else if hovered {
            background = theme.colors.stateLayerHover
        } else {
            background = clear
        }
        border = focused ? theme.colors.focusRing : clear
        borderWidth = focused ? 2 : 0
    }

    return BuiltinButtonChromeValues(background: background,
                                     border: border,
                                     borderWidth: borderWidth,
                                     radius: metrics.radius,
                                     opacity: state.isEnabled || theme.textEmphasis.disabled != nil ? 1 : (state.isSelected ? 0.75 : 0.55),
                                     scale: pressed && !chromeNode.prefersReducedMotion ? (theme.motion.pressScale.isFinite ? min(chromeNode.compositionValue(of: SurfaceEnvironment.key).input.contains(.touch) ? 0.96 : 1, max(0.8, theme.motion.pressScale)) : 0.97) : 1)
}

private func nearestButtonHostAncestor(of node: Node) -> Node? {
    var current = node.parent
    while let candidate = current {
        if candidate.attachments[ButtonHost.markerKey] as? Bool == true {
            return candidate
        }
        current = candidate.parent
    }
    return nil
}
