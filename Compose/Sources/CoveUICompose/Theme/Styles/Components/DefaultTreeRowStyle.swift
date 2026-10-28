import CoveUIScene
import CoveUIText
import CoveUIImaging

private struct _TreeRowVisualKey: Equatable, Sendable {
    let background: Color
    let alpha: Float
}

/// Tree row default: same density and hierarchy cues as the list style, with
/// enough selection contrast to read as an outline tree in an editor shell.
///
/// The navigation highlight leaves the hierarchy gutter clear. Subtle vertical
/// guides carry depth without connecting horizontal strokes to the row chrome.
public struct DefaultTreeRowStyle: TreeRowStyle {
    public init() {}

    public func makeBody(configuration: TreeRowStyleConfiguration) -> some View {
        let t = configuration.theme
        let colors = t.colors
        // A selected row is a solid accent band with a light label, matching
        // the reference; an unselected hover is a neutral surface face.
        let bg: Color = {
            if configuration.isSelected {
                return configuration.isHovered ? colors.accentHover : colors.accent
            }
            if configuration.isSearchHit { return colors.stateLayerSelected }
            return configuration.isHovered ? colors.surfaceVariant : .clear
        }()
        let alpha: Float = configuration.isEnabled ? 1 : 0.55
        let visualKey = _TreeRowVisualKey(background: bg, alpha: alpha)

        return Row(alignment: .center, spacing: 0) {
            Row(alignment: .center, spacing: t.spacing.sm) {
                configuration.content
                Spacer(minLength: 0)
            }
            .flex()
            .padding(horizontal: t.spacing.sm + 1, vertical: t.spacing.xs + 1)
        }
        // Only the selected band re-inks its label; unselected rows keep the
        // default ink, so a theme cross-fade doesn't animate a redundant color.
        .modifier(SelectedTreeRowInk(isSelected: configuration.isSelected, onAccent: colors.onAccent))
        .background(bg)
        .cornerRadius(t.radius.md)
        .opacity(alpha)
        .animation(.semantic(.fast, in: t), value: visualKey)
    }
}

private struct SelectedTreeRowInk: ViewModifier {
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
