import Foundation
import Testing
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Surface root", .serialized)
@MainActor
struct SurfaceRootTests: GuavaUIComposeSerializedSuite {
    private struct Probe: _PrimitiveView {
        func _makeNode() -> Node { Node() }
        func _makeLayoutNode() -> LayoutNode? { LayoutNode() }
        func _updateNode(_ node: Node) {
            node.attachments["body-size"] = node.textScaler.scale(16)
            node.attachments["heading-size"] = node.textScaler.scale(32)
        }
    }

    @Test("Nested surfaces replace the host ramp while retaining application scale")
    func scalingBoundaries() throws { try GlobalTestLock.locked {
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        defer { graph.install(root: EmptyView()) }
        var outer = UIEnvironment(); outer.textScale = .init(1.5)
        var inner = UIEnvironment()
        inner.textScale = .init(samples: [.init(size: 16, scaledSize: 32), .init(size: 32, scaledSize: 48)])
        graph.install(root: SurfaceRoot(outer) { SurfaceRoot(inner) { Probe() } }.textScaler(.init(1.25)))
        graph.computeLayout(width: 393, height: 852)
        func find(_ node: Node) -> Node? {
            if node.attachments["body-size"] != nil { return node }
            return node.children.lazy.compactMap(find).first
        }
        let probe = try #require(graph.tree.root.flatMap(find))
        #expect(probe.attachments["body-size"] as? Float == 40)
        #expect(probe.attachments["heading-size"] as? Float == 60)
        let ramp = TextScaler().applyingSystemScale(inner.textScale).multiplied(by: 1.25)
        #expect(ramp.scale(16) == 40 && ramp.scale(32) == 60)
    } }

    @Test("Safe area is consumed once and the IME replaces the bottom floor")
    func keyboardAndSafeArea() throws { try GlobalTestLock.locked {
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        defer { graph.install(root: EmptyView()) }
        var surface = UIEnvironment()
        surface.safeArea.top = 59; surface.safeArea.leading = 12
        surface.safeArea.bottom = 34; surface.safeArea.trailing = 20
        for keyboard: Float in [0, 336, 12, 0, .nan] {
            surface.keyboard.height = keyboard
            graph.install(root: SurfaceRoot(surface) {
                SafeArea(.top) {
                    SafeArea { Box {}.flex(1, shrink: 1, basis: 0).debugName("surface-fill") }
                        .flex(1, shrink: 1, basis: 0)
                }
            })
            for _ in 0..<3 { graph.recomposer.commitAll(); graph.computeLayout(width: 393, height: 852) }
            let frame = try #require(graph.layoutSnapshot().first { $0.debugName == "surface-fill" }?.absoluteFrame)
            let bottom: Float = keyboard.isFinite ? max(34, keyboard) : 34
            #expect(frame == CGRect(x: 12, y: 59, width: 361, height: CGFloat(852 - 59 - bottom)))
        }
    } }
}
