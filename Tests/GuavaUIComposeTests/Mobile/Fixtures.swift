import GuavaUIRuntime
@testable import GuavaUICompose

final class MobileFixture: View {
    enum Sample { case button, tabs, list, menu, context, popover, dialog, hoverCard, navigation, sections }
    struct Entry: Identifiable { let id: Int }
    let sample: Sample
    @State var surface: UIEnvironment
    @State var controlSize: ControlSize = .regular
    @State var selection = 0
    @State var selectedRow: Int? = 0
    @State var calls = 0
    @State var presented = false
    @State var documentTabs = false
    @State var extraTabs = false
    @State var variant = TabVariant.bar
    @State var route: [Int] = []
    @State var activated: Int?

    init(_ sample: Sample) {
        self.sample = sample
        var surface = UIEnvironment()
        surface.idiom = .phone; surface.input = [.touch]
        surface.viewport.width = 390; surface.viewport.height = 844
        surface.safeArea.top = 24; surface.safeArea.bottom = 24
        surface.reducesMotion = true
        _surface = State(wrappedValue: surface)
    }

    var entries: [MenuEntry] {
        [.item(MenuItem(id: "open", title: "Open") { self.calls += 1 }),
         .submenu(MenuSubmenu(id: "more", title: "More", entries: [
            .item(MenuItem(id: "export", title: "Export") { self.calls += 10 })])),
         .item(MenuItem(id: "delete", title: "Delete", role: .destructive) { self.calls += 100 }),
         .item(MenuItem(id: "disabled", title: "Unavailable", isEnabled: false) { self.calls += 1_000 })]
    }

    var body: some View {
        SurfaceRoot(surface) {
            LayerRoot {
                SafeArea {
                    Box(direction: .column, alignItems: .stretch, spacing: 0) { content }
                        .frame(minWidth: 0, minHeight: 0).flex(1, shrink: 1, basis: 0)
                }.flex(1, shrink: 1, basis: 0).frame(minWidth: 0, minHeight: 0)
            }
        }.controlSize(controlSize)
    }

    @ViewBuilder private var content: some View {
        switch sample {
        case .button:
            Button("Action") { self.calls += 1 }.accessibilityLabel("Action").debugName("mobile-action")
        case .tabs:
            TabView(selection: $selection, tabs: navigationTabs) { $0.variant = variant }.flex(1, shrink: 1, basis: 0)
        case .list:
            List(Array(0..<30), id: \.self, selection: $selectedRow) { value, _ in
                ListRow("Long title \(value) with enough words to truncate on a phone", configure: {
                    $0.subtitle = "Secondary detail"; $0.showsDisclosure = true
                    if value == 2 { $0.role = .destructive }
                }, leading: { Icon(UIIcons.File.file, size: .medium) }, trailing: {
                    Text("Synced").truncatingTail().font(.caption)
                })
            }.flex(1, shrink: 1, basis: 0)
        case .menu:
            MenuButton("Commands", entries: entries, width: 220)
        case .context:
            ContextMenu(entries: { self.entries }, onOpen: { self.calls += 10_000 }) {
                Button("Target") { self.calls += 1_000_000 }.accessibilityLabel("Target")
            }
        case .popover: popoverContent
        case .dialog:
            Button("Show") { self.presented = true }.accessibilityLabel("Show")
            Dialog(isPresented: $presented, title: "Rename") {
                Text("Dialog body")
            } footer: {
                Button("Save") { self.calls += 1; self.presented = false }.accessibilityLabel("Save")
            }
        case .hoverCard:
            HoverCard(label: { Text("Profile") }) {
                Button("Follow") { self.calls += 1 }.accessibilityLabel("Follow")
            }
        case .navigation:
            NavigationStack(path: $route) {
                Button("Open") { self.route.append(1) }.accessibilityLabel("Open")
            } destination: { value in
                Text("Page \(value)").debugName("detail-page")
            }.flex(1, shrink: 1, basis: 0)
        case .sections:
            SectionedList([ListSection("recent", title: "Recent", items: (0..<4).map(Entry.init))],
                          selection: $selectedRow, onActivate: { self.activated = $0.id }) { entry, _ in
                ListRow("Entry \(entry.id)")
            }.flex(1, shrink: 1, basis: 0)
        }
    }

    private var navigationTabs: [TabItem<Int>] {
        let base: [TabItem<Int>] = [
            TabItem("Home", id: 0, configure: {
                $0.icon = UIIcons.File.file; $0.badge = "3"
                if documentTabs { $0.onClose = { self.calls += 1 } }
            }) { Text("Home panel").debugName("home-panel") },
            TabItem("Search", id: 1, configure: { $0.icon = UIIcons.Action.search }) { Text("Search panel") },
            TabItem("Unavailable", id: 2, isEnabled: false) { EmptyView() }
        ]
        guard extraTabs else { return base }
        return base + (3..<9).map { value in TabItem("Page \(value)", id: value) { Text("Page \(value)") } }
    }

    private var popoverContent: some View {
        var popover = Popover(isPresented: $presented, width: 240, label: { Text("Details") }) {
            Column(alignment: .leading, spacing: 12) {
                Text("Popover content")
                Button("Commit") { self.calls += 1; self.presented = false }.accessibilityLabel("Commit")
            }
        }
        popover.triggerDescription = .init(label: "Details")
        return popover
    }
}
