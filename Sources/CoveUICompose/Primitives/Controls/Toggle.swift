#if canImport(CoreGraphics)
import CoreGraphics
#endif
import CovePlatformCore
import CoveUIRuntime

/// Boolean on/off control rendered as a switch track with a movable thumb.
public struct Toggle: View {
    public let isOn: Binding<Bool>
    public let isEnabled: Bool
    public let label: String?

    public init(isOn: Binding<Bool>,
                isEnabled: Bool = true,
                label: String? = nil) {
        self.isOn = isOn
        self.isEnabled = isEnabled
        self.label = label
    }

    public init(_ label: String,
                isOn: Binding<Bool>,
                isEnabled: Bool = true) {
        self.init(isOn: isOn, isEnabled: isEnabled, label: label)
    }

    public var body: some View {
        let control = _StatefulBoolControl(isOn: isOn,
                                           isEnabled: isEnabled,
                                           variant: .toggle)
        if let label {
            BoolControlLabel(label: label, isEnabled: isEnabled, control: control)
        } else {
            control
        }
    }
}

public enum CheckboxState: Sendable, Equatable { case off, on, mixed }

/// Mixed state represents a partially selected collection. Activation selects it fully.
public struct Checkbox: View {
    public let state: Binding<CheckboxState>
    public let isEnabled: Bool
    public let label: String?

    public init(state: Binding<CheckboxState>, isEnabled: Bool = true, label: String? = nil) {
        self.state = state
        self.isEnabled = isEnabled
        self.label = label
    }

    public init(isOn: Binding<Bool>,
                isEnabled: Bool = true,
                label: String? = nil) {
        self.state = Binding(get: { isOn.wrappedValue ? .on : .off },
                             set: { isOn.wrappedValue = $0 == .on })
        self.isEnabled = isEnabled
        self.label = label
    }

    public init(_ label: String,
                state: Binding<CheckboxState>,
                isEnabled: Bool = true) {
        self.init(state: state, isEnabled: isEnabled, label: label)
    }

    public init(_ label: String,
                isOn: Binding<Bool>,
                isEnabled: Bool = true) {
        self.init(isOn: isOn, isEnabled: isEnabled, label: label)
    }

    public var body: some View {
        let control = _StatefulBoolControl(isOn: Binding(get: { state.wrappedValue == .on },
                                          set: { state.wrappedValue = $0 ? .on : .off }),
                             isEnabled: isEnabled,
                             variant: state.wrappedValue == .mixed ? .mixedCheckbox : .checkbox)
        if let label {
            BoolControlLabel(label: label, isEnabled: isEnabled, control: control)
        } else {
            control
        }
    }
}

private struct BoolControlLabel<Control: View>: View {
    let label: String
    let isEnabled: Bool
    let control: Control

    var body: some View {
        Row(alignment: .center, spacing: 8) {
            control.compositionLocal(ActionControlAccessibility.label, label)
            Text(label)
                .font(.body)
                .foregroundColor(isEnabled ? .onSurface : .onSurfaceDisabled)
                .accessibilityHidden()
        }
        .frame(minWidth: 0)
    }
}

enum _BoolControlVariant: Sendable, Equatable {
    case toggle
    case checkbox
    case mixedCheckbox

    var layoutSize: (width: Float, height: Float) {
        switch self {
        case .toggle:
            // 36×20 track with a 16-point knob (reference metrics).
            return (36, 20)
        case .checkbox, .mixedCheckbox:
            return (16, 16)
        }
    }
}

struct _StatefulBoolControl: View {
    let isOn: Binding<Bool>
    let isEnabled: Bool
    let variant: _BoolControlVariant

    var body: some View {
        BoolControlHost(
            isOn: isOn,
            isEnabled: isEnabled,
            variant: variant
        )
    }
}

struct BoolControlHost: _PrimitiveView {
    let isOn: Binding<Bool>
    let isEnabled: Bool
    let variant: _BoolControlVariant

