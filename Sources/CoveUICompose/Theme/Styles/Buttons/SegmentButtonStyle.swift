import CoveUIRuntime

/// A segmented selection uses one raised thumb inside the group's shared well.
/// Keep action-button emphasis for commands elsewhere in the interface.
struct SegmentButtonStyle: ButtonStyle {
    func makeBody(configuration c: ButtonStyleConfiguration) -> some View {
        let base = c.isSelected ? c.theme.colors.surfaceRaised : .clear
        let fill = !c.isEnabled ? base : c.isPressed
            ? base.composited(over: c.theme.colors.stateLayerPressed)
            : c.isHovered ? base.composited(over: c.theme.colors.stateLayerHover) : base
        return Row(alignment: .center) { AnyView(c.label).font(.label) }
            .padding(horizontal: c.controlSize.horizontalPadding, vertical: 4)
            .frame(minHeight: max(20, c.controlSize.buttonHeight - 6))
            .foregroundColor(!c.isEnabled ? .onSurfaceDisabled : c.isSelected ? .onSurface : .onSurfaceVariant)
            .background(fill).cornerRadius(max(3, c.theme.radius.md - 3))
            .border(c.isFocused ? c.theme.colors.focusRing : .clear, width: c.isFocused ? 2 : 0)
            // A crisp 2-pt key shadow lifts the selected segment.
            .shadow(color: c.isSelected ? Color(r: 0, g: 0, b: 0, a: 0.12) : .clear,
                    offsetY: 1, blur: 2)
            .animation(motion: .fast, value: c.interactionKey)
    }
}
