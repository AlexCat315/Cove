import Testing
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Recomposition paint invalidation", .serialized)
struct RecompositionPaintTests: GuavaUIComposeSerializedSuite {
    private final class Counter { var paints = 0 }
    private struct Painter: _PrimitiveView {
        let value: Int
        let counter: Counter
        func _makeNode() -> Node { Node() }
        func _makeLayoutNode() -> LayoutNode? { LayoutNode() }
        func _updateNode(_ node: Node) {
            node.updateDraw(identity: value) { list, _ in
                counter.paints += 1
                list.addRect(UIRect(x: 0, y: 0, width: 10, height: 10), color: .white)
            }
        }
    }
    private final class Harness: View {
        @State var tick = 0
        @State var value = 0
        let counter = Counter()
        var body: some View {
            let _ = tick
            Painter(value: value, counter: counter).frame(width: 10, height: 10)
        }
    }

    @Test func unchangedOutputReusesGeometryWithDiagnosticsEnabled() { GlobalTestLock.locked {
        let harness = Harness()
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        graph.install(root: harness)
        graph.computeLayout(width: 100, height: 100)
        let renderer = LayerAwareNodeRenderer()
        for _ in 0..<3 { renderer.render(tree: graph.renderTree, into: DrawList()) }
        graph.tree.flush()
        let paints = harness.counter.paints
        harness.tick += 1
        graph.recomposer.commitAll()
        renderer.render(tree: graph.renderTree, into: DrawList())
        #expect(harness.counter.paints == paints)
        #expect(!graph.tree.hasRenderUpdates)
        harness.value += 1
        graph.recomposer.commitAll()
        renderer.render(tree: graph.renderTree, into: DrawList())
        #expect(harness.counter.paints > paints)
    } }
}
