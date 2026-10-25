internal import GuavaUIRuntime

/// A sheet navigates branches in place instead of opening another anchored
/// portal alongside the current level.
struct TouchMenu: View {
    let entries: [MenuEntry]
    let onItemActivated: (() -> Void)?
    @State private var path: [AnyHashable] = []
    @State private var highlighted: AnyHashable?

    private struct Level {
        var entries: [MenuEntry]
        var path: [AnyHashable] = []
        var title: String?
    }
    private var level: Level {
        var result = Level(entries: entries)
        for id in path {
            guard let entry = result.entries.first(where: { $0.id == id }),
                  case .submenu(let branch) = entry, branch.isEnabled else { break }
            result = Level(entries: branch.entries, path: result.path + [id], title: branch.title)
        }
        return result
    }

    var body: some View {
        let current = level
        let enabled = current.entries.filter { $0.row?.isEnabled == true }.map(\.id)
        let active = highlighted.flatMap { enabled.contains($0) ? $0 : nil } ?? enabled.first
        return ResolvedLayoutDirection { direction in
            RovingFocusHost(enabledItems: enabled, selection: { active }, onSelect: { highlighted = $0 },
                command: { event in
                    [Scancode.arrowUp, Scancode.arrowDown, Scancode.home, Scancode.end].contains(event.scancode)
                        ? SelectionNavigation.command(for: event.scancode) : nil
                }) {
                Box(direction: .column, alignItems: .stretch, spacing: 0) {
                    if let title = current.title {
                        Button(action: { path = Array(current.path.dropLast()); highlighted = nil }) {
                            Row(alignment: .center, spacing: 8) {
                                Icon(direction == .rtl ? UIIcons.Navigation.arrowRight : UIIcons.Navigation.arrowLeft, size: .small).accessibilityHidden()
                                Text(title).truncatingTail().font(.bodyStrong)
                            }
                        }.buttonStyle(TouchMenuRowStyle()).accessibilityLabel("Back")
                            .debugName("touch-menu-back")
                        Divider()
                    }
                    for entry in current.entries {
                        AnyView(entryView(entry, path: current.path, tabStop: active, direction: direction).id(entry.id))
                    }
                }.frame(minWidth: 0).debugName("touch-menu")
            }
        }
    }

    @ViewBuilder private func entryView(_ entry: MenuEntry, path: [AnyHashable], tabStop: AnyHashable?, direction: Direction) -> some View {
        if let row = entry.row {
            Button(role: row.role == .destructive ? .destructive : .normal,
                   isEnabled: row.isEnabled, isSelected: row.isSelected, action: {
                switch entry {
                case .item(let item): item.action(); onItemActivated?()
                case .submenu(let branch): self.path = path + [branch.id]; highlighted = nil
                default: break
                }
            }) {
                Row(alignment: .center, spacing: 12) {
                    if let icon = row.content.icon { Icon(icon, size: .medium).accessibilityHidden() }
                    Column(alignment: .leading, spacing: 4) {
                        if row.content.richTitle.isEmpty { Text(row.title).truncatingTail().font(.body) }
                        else { RichText(row.content.richTitle).displayOnly().font(.body) }
                        if let detail = row.content.detail {
                            Text(detail, lineLimit: 2).font(.caption).foregroundColor(.onSurfaceMuted)
                        }
                    }.frame(minWidth: 0).flex(1, shrink: 1, basis: 0)
                    if row.hasSubmenu { Icon(direction == .rtl ? UIIcons.Navigation.chevronLeft : UIIcons.Navigation.chevronRight, size: .small).accessibilityHidden() }
                    else if row.isSelected { Icon(UIIcons.Action.check, size: .small).accessibilityHidden() }
                    else if row.isLink { Icon(UIIcons.Action.externalLink, size: .small).accessibilityHidden() }
                }
            }.buttonStyle(TouchMenuRowStyle())
                .compositionLocal(ActionControlAccessibility.label, row.title)
                .modifier(RovingItemModifier(id: row.id, isTabStop: row.id == tabStop))
                .accessibility {
                    $0.role = row.isLink ? .link : .button; $0.label = row.title
                    $0.value = row.content.detail ?? ""; $0.state.isSelected = row.isSelected
                }
        } else {
            switch entry {
            case .label(_, let title): Text(title).font(.caption).foregroundColor(.onSurfaceMuted).padding(12)
            default: Divider()
            }
        }
    }
}

private struct TouchMenuRowStyle: ButtonStyle {
    func makeBody(configuration c: ButtonStyleConfiguration) -> some View {
        Box(direction: .column, alignItems: .stretch) { AnyView(c.label) }
            .padding(horizontal: 12, vertical: 12).frame(minWidth: 0, minHeight: 48)
            .flex(1, shrink: 1)
            .foregroundColor(!c.isEnabled ? c.theme.colors.onSurfaceMuted : c.role == .destructive ? c.theme.colors.error : c.theme.colors.onSurface)
            .background(c.isPressed ? c.theme.colors.stateLayerPressed : .clear).cornerRadius(10)
            .border(c.isFocused ? c.theme.colors.focusRing : .clear, width: c.isFocused ? 2 : 0)
            .opacity(c.isEnabled ? 1 : 0.55).animation(motion: .fast, value: c.interactionKey)
    }
}
