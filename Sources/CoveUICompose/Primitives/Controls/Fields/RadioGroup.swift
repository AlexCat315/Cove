import CoveUIRuntime

public struct RadioOption<Value: Hashable>: Identifiable {
    public let value: Value
    public let title: String
    public var detail = ""
    public var isEnabled = true
    public var id: Value { value }
    public init(_ value: Value, _ title: String, configure: (inout Self) -> Void = { _ in }) {
        self.value = value; self.title = title; configure(&self)
    }
}

/// Controlled selection with one Tab stop and wrapping arrow-key navigation.
public struct RadioGroup<Value: Hashable>: View {
    public let selection: Binding<Value?>
    public let options: [RadioOption<Value>]
    public var isEnabled = true
    public var isHorizontal = false
    public init(selection: Binding<Value?>, options: [RadioOption<Value>], configure: (inout Self) -> Void = { _ in }) {
        self.selection = selection; self.options = options; configure(&self)
        precondition(Set(options.map(\.value)).count == options.count, "Radio option values must be unique")
    }
    public var body: some View {
        let enabled = options.filter { isEnabled && $0.isEnabled }.map(\.value)
        let tabStop = selection.wrappedValue.flatMap { enabled.contains($0) ? $0 : nil } ?? enabled.first
        RovingFocusHost(enabledItems: enabled.map { AnyHashable($0) }, selection: { selection.wrappedValue.map { AnyHashable($0) } },
                        onSelect: { if let value = $0.base as? Value { selection.wrappedValue = value } }) {
            Box(direction: isHorizontal ? .row : .column, alignItems: isHorizontal ? .center : .stretch, spacing: 8) {
                for option in options {
                    AnyView(Button(isEnabled: isEnabled && option.isEnabled, isSelected: selection.wrappedValue == option.value,
                                   action: { selection.wrappedValue = option.value }) {
                        Row(alignment: .center, spacing: 8) {
                            RadioMark(isSelected: selection.wrappedValue == option.value, isEnabled: isEnabled && option.isEnabled)
                            Column(alignment: .leading, spacing: 3) {
                                Text(option.title).font(.label)
                                if !option.detail.isEmpty { Text(option.detail).font(.caption).foregroundColor(.onSurfaceMuted) }
                            }
                        }
                    }.accessibility { $0.role = .radio; $0.label = option.title; $0.help = option.detail }.buttonStyle(RadioItemStyle()).modifier(RovingItemModifier(id: AnyHashable(option.value), isTabStop: option.value == tabStop)))
                }
            }
        }
    }
}

private struct RadioItemStyle: ButtonStyle {
    func makeBody(configuration c: ButtonStyleConfiguration) -> some View {
        Row(alignment: .center) { AnyView(c.label) }
            .flex(1, shrink: 1).padding(7)
            .foregroundColor(c.isEnabled ? .onSurface : .onSurfaceDisabled)
            .background(c.isHovered ? c.theme.colors.stateLayerHover : .clear)
            .cornerRadius(c.theme.radius.sm).border(c.isFocused ? c.theme.colors.focusRing : .clear, width: c.isFocused ? 2 : 0)
            .animation(motion: .fast, value: c.interactionKey)
    }
}

private struct RadioMark: _PrimitiveView {
    let isSelected: Bool
    let isEnabled: Bool
    var isHovered: Bool = false
    var isPressed: Bool = false
    private static let edge: Float = 16
    func _makeNode() -> Node { let node = Node(); node.isHitTestable = false; return node }
    func _makeLayoutNode() -> LayoutNode? { let layout = LayoutNode(); layout.width = Self.edge; layout.height = Self.edge; return layout }
    func _updateNode(_ node: Node) {
        node.draw = { [weak node] list, origin in
            guard let node else { return }
            let colors = node.theme.colors
            let edge = Self.edge
            let x = Float(origin.x), y = Float(origin.y)
            let rect = UIRect(x: x, y: y, width: edge, height: edge)
            // White face with a thin border, like the check box.
            let fill = isEnabled ? colors.background : colors.surface
            let border: Color
            if !isEnabled {
                border = colors.onSurface.multipliedAlpha(0.3)
            } else if isSelected {
                border = colors.accent
            } else if isPressed {
                border = colors.accentPressed
            } else if isHovered {
                border = colors.borderStrong
            } else {
                border = colors.border
            }
            list.addRoundedRect(rect, radius: edge * 0.5, color: fill.multipliedAlpha(node.opacity))
            // The selected ring is 2 px; the empty ring is 1 px.
            list.addRoundedRectStroke(rect, radius: edge * 0.5,
                                      width: isSelected ? 2 : 1,
                                      color: border.multipliedAlpha(node.opacity))
            if isSelected {
                let dot = isEnabled ? colors.accent : colors.onSurfaceMuted
                let d: Float = 6
                list.addRoundedRect(UIRect(x: x + (edge - d) * 0.5, y: y + (edge - d) * 0.5,
                                           width: d, height: d),
                                    radius: d * 0.5,
                                    color: dot.multipliedAlpha(node.opacity))
            }
        }
    }
}
