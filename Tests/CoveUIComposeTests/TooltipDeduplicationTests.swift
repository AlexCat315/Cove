import Testing
import CoveUIRuntime
@testable import CoveUICompose

/// A tooltip that only repeats the control's drawn text tells the pointer user
/// nothing, so it must stay hidden. Select and Combobox triggers hid their
/// label from accessibility while publishing their own name, which made the
/// old visible-text check come back empty and let the duplicate through.
@Suite("Tooltip deduplication", .serialized)
struct TooltipDeduplicationTests: CoveUIComposeSerializedSuite {
    private func findButtonHost(_ root: Node) -> Node? {
        if root.attachments[ButtonHost.markerKey] != nil { return root }
        for child in root.children {
            if let host = findButtonHost(child) { return host }
        }
        return nil
    }

    private func tooltipAddsText(root: some View) throws -> Bool {
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: root)
        graph.computeLayout(width: 320, height: 60)
        let host = try #require(findButtonHost(tree.root!))
        return ButtonHost.tooltipAddsText(to: host)
    }

    @Test("A tooltip equal to the button's label is suppressed")
    func repeatedLabelIsSuppressed() throws {
        try GlobalTestLock.locked {
            let addsText = try tooltipAddsText(root: Button("Medium", tooltip: "Medium") {})
            #expect(addsText == false)
        }
    }

    @Test("A hidden label still counts as drawn text")
    func hiddenLabelIsSuppressed() throws {
        try GlobalTestLock.locked {
            let button = Button(tooltip: "Medium", action: {}) {
                Text("Medium").accessibilityHidden()
            }
            let addsText = try tooltipAddsText(root: button)
            #expect(addsText == false)
        }
    }

    @Test("A tooltip that adds information survives")
    func distinctTooltipIsKept() throws {
        try GlobalTestLock.locked {
            let addsText = try tooltipAddsText(root: Button("Medium", tooltip: "Canvas text size") {})
            #expect(addsText == true)
        }
    }

    @Test("A Select trigger does not repeat its selected value")
    func selectTriggerIsQuiet() throws {
        try GlobalTestLock.locked {
            struct Sample: View {
                @State var value = 1
                var body: some View {
                    Select(selection: $value,
                           options: [SelectOption(value: 1, label: "Medium")],
                           width: 160)
                }
            }
            let tree = NodeTree()
            let graph = ViewGraph(tree: tree, recomposer: Recomposer())
            graph.install(root: Sample())
            for _ in 0..<3 {
                graph.recomposer.commitAll()
                graph.computeLayout(width: 200, height: 60)
            }
            let host = try #require(findButtonHost(tree.root!))
            // The trigger's visible value is its name and value, never help:
            // repeating it as help would make the system describe it twice.
            #expect(host.accessibility?.help == "")
            #expect(host.firstResource(TooltipSession.self) == nil)
            #expect(ButtonHost.tooltipAddsText(to: host) == false)
        }
    }

    @Test("A tip stays hidden when its text becomes redundant after hover begins")
    func lateDeduplicationHidesWaitingTip() {
        GlobalTestLock.locked {
            let oldPortal = PortalStoreHolder.current
            let portal = PortalStore()
            PortalStoreHolder.current = portal
            defer { PortalStoreHolder.current = oldPortal }
            let node = Node()
            node.addResource(PortalResource())
            let session = TooltipSession()
            node.addResource(session)
            session.mount(node: node)
            var canShow = false
            var options = TooltipOptions()
            options.delay = 0
            session.configure(content: AnyView(Text("Medium")), options: options,
                              focused: false, canShow: { canShow })
            session.setHovered(true)
            #expect(session.isPresented == false)
            #expect(portal.entries.isEmpty)
            canShow = true
            session.setHovered(false)
            session.setHovered(true)
            #expect(session.isPresented == true)
            #expect(portal.entries.count == 1)
        }
    }
}