    static let pressedKey = "__bool_control_pressed"
    static let hoveredKey = "__bool_control_hovered"
    static let onKey = "__bool_control_on"
    static let variantKey = "__bool_control_variant"

    private struct PaintIdentity: Equatable {
        let isOn: Bool
        let isEnabled: Bool
        let variant: _BoolControlVariant
    }

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = true
        node.isFocusable = true
        return node
    }

    func _updateNode(_ node: Node) {
        node.applyTouchControlExtent()
        if !node.supportsControlHover { node.attachments[Self.hoveredKey] = false }
        node.isFocusable = isEnabled
        node.accessibility = AccessibilitySemantics(variant == .toggle ? .toggle : .checkbox) {
            $0.state.isEnabled = isEnabled; $0.state.isSelected = isOn.wrappedValue
            $0.value = variant == .mixedCheckbox ? "mixed" : isOn.wrappedValue ? "1" : "0"
            if let label = node.compositionValue(of: ActionControlAccessibility.label) { $0.label = label }
        }
        node.accessibilityActions = AccessibilityActions()
        let toggle = { [weak node] in
            if let node { WindowRevealContext.recordActivation(node) }
            isOn.wrappedValue.toggle()
        }
        if isEnabled { node.accessibilityActions.activate = toggle }
        if !isEnabled {
            node.attachments[Self.pressedKey] = false
            node.attachments[Self.hoveredKey] = false
            if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
            if FocusChainHolder.current?.focused === node { FocusChainHolder.current?.clear() }
        } else {
            if node.attachments[Self.pressedKey] == nil {
                node.attachments[Self.pressedKey] = false
            }
            if node.attachments[Self.hoveredKey] == nil {
                node.attachments[Self.hoveredKey] = false
            }
        }
        node.attachments[Self.onKey] = isOn.wrappedValue || variant == .mixedCheckbox
        node.attachments[Self.variantKey] = variant
        node.cursor = isEnabled ? .pointer : .notAllowed

        updateBoolControlAppearance(node, isOn: isOn.wrappedValue || variant == .mixedCheckbox, isEnabled: isEnabled)
        let snapshot = self
        node.updateDraw(identity: PaintIdentity(isOn: isOn.wrappedValue,
                                                isEnabled: isEnabled,
                                                variant: variant)) { list, origin in
            snapshot.render(node: node, origin: origin, list: list)
        }

        guard isEnabled, let registry = InteractionRegistryHolder.current else {
            InteractionRegistryHolder.current?.remove(node)
            return
        }

        registry.setHover(node) { phase in
            switch phase {
            case .enter:
                setBoolControlInteraction(node, key: Self.hoveredKey, value: node.supportsControlHover)
            case .leave:
                setBoolControlInteraction(node, key: Self.hoveredKey, value: false)
            }
        }
        registry.setPointer(node) { event, phase, _ in
            if event.button != .left { return .ignored }
            switch phase {
            case .down:
                PointerCaptureHolder.current?.acquire(node)
                setBoolControlInteraction(node, key: Self.pressedKey, value: true)
                return .handled
            case .up:
                let wasPressed = node.attachments[Self.pressedKey] as? Bool ?? false
                setBoolControlInteraction(node, key: Self.pressedKey, value: false)
                guard wasPressed else { return .ignored }
                if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                guard node.absoluteFrame.contains(CGPoint(x: CGFloat(event.x), y: CGFloat(event.y))) else { return .handled }
                toggle()
                node.attachments[Self.onKey] = isOn.wrappedValue
                updateBoolControlAppearance(node, isOn: isOn.wrappedValue, isEnabled: isEnabled)
                node.markRenderDirty(reason: .styleSet(field: "boolControlValue"))
                return .handled
            }
        }
        registry.setKey(node) { event, phase in
            guard phase == .target, !event.isRepeat else { return .ignored }
            switch event.scancode {
            case Scancode.return, Scancode.space, Scancode.keypadEnter:
                toggle()
                node.attachments[Self.onKey] = isOn.wrappedValue
                updateBoolControlAppearance(node, isOn: isOn.wrappedValue, isEnabled: isEnabled)
                node.markRenderDirty(reason: .styleSet(field: "boolControlValue"))
                return .handled
            default:
                return .ignored
            }
        }
    }

    func _makeLayoutNode() -> LayoutNode? {
        let layout = LayoutNode()
        let size = variant.layoutSize
        layout.width = size.width
        layout.height = size.height
        return layout
    }

    func _updateLayout(_ layout: LayoutNode) {
        let size = variant.layoutSize
        layout.width = size.width
        layout.height = size.height
    }

    private func render(node: Node, origin: CGPoint, list: DrawList) {
        switch variant {
        case .toggle:
            renderToggle(node: node, origin: origin, list: list)
        case .checkbox, .mixedCheckbox:
            renderCheckbox(node: node, origin: origin, list: list)
        }
    }

    /// Centered within a touch-sized target, leading-aligned when stretched wider.
    static func leadingOffset(available: Float, content: Float) -> Float {
        let excess = max(0, available - content)
        return available <= 44 ? excess * 0.5 : 0
    }

    private func renderToggle(node: Node, origin: CGPoint, list: DrawList) {
        let frame = node.frame
        let width = Float(frame.width)
        let height = Float(frame.height)
        guard width > 0, height > 0 else { return }

        let colors = node.theme.colors
        let originX = Float(origin.x)
        let originY = Float(origin.y)
        let trackHeight: Float = 20
        let thumbDiameter: Float = 16
        let thumbInset: Float = 2
        // The switch keeps its intrinsic size. A touch extent centers it in the
        // 44pt target; a layout that stretches the control (a form value slot)
        // leaves it at the leading edge instead of painting a full-width pill.
        let trackWidth = min(width, variant.layoutSize.width)
        let trackRect = UIRect(x: originX + Self.leadingOffset(available: width, content: trackWidth),
                               y: originY + (height - trackHeight) * 0.5,
                               width: trackWidth,
                               height: trackHeight)
        list.addRoundedRect(trackRect,
                            radius: trackHeight * 0.5,
                            color: resolvedFillColor(node: node, colors: colors))

        let thumbTravel = trackRect.width - 2 * thumbInset - thumbDiameter
        let progress = node.attachments["__bool_visual_progress"] as? Float ?? (isOn.wrappedValue ? 1 : 0)
        let thumbX = trackRect.minX + thumbInset + thumbTravel * progress
        let thumbRect = UIRect(x: thumbX,
                               y: trackRect.minY + (trackRect.height - thumbDiameter) * 0.5,
                               width: thumbDiameter,
                               height: thumbDiameter)

        // The knob is a white disc with a small key shadow, as in the
        // reference — a raised disc rather than an outlined one.
        let thumbFill = resolvedToggleThumbColor(colors: colors)
        list.addShadow(SurfaceShadow(color: Color(r: 0, g: 0, b: 0, a: 0.22),
                                     offsetX: 0, offsetY: 1, blur: 2),
                       rect: thumbRect, cornerRadius: thumbRect.height * 0.5)
        list.addRoundedRect(thumbRect,
                            radius: thumbRect.height * 0.5,
                            color: thumbFill)

        // The ring belongs outside the whole control, drawn as a ring and with a
        // gap: flush against the fill it cannot be told apart from it.
        if FocusChainHolder.current?.focused === node, FocusChainHolder.current?.isFocusVisible == true {
            let gap: Float = 1
            let ring: Float = 2
            let outset = gap + ring
            let ringRect = UIRect(x: trackRect.minX - outset,
                                  y: trackRect.minY - outset,
                                  width: trackRect.width + 2 * outset,
                                  height: trackRect.height + 2 * outset)
            list.addRoundedRectStroke(ringRect,
                                      radius: ringRect.height * 0.5,
                                      width: ring,
                                      color: colors.focusRing)
        }
    }

    private func renderCheckbox(node: Node, origin: CGPoint, list: DrawList) {
        let frame = node.frame
        let width = Float(frame.width)
        let height = Float(frame.height)
        guard width > 0, height > 0 else { return }

        let colors = node.theme.colors
        let originX = Float(origin.x)
        let originY = Float(origin.y)
        let edge = min(width, height, variant.layoutSize.width)
        let boxRect = UIRect(x: originX + Self.leadingOffset(available: width, content: edge),
                             y: originY + (height - edge) * 0.5,
                             width: edge,
                             height: edge)
        let isFocused = (FocusChainHolder.current?.focused === node
            && FocusChainHolder.current?.isFocusVisible == true)
        // A 16×16 box: white fill (background) with a 1-px inset border,
        // matching the reference control.
        let fill = resolvedCheckboxFillColor(node: node, colors: colors)
        let borderColor: Color = {
            if !isEnabled { return colors.onSurface.multipliedAlpha(0.3) }
            if isOn.wrappedValue { return fill }
            let hovered = node.attachments[Self.hoveredKey] as? Bool ?? false
            return hovered ? colors.borderStrong : colors.border
        }()
        list.addRoundedRect(boxRect,
                            radius: 4,
                            color: fill)
        list.addRoundedRectStroke(boxRect,
                                  radius: 4,
                                  width: 1,
                                  color: borderColor)
        if isFocused {
            let gap: Float = 1
            let ring: Float = 2
            let outset = gap + ring
            let ringRect = UIRect(x: boxRect.minX - outset,
                                  y: boxRect.minY - outset,
                                  width: boxRect.width + 2 * outset,
                                  height: boxRect.height + 2 * outset)
            list.addRoundedRectStroke(ringRect,
                                      radius: 4 + outset,
                                      width: ring,
                                      color: colors.focusRing)
        }

        let checkProgress = node.attachments["__bool_visual_progress"] as? Float ?? (isOn.wrappedValue ? 1 : 0)
        if checkProgress > 0 {
            let inset = max(3, edge * 0.18)
            let x0 = boxRect.minX + inset
            let y0 = boxRect.minY + edge * 0.55
            let x1 = boxRect.minX + edge * 0.42
            let y1 = boxRect.minY + edge - inset
            let x2 = boxRect.minX + edge - inset
            let y2 = boxRect.minY + inset
            let lineColor = (isEnabled ? colors.onAccent : colors.onSurfaceMuted).multipliedAlpha(checkProgress * node.opacity)
            if variant == .mixedCheckbox {
                list.addLine(fromX: x0, fromY: boxRect.minY + edge / 2,
                             toX: x2, toY: boxRect.minY + edge / 2, thickness: 2, color: lineColor)
                return
            }
            list.addLine(fromX: x0, fromY: y0,
                         toX: x1, toY: y1,
                         thickness: 2,
                         color: lineColor)
            list.addLine(fromX: x1, fromY: y1,
                         toX: x2, toY: y2,
                         thickness: 2,
                         color: lineColor)
        }
    }

    private func resolvedFillColor(node: Node, colors: ColorScheme) -> Color {
        if let fill = node.attachments["__bool_visual_fill"] as? Color { return fill.multipliedAlpha(node.opacity) }
        if !isEnabled {
            // Disabled track shares the same dimmed border fill.
            return colors.border.multipliedAlpha(0.8)
        }
        let isPressed = node.attachments[Self.pressedKey] as? Bool ?? false
        let isHovered = node.attachments[Self.hoveredKey] as? Bool ?? false
        if isOn.wrappedValue {
            if isPressed { return colors.accentPressed }
            if isHovered { return colors.accentHover }
            return colors.accent
        }

        // Off is a solid neutral track (border darkened toward text),
        // matching the reference: a grey track rather than a translucent wash.
        let base = colors.borderStrong
        if isPressed { return colors.borderStrong.mixed(with: colors.onSurface, amount: 0.08) }
        if isHovered { return colors.borderStrong.mixed(with: colors.onSurface, amount: 0.04) }
        return base
    }

    /// A white disc in both states, as on phones: on a raised surface a surface-
    /// coloured thumb vanishes into the card.
    private func resolvedToggleThumbColor(colors: ColorScheme) -> Color {
        isEnabled ? .white : Color.white.multipliedAlpha(0.6)
    }

    /// The fill of a check box. Unchecked it is the window background (white),
    /// disabled it is the control face (surface); checked it is the accent
    /// ramp. This differs from the switch, whose off track is a solid neutral.
    private func resolvedCheckboxFillColor(node: Node, colors: ColorScheme) -> Color {
        if let fill = node.attachments["__bool_visual_fill"] as? Color {
            return fill.multipliedAlpha(node.opacity)
        }
        if !isEnabled { return colors.surface }
        let isPressed = node.attachments[Self.pressedKey] as? Bool ?? false
        let isHovered = node.attachments[Self.hoveredKey] as? Bool ?? false
        if isOn.wrappedValue {
            if isPressed { return colors.accentPressed }
            if isHovered { return colors.accentHover }
            return colors.accent
        }
        return colors.background
    }
}

