import Foundation
import Testing
@testable import CoveUIScene

@Suite("Flat render plan", .serialized)
@MainActor
struct FlatRenderPlanTests {
    private final class Resource: NodeResource {
        let unmounted: () -> Void
        init(_ unmounted: @escaping () -> Void) { self.unmounted = unmounted }
        func mount(node: Node) {}
        func unmount(node: Node) { unmounted() }
    }

    @Test("Iterative resource teardown preserves child-before-parent and sibling order")
    func resourceOrder() {
        let root = Node(), group = Node(), first = Node(), last = Node(), grandchild = Node()
        root.addChild(group); group.addChild(first); group.addChild(last); first.addChild(grandchild)
        var released: [ElementID] = []
        for node in [group, first, last, grandchild] {
            let id = node.id
            node.addResource(Resource { released.append(id) })
        }
        root.removeChild(group)
        #expect(released == [grandchild.id, first.id, last.id, group.id])
    }

    @Test("Deep render plans install, paint, skip cached subtrees and tear down without recursive traversal")
    func deepTree() {
        let leaf = Node()
        leaf.frame = CGRect(x: 0, y: 0, width: 8, height: 8); leaf.backgroundColor = .white
        var nodes = [leaf]
        for _ in 0..<2_048 {
            let parent = Node()
            parent.frame = CGRect(x: 0, y: 0, width: 20, height: 20)
            parent.addChild(nodes.last!); nodes.append(parent)
        }
        let tree = RenderTree(); tree.install(rootNode: nodes.last!)
        #expect(tree.paintPlan().subtreeEnds.allSatisfy { $0 == nodes.count })
        let renderer = LayerAwareNodeRenderer()
        for _ in 0..<3 {
            let list = DrawList(); renderer.render(tree: tree, into: list)
            #expect(list.vertices.count == 4)
        }
        let scene = NodeTree(); scene.root = nodes.last!
        scene.flush()
        leaf.markDirty()
        #expect(nodes.allSatisfy { $0.isDirty && $0.renderDirty })
        scene.flush()
        #expect(nodes.allSatisfy { !$0.isDirty && !$0.renderDirty })
        tree.install(rootNode: nodes.last!)
        #expect(tree.flatDrawOrder().count == nodes.count)
        tree.tearDown(node: nodes.last!)
        #expect(tree.flatDrawOrder().isEmpty && tree.objectCount == 0)
        // Release scene ownership from the top, keeping every node alive until detached.
        for node in nodes.reversed() { for child in node.children { node.removeChild(child) } }
    }

    @Test("A cached plan observes zIndex changes, including returning to zero")
    func zOrderInvalidation() {
        let root = Node(), a = Node(), b = Node(), grandchild = Node()
        root.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        a.frame = CGRect(x: 0, y: 0, width: 40, height: 40); a.backgroundColor = .white
        b.frame = a.frame; b.backgroundColor = .black
        root.addChild(a); root.addChild(b); a.addChild(grandchild)
        let tree = RenderTree(); tree.install(rootNode: root)
        #expect(tree.flatDrawOrder().map(\.elementID) == [root.id, a.id, grandchild.id, b.id])
        a.zIndex = 1
        #expect(tree.flatDrawOrder().map(\.elementID) == [root.id, b.id, a.id, grandchild.id])
        a.zIndex = 0
        #expect(tree.flatDrawOrder().map(\.elementID) == [root.id, a.id, grandchild.id, b.id])
        for z: Float in [1, -1, 0] {
            a.zIndex = z
            let actual = DrawList(), expected = DrawList()
            LayerAwareNodeRenderer().render(tree: tree, into: actual)
            NodeRenderer().render(root: root, into: expected)
            #expect(actual.vertices.map(\.color) == expected.vertices.map(\.color))
        }
    }

