import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import CovePlatformCore
import CoveUIRuntime
import CoveUIScene
@testable import CoveUICompose

/// End-to-end selection over the mounted tree: a precise pointer drags a range,
/// a finger long-presses a word, and every label in a `textSelection()` region
/// shares one scope, so a drag can run from one label into the next and copy as
/// a single string.
@Suite("Label text selection", .serialized)
struct LabelSelectionTests: CoveUIComposeSerializedSuite {

    private final class Counter {
        var taps = 0
    }

    // MARK: - Harness

    private func nodes(_ root: Node?) -> [Node] {
        guard let root else { return [] }
        return [root] + root.children.flatMap(nodes)
    }

    private func withScene(_ test: (ViewGraph, EventDispatcher, PortalStore) -> Void) {
        GlobalTestLock.locked {
            let oldRegistry = InteractionRegistryHolder.current
            let oldFocus = FocusChainHolder.current
            let oldCapture = PointerCaptureHolder.current
            let oldText = TextEnvironmentHolder.current
            let oldClipboard = ClipboardHolder.write
            let registry = InteractionRegistry(), focus = FocusChain(), capture = PointerCapture()
            let previousStore = PortalStoreHolder.current
            let portals = PortalStore()
            PortalStoreHolder.current = portals
            InteractionRegistryHolder.current = registry
            FocusChainHolder.current = focus
            PointerCaptureHolder.current = capture
            TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer {
                InteractionRegistryHolder.current = oldRegistry
                FocusChainHolder.current = oldFocus
                PointerCaptureHolder.current = oldCapture
                TextEnvironmentHolder.current = oldText
                ClipboardHolder.write = oldClipboard
                PortalStoreHolder.current = previousStore
            }
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            let dispatcher = EventDispatcher(tree: graph.tree, interactions: registry,
                                             capture: capture, focusChain: focus)
            test(graph, dispatcher, portals)
            graph.install(root: EmptyView())
        }
    }

    private func labels(_ graph: ViewGraph) -> [Node] {
        nodes(graph.tree.root)
            .filter { $0.accessibility?.role == .staticText }
            .sorted { $0.absoluteFrame.minY < $1.absoluteFrame.minY }
    }

    private func drag(_ dispatcher: EventDispatcher, from: CGPoint, to: CGPoint,
                      kind: PointerKind = .mouse) {
        let down = MouseButtonEvent(button: .left, x: Float(from.x), y: Float(from.y),
                                    clicks: 1, kind: kind)
        dispatcher.dispatch(.mouseButtonDown(down))
        for step in 1...4 {
            let t = Float(step) / 4
            let point = CGPoint(x: from.x + (to.x - from.x) * CGFloat(t),
                                y: from.y + (to.y - from.y) * CGFloat(t))
            dispatcher.dispatch(.mouseMotion(MouseMotionEvent(x: Float(point.x), y: Float(point.y),
                                                              deltaX: 1, deltaY: 0, kind: kind)))
        }
        dispatcher.dispatch(.mouseButtonUp(MouseButtonEvent(button: .left, x: Float(to.x),
                                                            y: Float(to.y), clicks: 1, kind: kind)))
    }

    private func copy(_ dispatcher: EventDispatcher) -> String? {
        var copied: String?
        ClipboardHolder.write = { copied = $0 }
        dispatcher.dispatch(.keyDown(KeyEvent(scancode: Scancode.c, keycode: 0,
                                              modifiers: .gui, isRepeat: false)))
        dispatcher.dispatch(.keyUp(KeyEvent(scancode: Scancode.c, keycode: 0,
                                            modifiers: .gui, isRepeat: false)))
        return copied
    }

    private struct LabelHarness: View {
        let text: String
        var body: some View {
            Text(text).frame(width: 320, height: 40).textSelection()
        }
    }

    private struct TwoLineHarness: View {
        var body: some View {
            Column(alignment: .leading, spacing: 0) {
                Text("one").frame(width: 200, height: 24)
                Text("two").frame(width: 200, height: 24)
            }
            .frame(width: 240, height: 80)
            .textSelection()
        }
    }

    private struct ButtonHarness: View {
        let counter: Counter
        var body: some View {
            Button(action: { counter.taps += 1 }) { Text("Go") }
                .frame(width: 200, height: 40)
                .textSelection()
        }
    }

    private struct OptOutHarness: View {
        var body: some View {
            Box(direction: .column, alignItems: .stretch, spacing: 0) {
                Text("one").frame(width: 200, height: 24)
                Text("two").frame(width: 200, height: 24).textSelection(false)
            }
            .frame(width: 240, height: 80)
            .textSelection()
        }
    }