private func setBoolControlInteraction(_ node: Node, key: String, value: Bool) {
    let previous = node.attachments[key] as? Bool
    guard previous != value else { return }
    node.attachments[key] = value
    updateBoolControlAppearance(node, isOn: node.attachments[BoolControlHost.onKey] as? Bool == true,
                                isEnabled: node.cursor != .notAllowed)
    node.markRenderDirty(reason: .styleSet(field: key))
}

private func updateBoolControlAppearance(_ node: Node, isOn: Bool, isEnabled: Bool) {
    let colors = node.theme.colors
    let pressed = node.attachments[BoolControlHost.pressedKey] as? Bool == true
    let hovered = node.attachments[BoolControlHost.hoveredKey] as? Bool == true
    let isSwitch = node.attachments[BoolControlHost.variantKey] as? _BoolControlVariant == .toggle
    // Off fill differs by control: the switch has a solid neutral track, the
    // check box shows the window background (white) inside its border.
    let offBase = isSwitch ? colors.borderStrong : colors.background
    let base = !isEnabled ? (isSwitch ? colors.border.multipliedAlpha(0.8) : colors.surface)
        : isOn ? colors.accent : offBase
    // An unchecked box keeps its fill; only the border reacts to hover.
    let offHover = isSwitch ? colors.borderStrong.mixed(with: colors.onSurface, amount: 0.04) : colors.background
    let offPress = isSwitch ? colors.borderStrong.mixed(with: colors.onSurface, amount: 0.08) : colors.background
    let fill = !isEnabled ? base
        : pressed ? (isOn ? colors.accentPressed : offPress)
        : hovered ? (isOn ? colors.accentHover : offHover)
        : base
    let previous = node.attachments["__bool_visual_progress"] as? Float
    let applyProgress = { [node] in
        node.animatableSet(propertyKey: "bool.progress", current: previous ?? (isOn ? 1 : 0), to: Float(isOn ? 1 : 0)) { [weak node] value in
            node?.attachments["__bool_visual_progress"] = value
            node?.markRenderDirty(reason: .styleSet(field: "bool.progress"))
        }
    }
    let applyFill = { [node] in
        node.animatableSet(propertyKey: "bool.fill", current: node.attachments["__bool_visual_fill"] as? Color ?? fill, to: fill) { [weak node] value in
            node?.attachments["__bool_visual_fill"] = value
            node?.markRenderDirty(reason: .styleSet(field: "bool.fill"))
        }
    }
    if previous != nil {
        let motion: Animation = isSwitch
            ? .spring(response: node.theme.motion.fast.seconds, dampingFraction: 0.9, allowsOvershoot: true)
            : .semantic(.fast, in: node.theme)
        withAnimation(motion, applyProgress)
        withAnimation(.semantic(.fast, in: node.theme), applyFill)
    } else { applyProgress(); applyFill() }
}
