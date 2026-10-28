import CoveUIScene
import CoveUIText
import CoveUIImaging

private struct _ListRowVisualKey: Equatable, Sendable {
    let fill: Color
    let alpha: Float
}

/// Pointer rows use the shared navigation highlight; touch rows use a full-width
/// band with an inset separator. The host owns the fixed row slot so oversized
/// content cannot expand the hit region into a neighboring row.
public struct DefaultListRowStyle: ListRowStyle {
    public init() {}

    public func makeBody(configuration: ListRowStyleConfiguration) -> some View {
        let theme = configuration.theme
        let touch = configuration.input.contains(.touch)
        let colors = theme.colors
        // A selected row is a solid accent band with a light label, like the
        // reference; an unselected hover is a neutral surface face.
        let fill: Color = {
            if configuration.isSelected {
                return configuration.isHovered ? colors.accentHover : colors.accent
            }
            return configuration.isHovered ? colors.surfaceVariant : .clear
        }()
        let radius: Float = touch ? 0 : theme.radius.md
        let alpha: Float = configuration.isEnabled ? 1 : 0.55
        return Row(alignment: .center, spacing: theme.spacing.sm) {
            configuration.content.frame(minWidth: 0).flex(1, shrink: 1, basis: 0)
        }
        .frame(height: max(0, configuration.rowHeight - 2 * (touch ? 0 : theme.spacing.xs)))
        // Only the selected band re-inks its label; unselected rows keep the
        // default ink, so a theme cross-fade doesn't animate a redundant color.
        .modifier(SelectedRowInk(isSelected: configuration.isSelected, onAccent: colors.onAccent))
        .background(fill)
        .cornerRadius(radius)
        .padding(horizontal: touch ? 16 : theme.spacing.sm, vertical: 0)
        .modifier(ListRowSeparator(color: theme.colors.divider, isEnabled: touch))
        .opacity(alpha)
        .animation(.semantic(.fast, in: theme), value: _ListRowVisualKey(fill: fill, alpha: alpha))
    }
}

private struct SelectedRowInk: ViewModifier {
    let isSelected: Bool
    let onAccent: Color
    @ViewBuilder
    func body(content: Content) -> some View {
        if isSelected {
            content.foregroundColor(onAccent)
        } else {
            content
        }
    }
}

private struct ListRowSeparator: ViewModifier {
    let color: Color
    let isEnabled: Bool
    func apply(node: Node) {
        guard isEnabled else { node.draw = nil; return }
        var parent: Node? = node
        var rtl = false
        while let current = parent {
            if let value = current.layoutNode?.direction, value != .inherit { rtl = value == .rtl; break }
            parent = current.parent
        }
        node.draw = { [weak node] list, origin in
            guard let node else { return }
            let left: Float = rtl ? 0 : 16
            // A hairline, as on phones: it marks the row edge without drawing a rule.
            list.addRect(UIRect(x: Float(origin.x) + left, y: Float(origin.y + node.frame.height) - 0.5,
                                width: max(0, Float(node.frame.width) - 16), height: 0.5), color: color)
        }
    }
}