    private struct BareHarness: View {
        var body: some View { Text("bare").frame(width: 200, height: 24) }
    }

    // MARK: - Dragging

    @Test("Dragging a label selects the dragged range and Cmd+C copies it")
    func dragCopiesRange() {
        withScene { graph, dispatcher, _ in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 80)
            let label = labels(graph).first
            #expect(label != nil)
            guard let label else { return }
            let frame = label.absoluteFrame
            drag(dispatcher,
                 from: CGPoint(x: frame.minX + 1, y: frame.midY),
                 to: CGPoint(x: frame.maxX - 1, y: frame.midY))
            #expect(copy(dispatcher) == "hello world")
        }
    }

    @Test("Shift+Right extends a caret selection one character")
    func shiftArrowExtends() {
        withScene { graph, dispatcher, _ in
            graph.install(root: LabelHarness(text: "hello"))
            graph.computeLayout(width: 360, height: 80)
            guard let label = labels(graph).first else { return #expect(Bool(false)) }
            let frame = label.absoluteFrame
            let down = MouseButtonEvent(button: .left, x: Float(frame.minX + 1),
                                        y: Float(frame.midY), clicks: 1)
            dispatcher.dispatch(.mouseButtonDown(down))
            dispatcher.dispatch(.mouseButtonUp(down))
            dispatcher.dispatch(.keyDown(KeyEvent(scancode: Scancode.arrowRight, keycode: 0,
                                                  modifiers: .shift, isRepeat: false)))
            #expect(copy(dispatcher) == "h")
        }
    }

    @Test("One drag runs from the first label into the second")
    func dragSpansLabels() {
        withScene { graph, dispatcher, _ in
            graph.install(root: TwoLineHarness())
            graph.computeLayout(width: 260, height: 100)
            let lines = labels(graph)
            #expect(lines.count == 2)
            guard lines.count == 2 else { return }
            let first = lines[0].absoluteFrame, second = lines[1].absoluteFrame
            drag(dispatcher,
                 from: CGPoint(x: first.minX + 1, y: first.midY),
                 to: CGPoint(x: second.maxX - 1, y: second.midY))
            #expect(copy(dispatcher) == "one\ntwo")
        }
    }

    @Test("A finger long-press selects the word under it")
    func longPressSelectsWord() {
        withScene { graph, dispatcher, _ in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 80)
            guard let label = labels(graph).first else { return #expect(Bool(false)) }
            let frame = label.absoluteFrame
            let point = CGPoint(x: frame.minX + 2, y: frame.midY)
            let event = MouseButtonEvent(button: .left, x: Float(point.x), y: Float(point.y),
                                         clicks: 1, kind: .touch)
            dispatcher.dispatch(.mouseButtonDown(event))
            // The press must be held long enough to count as a long press.
            Thread.sleep(forTimeInterval: 0.5)
            dispatcher.dispatch(.mouseButtonUp(event))
            #expect(copy(dispatcher) == "hello")
        }
    }

    // MARK: - Controls keep their clicks

    @Test("A label inside a button leaves the press to the button")
    func buttonKeepsClick() {
        withScene { graph, dispatcher, _ in
            let counter = Counter()
            graph.install(root: ButtonHarness(counter: counter))
            graph.computeLayout(width: 260, height: 80)
            let button = nodes(graph.tree.root).first { $0.attachments[ButtonHost.pressedKey] != nil }
            #expect(button != nil)
            guard let button else { return }
            let frame = button.absoluteFrame
            let event = MouseButtonEvent(button: .left, x: Float(frame.midX),
                                         y: Float(frame.midY), clicks: 1)
            dispatcher.dispatch(.mouseButtonDown(event))
            dispatcher.dispatch(.mouseButtonUp(event))
            #expect(counter.taps == 1)
        }
    }

    // MARK: - Region scope

    @Test("A label outside any selection region stays inert")
    func bareLabelStaysInert() {
        withScene { graph, _, _ in
            graph.install(root: BareHarness())
            graph.computeLayout(width: 260, height: 60)
            guard let label = labels(graph).first else { return #expect(Bool(false)) }
            #expect(label.isHitTestable == false)
            #expect(label.isFocusable == false)
        }
    }

    @Test("textSelection(false) makes a subtree inert inside a region")
    func disabledSubtreeIsInert() {
        withScene { graph, _, _ in
            graph.install(root: OptOutHarness())
            graph.computeLayout(width: 260, height: 100)
            let lines = labels(graph)
            #expect(lines.count == 2)
            guard lines.count == 2 else { return }
            #expect(lines[0].isHitTestable)
            #expect(lines[1].isHitTestable == false)
        }
    }

    @Test("A region ignores secondary clicks that miss its text")
    func regionIgnoresPressesAwayFromText() {
        withScene { graph, dispatcher, _ in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 160)
            guard let label = labels(graph).first else { return #expect(Bool(false)) }
            guard let scope = label.compositionValue(of: TextSelectionScope.key) else {
                return #expect(Bool(false))
            }
            let frame = label.absoluteFrame
            // Well below the label: the press belongs to whatever is underneath,
            // so the region must not select a word for it.
            let away = MouseButtonEvent(button: .right, x: Float(frame.minX + 4),
                                        y: Float(frame.maxY + 60), clicks: 1)
            dispatcher.dispatch(.mouseButtonDown(away))
            #expect(scope.hasSelection == false)

            let onText = MouseButtonEvent(button: .right, x: Float(frame.minX + 2),
                                          y: Float(frame.midY), clicks: 1)
            dispatcher.dispatch(.mouseButtonDown(onText))
            #expect(scope.hasSelection)
            #expect(scope.selectedText == "hello")
        }
    }

    @Test("textUnder reports the paragraphs a window point lands on")
    func textUnderReportsHit() {
        withScene { graph, _, _ in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 160)
            guard let label = labels(graph).first,
                  let scope = label.compositionValue(of: TextSelectionScope.key) else {
                return #expect(Bool(false))
            }
            let frame = label.absoluteFrame
            #expect(scope.textUnder(CGPoint(x: frame.midX, y: frame.midY)))
            #expect(scope.textUnder(CGPoint(x: frame.midX, y: frame.maxY + 40)) == false)
        }
    }

    @Test("A drag paints the theme highlight behind the selected glyphs")
    func selectionPaintsHighlight() {
        withScene { graph, dispatcher, _ in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 80)
            guard let label = labels(graph).first, let root = graph.tree.root else {
                return #expect(Bool(false))
            }
            let frame = label.absoluteFrame
            drag(dispatcher,
                 from: CGPoint(x: frame.minX + 1, y: frame.midY),
                 to: CGPoint(x: frame.maxX - 1, y: frame.midY))
            let list = DrawList()
            NodeRenderer().render(root: root, into: list)
            let highlight = label.theme.colors.selection.rgba8
            #expect(list.vertices.contains { $0.color == highlight })
        }
    }

    @Test("A secondary click on text presents the selection menu")
    func secondaryClickPresentsMenu() {
        withScene { graph, dispatcher, portals in
            graph.install(root: LabelHarness(text: "hello world"))
            graph.computeLayout(width: 360, height: 80)
            guard let label = labels(graph).first else { return #expect(Bool(false)) }
            #expect(portals.entries.isEmpty)
            let frame = label.absoluteFrame
            let right = MouseButtonEvent(button: .right, x: Float(frame.minX + 2),
                                         y: Float(frame.midY), clicks: 1)
            dispatcher.dispatch(.mouseButtonDown(right))
            graph.recomposer.commitAll()
            graph.computeLayout(width: 360, height: 80)
            #expect(portals.entries.count == 1)
        }
    }

    // MARK: - Chrome geometry

    @Test("Handle centres sit below the first line and above the last")
    func handleCenters() {
        let caret = CGRect(x: 10, y: 20, width: 0, height: 16)
        let start = TextSelectionChrome.handleCenter(caret: caret, isStart: true)
        let end = TextSelectionChrome.handleCenter(caret: caret, isStart: false)
        #expect(start == CGPoint(x: 10, y: 36 + TextSelectionChrome.handleRadius))
        #expect(end == CGPoint(x: 10, y: 20 - TextSelectionChrome.handleRadius))
    }

    @Test("A press within the slop grabs an endpoint, a press outside grabs none")
    func endpointHitTest() {
        let start = CGRect(x: 0, y: 0, width: 0, height: 16)
        let end = CGRect(x: 40, y: 0, width: 0, height: 16)
        let startCenter = TextSelectionChrome.handleCenter(caret: start, isStart: true)
        let endCenter = TextSelectionChrome.handleCenter(caret: end, isStart: false)
        #expect(TextSelectionChrome.endpoint(at: startCenter, start: start, end: end) == .start)
        #expect(TextSelectionChrome.endpoint(at: endCenter, start: start, end: end) == .end)
        let middle = CGPoint(x: 20, y: 8)
        #expect(TextSelectionChrome.endpoint(at: middle, start: start, end: end) == nil)
    }
}
