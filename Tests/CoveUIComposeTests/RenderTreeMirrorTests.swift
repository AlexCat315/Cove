import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import CoveUIRuntime
@testable import CoveUICompose
@testable import CoveUIComposeCore

/// Phase 4a foundation tests.
///
/// Verifies the RenderObject mirror is built, kept in sync across reconcile,
/// and classifies layer roots from Node properties (clip, opacity, shadow).
@Suite("Phase 4a RenderTree mirror")
struct RenderTreeMirrorTests {

    /// Simple primitive whose `_makeNode` accepts an externally-supplied
    /// configurator so individual tests can flip layer-root inputs without
    /// inventing a primitive per test.
    struct _LayerNode: _PrimitiveView {
        let configure: (Node) -> Void
        func _makeNode() -> Node {
            let n = Node()
            configure(n)
            return n
        }
        func _updateNode(_ node: Node) { configure(node) }
    }

    @Test("Install mirrors the entire Node tree, sets back-pointers, and counts the root")
    func installBuildsMirror() { GlobalTestLock.locked {
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: _LayerNode { _ in })

        let root = tree.root!
        let renderRoot = graph.renderTree.root
        #expect(renderRoot != nil)
        #expect(renderRoot?.node === root)
        #expect(root.renderObject === renderRoot)
        #expect(graph.renderTree.objectCount >= 1)
        #expect(renderRoot?.isLayerRoot == true) // root always
    } }

    @Test("Plain primitive without clip/opacity/shadow is not a layer root")
    func nonLayerLeafIsNotALayerRoot() { GlobalTestLock.locked {
        struct Wrapper: View {
            var body: some View {
                _LayerNode { _ in }
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Wrapper())

        let leaf = tree.root!.children.first!.children.first!
        let leafObj = graph.renderTree.renderObject(for: leaf)!
        #expect(leafObj.isLayerRoot == false)
    } }

    @Test("clipsToBounds promotes a node to layer root")
    func clipPromotesToLayerRoot() { GlobalTestLock.locked {
        struct Wrapper: View {
            var body: some View {
                _LayerNode { node in node.clipsToBounds = true }
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Wrapper())
        let leaf = tree.root!.children.first!.children.first!
        let obj = graph.renderTree.renderObject(for: leaf)!
        #expect(obj.isLayerRoot == true)
    } }

    @Test("Sub-1.0 opacity promotes a node to layer root")
    func opacityPromotesToLayerRoot() { GlobalTestLock.locked {
        struct Wrapper: View {
            var body: some View {
                _LayerNode { node in node.opacity = 0.5 }
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Wrapper())
        let leaf = tree.root!.children.first!.children.first!
        let obj = graph.renderTree.renderObject(for: leaf)!
        #expect(obj.isLayerRoot == true)
    } }

    @Test("Reconcile keeps RenderObjects for reused children")
    func reconcileReusesRenderObjects() { GlobalTestLock.locked {
        struct Initial: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
                _LayerNode { _ in }.id("b")
            }
        }
        struct Reordered: View {
            var body: some View {
                _LayerNode { _ in }.id("b")
                _LayerNode { _ in }.id("a")
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Initial())
        let anchor = tree.root!.children.first!
        let originalA = anchor.children[0]
        let originalB = anchor.children[1]
        let renderA = graph.renderTree.renderObject(for: originalA)!
        let renderB = graph.renderTree.renderObject(for: originalB)!

        graph.reconcileChildren(parent: anchor,
                                layoutParent: graph.layoutNode(for: anchor),
                                newViews: [Reordered().body])

        // Same RenderObjects, in new order.
        #expect(graph.renderTree.renderObject(for: originalA) === renderA)
        #expect(graph.renderTree.renderObject(for: originalB) === renderB)
        let mirror = graph.renderTree.renderObject(for: anchor)!
        #expect(mirror.children.count == 2)
        #expect(mirror.children[0] === renderB)
        #expect(mirror.children[1] === renderA)
    } }

    @Test("Tearing down a node drops its mirror")
    func tearDownClearsMirror() { GlobalTestLock.locked {
        struct Initial: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
                _LayerNode { _ in }.id("b")
            }
        }
        struct Trimmed: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Initial())
        let anchor = tree.root!.children.first!
        let bNode = anchor.children[1]
        #expect(graph.renderTree.renderObject(for: bNode) != nil)

        graph.reconcileChildren(parent: anchor,
                                layoutParent: graph.layoutNode(for: anchor),
                                newViews: [Trimmed().body])

        #expect(graph.renderTree.renderObject(for: bNode) == nil)
        let mirror = graph.renderTree.renderObject(for: anchor)!
        #expect(mirror.children.count == 1)
    } }

    @Test("LayerInventory enumerates root + clipped descendants")
    func layerInventoryEnumeratesLayerRoots() { GlobalTestLock.locked {
        struct Wrapper: View {
            var body: some View {
                _LayerNode { _ in }                                           // not layer
                _LayerNode { node in node.clipsToBounds = true }              // layer
                _LayerNode { node in node.opacity = 0.5 }                     // layer
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Wrapper())

        let layers = graph.renderTree.layerRoots()
        // Root is always one layer; plus the two promoted children.
        #expect(layers.count == 3)
        #expect(layers.first === graph.renderTree.root)
    } }

    // MARK: - Phase 4c: flat draw-order table

    /// The flat table is the precomputed "draw order" the renderer will consume
    /// to turn its per-frame recursive DFS into a single linear scan. These
    /// tests pin the two properties that make a linear pass correct: every
    /// parent is visited before its children, and siblings are already in
    /// z-order so the pass never needs to sort.
    @Test("Flat draw order is complete and visits every parent before its children")
    func flatDrawOrderIsPreorderAndComplete() { GlobalTestLock.locked {
        struct Nested: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
                _LayerNode { node in node.clipsToBounds = true }.id("b")
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Nested())

        let flat = graph.renderTree.flatDrawOrder()
        // One entry per mirrored object, no duplicates dropped or repeated.
        #expect(flat.count == graph.renderTree.objectCount)
        var indexOf: [ObjectIdentifier: Int] = [:]
        for (index, obj) in flat.enumerated() { indexOf[ObjectIdentifier(obj)] = index }
        for (index, obj) in flat.enumerated() where index > 0 {
            guard let parent = obj.parent,
                  let parentIndex = indexOf[ObjectIdentifier(parent)] else { continue }
            #expect(parentIndex < index)
        }
    } }

    @Test("Flat draw order emits siblings in zIndex order, so the pass needs no sort")
    func flatDrawOrderRespectsZIndex() { GlobalTestLock.locked {
        struct ZStacked: View {
            var body: some View {
                _LayerNode { node in node.zIndex = 5 }.id("high")
                _LayerNode { node in node.zIndex = -1 }.id("low")
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: ZStacked())

        let container = tree.root!.children.first!
        let highNode = container.children[0]   // zIndex 5
        let lowNode = container.children[1]    // zIndex -1
        let flat = graph.renderTree.flatDrawOrder()
        let highIndex = flat.firstIndex { $0.node === highNode }
        let lowIndex = flat.firstIndex { $0.node === lowNode }
        #expect(highIndex != nil)
        #expect(lowIndex != nil)
        // The lower zIndex paints first even though it was declared second.
        #expect(lowIndex! < highIndex!)
    } }

    @Test("Flat draw order rebuilds after a structural reconcile")
    func flatDrawOrderRebuildsAfterReconcile() { GlobalTestLock.locked {
        struct Initial: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
                _LayerNode { _ in }.id("b")
            }
        }
        struct Trimmed: View {
            var body: some View {
                _LayerNode { _ in }.id("a")
            }
        }
        let tree = NodeTree()
        let graph = ViewGraph(tree: tree, recomposer: Recomposer())
        graph.install(root: Initial())
        let before = graph.renderTree.flatDrawOrder().count

        let anchor = tree.root!.children.first!
        graph.reconcileChildren(parent: anchor,
                                layoutParent: graph.layoutNode(for: anchor),
                                newViews: [Trimmed().body])

        let after = graph.renderTree.flatDrawOrder().count
        // A torn-down subtree must leave the memoised table.
        #expect(after < before)
    } }
}
