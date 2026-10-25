internal import GuavaUIRuntime

struct MobileTabBar<ID: Hashable>: View {
    let tabs: [TabItem<ID>]
    let selection: Binding<ID>
    let tabStop: ID?
    let isEnabled: Bool
    @State private var showsMore = false

    var body: some View {
        let visible = visibleTabs
        let ids = Set(visible.map(\.id))
        let hidden = tabs.filter { !ids.contains($0.id) }
        return Box(direction: .column, alignItems: .stretch, spacing: 0) {
            Divider()
            SafeArea([.horizontal, .bottom]) {
                Row(alignment: .center, spacing: 0) {
                    for tab in visible { AnyView(item(tab).id(tab.id)) }
                    if !hidden.isEmpty { AnyView(more(hidden)) }
                }.padding(horizontal: 4, vertical: 6)
            }
        }.background(.surface).debugName("mobile-tab-bar")
    }

    private var visibleTabs: [TabItem<ID>] {
        guard tabs.count > 5 else { return tabs }
        var result = Array(tabs.prefix(4))
        if let selected = tabs.first(where: { $0.id == selection.wrappedValue }), !result.contains(where: { $0.id == selected.id }) {
            result[3] = selected
        }
        return result
    }

    private func more(_ hidden: [TabItem<ID>]) -> some View {
        var popover = Popover(isPresented: $showsMore, isEnabled: isEnabled && hidden.contains(where: \.isEnabled),
            triggerStyle: AnyButtonStyle(MobileTabStyle()), label: {
                Column(alignment: .center, spacing: 4) {
                    Icon(UIIcons.Action.moreHorizontal, size: 24).accessibilityHidden()
                    Text("More").truncatingTail().font(.caption).accessibilityHidden()
                }.frame(minWidth: 0)
            }, content: {
                Menu(hidden.map { tab in
                    .item(MenuItem(id: tab.id, title: tab.label, isEnabled: tab.isEnabled,
                        content: .init(icon: tab.icon, detail: tab.badge)) { selection.wrappedValue = tab.id })
                }, onItemActivated: { showsMore = false })
            })
        popover.triggerDescription = .init(label: "More tabs")
        return popover.flex(1, shrink: 1, basis: 0).frame(minWidth: 0)
    }

    private func item(_ tab: TabItem<ID>) -> some View {
        Button(isEnabled: isEnabled && tab.isEnabled, isSelected: selection.wrappedValue == tab.id,
               action: { selection.wrappedValue = tab.id }) {
            Column(alignment: .center, spacing: 4) {
                if let icon = tab.icon {
                    Icon(icon, size: 24).accessibilityHidden()
                        .overlay(alignment: .topTrailing) {
                            if let badge = tab.badge {
                                Badge(badge) { $0.variant = .solid }.offset(x: 10, y: -5).accessibilityHidden()
                            }
                        }
                }
                Text(tab.label).truncatingTail().font(.caption).accessibilityHidden()
                if tab.icon == nil, let badge = tab.badge {
                    Badge(badge) { $0.variant = .solid }.accessibilityHidden()
                }
            }.frame(minWidth: 0)
        }
        .buttonStyle(MobileTabStyle())
        .compositionLocal(ActionControlAccessibility.label, tab.label)
        .modifier(RovingItemModifier(id: AnyHashable(tab.id), isTabStop: tab.id == tabStop))
        .flex(1, shrink: 1, basis: 0).frame(minWidth: 0)
        .accessibility {
            $0.role = .tab; $0.label = tab.label; $0.value = tab.badge ?? ""
            $0.state.isSelected = selection.wrappedValue == tab.id
        }
    }
}

private struct MobileTabStyle: ButtonStyle {
    func makeBody(configuration c: ButtonStyleConfiguration) -> some View {
        Box(direction: .column, alignItems: .stretch) { AnyView(c.label) }
            .padding(horizontal: 4, vertical: 6).frame(minWidth: 0, minHeight: 52)
            .flex(1, shrink: 1)
            .foregroundColor(!c.isEnabled ? c.theme.colors.onSurfaceMuted : c.isSelected ? c.theme.colors.accent : c.theme.colors.onSurfaceVariant)
            .background(c.isPressed ? c.theme.colors.stateLayerPressed : .clear)
            .cornerRadius(12)
            .border(c.isFocused ? c.theme.colors.focusRing : .clear, width: c.isFocused ? 2 : 0)
            .opacity(c.isEnabled ? 1 : 0.55)
            .animation(motion: .fast, value: c.interactionKey)
    }
}