    @Test("Subtree ranges survive reconcile, reorder, teardown and root replacement")
    func structuralInvalidation() {
        let root = Node(), a = Node(), b = Node(), child = Node()
        root.addChild(a); root.addChild(b); a.addChild(child)
        let tree = RenderTree(); tree.install(rootNode: root)
        #expect(tree.paintPlan().subtreeEnds == [4, 3, 3, 4])
        root.reorderChildren([b, a]); tree.reconcileChildren(of: root)
        #expect(tree.flatDrawOrder().map(\.elementID) == [root.id, b.id, a.id, child.id])
        #expect(tree.paintPlan().subtreeEnds == [4, 2, 4, 4])
        root.removeChild(a); tree.reconcileChildren(of: root)
        #expect(tree.paintPlan().subtreeEnds == [2, 2])
        #expect(tree.renderObject(for: child) == nil)
        tree.tearDown(node: b)
        #expect(tree.flatDrawOrder().map(\.elementID) == [root.id])
        tree.install(rootNode: child)
        #expect(tree.flatDrawOrder().map(\.elementID) == [child.id])
    }

    @Test("Linear enter/exit ordering preserves clips, fades, overlays, scales and cached siblings")
    func painterParity() {
        let root = Node(), clipped = Node(), leaf = Node(), sibling = Node()
        root.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        clipped.frame = CGRect(x: 20, y: 20, width: 100, height: 100)
        clipped.clipsToBounds = true; clipped.cornerRadius = 16
        clipped.opacity = 0.7; clipped.subtreeOpacity = 0.6
        clipped.backgroundColor = .white; clipped.backdropBlur = 8
        leaf.frame = CGRect(x: 0, y: 0, width: 120, height: 120)
        leaf.draw = { list, origin in
            list.addRect(UIRect(x: Float(origin.x), y: Float(origin.y), width: 120, height: 120), color: .black)
        }
        clipped.overlayDraw = { list, origin in
            list.addRect(UIRect(x: Float(origin.x) + 20, y: Float(origin.y) + 20, width: 10, height: 10), color: .white)
        }
        sibling.frame = CGRect(x: 200, y: 20, width: 40, height: 40); sibling.backgroundColor = .black
        root.addChild(clipped); clipped.addChild(leaf); root.addChild(sibling)
        let tree = RenderTree(); tree.install(rootNode: root)
        let renderer = LayerAwareNodeRenderer()
        for scale: Float in [1, 0.75, 1] {
            clipped.transform = NodeTransform(scale: scale, anchor: CGPoint(x: 0.5, y: 0.5))
            for _ in 0..<3 {
                let actual = DrawList(), expected = DrawList()
                renderer.render(tree: tree, into: actual); NodeRenderer().render(root: root, into: expected)
                #expect(actual.vertices.map { [$0.posX, $0.posY, $0.u, $0.v] } ==
                        expected.vertices.map { [$0.posX, $0.posY, $0.u, $0.v] })
                #expect(actual.vertices.map(\.color) == expected.vertices.map(\.color))
                #expect(actual.indices == expected.indices)
                #expect(actual.batches.compactMap(\.backdrop) == expected.batches.compactMap(\.backdrop))
            }
        }
    }

    @Test("Translated cached leaves remain clipped as they cross rounded corners")
    func roundedScrollParity() {
        let root = Node(), leaf = Node()
        root.frame = CGRect(x: 0, y: 0, width: 100, height: 100)
        root.clipsToBounds = true; root.cornerRadius = 24
        leaf.frame = CGRect(x: 2, y: 40, width: 20, height: 20)
        leaf.draw = { list, origin in
            list.addRect(UIRect(x: Float(origin.x), y: Float(origin.y), width: 20, height: 20), color: .white)
        }
        root.addChild(leaf)
        let tree = RenderTree(); tree.install(rootNode: root)
        let renderer = LayerAwareNodeRenderer()
        for offset in [0, 4, 20, 32, 42, 100, 4] {
            root.contentOffset.y = CGFloat(offset)
            for _ in 0..<3 {
                let actual = DrawList(), expected = DrawList()
                renderer.render(tree: tree, into: actual); NodeRenderer().render(root: root, into: expected)
                // Offscreen cached leaves may omit unreferenced vertices.
                let actualPoints = actual.indices.map { actual.vertices[Int($0)] }
                let expectedPoints = expected.indices.map { expected.vertices[Int($0)] }
                #expect(actualPoints.map { [$0.posX, $0.posY] } == expectedPoints.map { [$0.posX, $0.posY] })
                #expect(actualPoints.map(\.color) == expectedPoints.map(\.color))
            }
        }
    }
}
