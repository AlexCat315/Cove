import Foundation
import Testing
import CovePlatformCore
import CoveUIRuntime
@testable import CoveUICompose

@Suite("Native menu", .serialized)
struct NativeMenuTests: CoveUIComposeSerializedSuite {
    private func nodes(_ node: Node?) -> [Node] {
        guard let node else { return [] }
        return [node] + node.children.flatMap { nodes($0) }
    }

    private func withScene(_ test: (ViewGraph, InteractionRegistry, FocusChain, PointerCapture, PortalStore) -> Void) {
        GlobalTestLock.locked {
            let oldRegistry = InteractionRegistryHolder.current
            let oldFocus = FocusChainHolder.current
            let oldCapture = PointerCaptureHolder.current
            let oldPortal = PortalStoreHolder.current
            let oldText = TextEnvironmentHolder.current
            let oldPresenter = NativeMenuPresenter.present
            let registry = InteractionRegistry(), focus = FocusChain(), capture = PointerCapture()
            let portal = PortalStore()
            InteractionRegistryHolder.current = registry
            FocusChainHolder.current = focus
            PointerCaptureHolder.current = capture
            PortalStoreHolder.current = portal
            TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer {
                InteractionRegistryHolder.current = oldRegistry
                FocusChainHolder.current = oldFocus
                PointerCaptureHolder.current = oldCapture
                PortalStoreHolder.current = oldPortal
                TextEnvironmentHolder.current = oldText
                NativeMenuPresenter.present = oldPresenter
            }
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            test(graph, registry, focus, capture, portal)
            graph.install(root: EmptyView())
            AnimatorScheduler.current.tick(deltaTime: 1)
        }
    }

    private func settle(_ graph: ViewGraph) {
        for _ in 0..<5 {
            graph.recomposer.commitAll()
            graph.computeLayout(width: 800, height: 600)
            AnimatorScheduler.current.tick(deltaTime: 1)
        }
    }

    private func click(_ node: Node, graph: ViewGraph, registry: InteractionRegistry,
                       focus: FocusChain, capture: PointerCapture) {
        let frame = node.absoluteFrame
        let dispatcher = EventDispatcher(tree: graph.tree, interactions: registry,
                                         capture: capture, focusChain: focus)
        let point = MouseButtonEvent(button: .left, x: Float(frame.midX), y: Float(frame.midY), clicks: 1)
        dispatcher.dispatch(.mouseButtonDown(point))
        graph.recomposer.commitAll()
        dispatcher.dispatch(.mouseButtonUp(point))
        graph.recomposer.commitAll()
    }

    @Test("MenuButton uses the native presenter and runs the chosen item")
    func nativeMenuButtonRunsChosen() {
        withScene { graph, registry, focus, capture, portal in
            var runs = 0
            var presentedSnapshot: NativeMenuSnapshot?
            NativeMenuPresenter.present = { snapshot, _ in
                presentedSnapshot = snapshot
                // Choose the first regular item.
                return snapshot.items.first(where: { $0.kind == .regular })?.id
            }
            graph.install(root: LayerRoot {
                MenuButton("Options", entries: [
                    .item(MenuItem(id: "save", title: "Save") { runs += 1 }),
                    .item(MenuItem(id: "export", title: "Export") { runs += 10 }),
                ])
            })
            settle(graph)
            guard let trigger = nodes(graph.tree.root).first(where: {
                $0.attachments[ButtonHost.markerKey] != nil
            }) else {
                Issue.record("menu button trigger was not mounted")
                return
            }
            click(trigger, graph: graph, registry: registry, focus: focus, capture: capture)
            settle(graph)
            #expect(presentedSnapshot != nil)
            #expect(presentedSnapshot?.items.count == 2)
            #expect(runs == 1)
            // The native path never opens a self-drawn portal.
            #expect(portal.entries.isEmpty)
        }
    }

    @Test("Snapshot encodes labels, separators, submenus, checks and disabled items")
    func snapshotEncodesTree() {
        let entries: [MenuEntry] = [
            .label(id: "group", title: "Workspace"),
            .item(MenuItem(id: "pin", title: "Pin", isSelected: true, action: {})),
            .separator(id: "sep"),
            .submenu(MenuSubmenu(id: "settings", title: "Settings", entries: [
                .item(MenuItem(id: "general", title: "General", isEnabled: false, action: {})),
            ])),
            .item(MenuItem(id: "delete", title: "Delete", isEnabled: false, action: {})),
        ]
        let built = NativeMenuBuilder.build(entries)
        let items = built.snapshot.items
        #expect(items.count == 5)
        // Label becomes a disabled regular item.
        #expect(items[0].title == "Workspace")
        #expect(items[0].isEnabled == false)
        // Checked item.
        #expect(items[1].isChecked == true)
        // Separator.
        guard case .separator = items[2].kind else { Issue.record("expected separator"); return }
        // Submenu with one disabled child.
        guard case .submenu(let branch) = items[3].kind else { Issue.record("expected submenu"); return }
        #expect(branch.items.first?.isEnabled == false)
        // Disabled regular item.
        #expect(items[4].isEnabled == false)
    }

    @Test("Shortcut parser decodes glyphs and word modifiers")
    func shortcutParser() {
        let mac = NativeMenuBuilder.parseShortcut("⌘⇧K")
        #expect(mac?.key == "K")
        #expect(mac?.meta == true)
        #expect(mac?.shift == true)
        #expect(mac?.ctrl == false)
        let win = NativeMenuBuilder.parseShortcut("Ctrl+Alt+F")
        #expect(win?.key == "F")
        #expect(win?.ctrl == true)
        #expect(win?.alt == true)
        #expect(NativeMenuBuilder.parseShortcut(nil) == nil)
    }

    @Test("Item icons rasterize into a template PNG")
    func iconRasterizes() {
        let entries: [MenuEntry] = [
            .item(MenuItem(id: "save", title: "Save",
                           content: MenuItemContent(icon: UIIcons.File.save), action: {})),
            .item(MenuItem(id: "plain", title: "Plain", action: {})),
        ]
        let items = NativeMenuBuilder.build(entries).snapshot.items
        guard let icon = items[0].icon else {
            Issue.record("expected a template icon"); return
        }
        #expect(icon.isTemplate == true)
        #expect(!icon.png.isEmpty)
        #expect(items[1].icon == nil)
    }
}
