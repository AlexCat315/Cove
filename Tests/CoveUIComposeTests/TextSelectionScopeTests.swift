import Foundation
import Testing
import CoveUIScene
@testable import CoveUICompose

@Suite("Text selection scope", .serialized)
struct TextSelectionScopeTests: CoveUIComposeSerializedSuite {
    private func position(_ node: Node, _ offset: Int) -> TextSelectionScope.Position {
        .init(member: ObjectIdentifier(node), offset: offset)
    }

    private func paragraph(_ text: String, at frame: CGRect, in parent: Node,
                           scope: TextSelectionScope) -> Node {
        let node = Node()
        node.frame = frame
        parent.addChild(node)
        scope.register(node, text: { text }, offsetAt: { _ in min(2, text.utf8.count) })
        return node
    }

    @Test("Dragging repaints only changed paragraphs and ignores repeated endpoints")
    func selectiveInvalidation() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), tree = NodeTree(), root = Node()
            tree.root = root
            let nodes = (0..<8).map {
                paragraph("abcdef", at: CGRect(x: 0, y: $0 * 24, width: 100, height: 20), in: root, scope: scope)
            }
            tree.flush()
            scope.select(from: position(nodes[1], 1), to: position(nodes[1], 1))
            #expect(!tree.hasRenderUpdates)
            scope.extend(to: position(nodes[3], 2))
            #expect(nodes.indices.filter { nodes[$0].renderDirty } == [1, 2, 3])
            tree.flush()

            scope.extend(to: position(nodes[3], 4))
            #expect(nodes.indices.filter { nodes[$0].renderDirty } == [3])
            tree.flush()
            scope.extend(to: position(nodes[3], 4))
            #expect(!tree.hasRenderUpdates)

            scope.extend(to: position(nodes[5], 2))
            #expect(nodes.indices.filter { nodes[$0].renderDirty } == [3, 4, 5])
            tree.flush()
            scope.extend(to: position(nodes[0], 3))
            #expect(nodes.indices.filter { nodes[$0].renderDirty } == [0, 1, 2, 3, 4, 5])
            #expect(scope.range(for: nodes[0]) == 3..<6)
            #expect(scope.range(for: nodes[1]) == 0..<1)
            #expect(scope.range(for: nodes[2]) == nil)
            tree.flush()

            scope.clear()
            #expect(nodes.indices.filter { nodes[$0].renderDirty } == [0, 1])
            #expect(scope.selectedText.isEmpty)
        }
    }

    @Test("Cached hit frames follow ancestor scrolling, movement and removal")
    func geometryInvalidation() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), root = Node(), container = Node()
            root.addChild(container)
            let first = paragraph("first", at: CGRect(x: 10, y: 100, width: 80, height: 20), in: container, scope: scope)
            let second = paragraph("second", at: CGRect(x: 10, y: 140, width: 80, height: 20), in: container, scope: scope)
            #expect(scope.position(at: CGPoint(x: 20, y: 110)) == position(first, 2))
            #expect(scope.textUnder(CGPoint(x: 20, y: 110)))

            container.contentOffset = CGPoint(x: 0, y: 100)
            #expect(!scope.textUnder(CGPoint(x: 20, y: 110)))
            #expect(scope.position(at: CGPoint(x: 20, y: 10)) == position(first, 2))
            #expect(scope.position(at: CGPoint(x: 200, y: 50)) == position(second, 2))
            #expect(scope.position(at: CGPoint(x: 20, y: 30)) == position(second, 0))
            #expect(scope.position(at: CGPoint(x: 20, y: 90)) == position(second, 6))

            container.frame.origin.x = 100
            #expect(!scope.textUnder(CGPoint(x: 20, y: 10)))
            #expect(scope.textUnder(CGPoint(x: 120, y: 10)))
            container.removeChild(first)
            #expect(!scope.textUnder(CGPoint(x: 120, y: 10)))
            #expect(scope.position(at: CGPoint(x: 120, y: 10)) == position(second, 0))
        }
    }

    @Test("Detached subtrees and reused nodes leave their previous selection scope")
    func registrationLifetime() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), next = TextSelectionScope(), root = Node(), container = Node()
            root.addChild(container)
            let node = paragraph("hello", at: CGRect(x: 0, y: 0, width: 80, height: 20), in: container, scope: scope)
            #expect(scope.textUnder(CGPoint(x: 10, y: 10)))
            root.removeChild(container)
            // The paragraph still has its immediate parent while its entire
            // subtree is detached. Registration must follow the mount lifetime.
            #expect(node.parent === container)
            #expect(!scope.textUnder(CGPoint(x: 10, y: 10)))
            root.addChild(container)
            scope.register(node, text: { "hello" }, offsetAt: { _ in 2 })
            #expect(scope.textUnder(CGPoint(x: 10, y: 10)))
            next.register(node, text: { "hello" }, offsetAt: { _ in 2 })
            #expect(!scope.textUnder(CGPoint(x: 10, y: 10)))
            #expect(next.textUnder(CGPoint(x: 10, y: 10)))
            TextSelectionScope.unregister(node)
            #expect(!next.textUnder(CGPoint(x: 10, y: 10)))
        }
    }

    @Test("Reflow and insertion update cached ranges and retained highlights")
    func readingOrderChanges() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), tree = NodeTree(), root = Node()
            tree.root = root
            let a = paragraph("alpha", at: CGRect(x: 0, y: 0, width: 80, height: 20), in: root, scope: scope)
            let b = paragraph("beta", at: CGRect(x: 0, y: 30, width: 80, height: 20), in: root, scope: scope)
            let c = paragraph("gamma", at: CGRect(x: 0, y: 60, width: 80, height: 20), in: root, scope: scope)
            scope.select(from: position(a, 0), to: position(c, 5))
            #expect(scope.selectedText == "alpha\nbeta\ngamma")
            tree.flush()

            b.frame.origin.y = 90
            tree.flush()
            #expect(scope.range(for: b) == nil)
            #expect(b.renderDirty)
            #expect(scope.selectedText == "alpha\ngamma")
            let d = paragraph("delta", at: CGRect(x: 0, y: 30, width: 80, height: 20), in: root, scope: scope)
            #expect(scope.range(for: d) == 0..<5)
            #expect(scope.selectedText == "alpha\ndelta\ngamma")
            root.removeChild(c)
            #expect(scope.selectedText.isEmpty)
            #expect(scope.range(for: a) == nil)
        }
    }

    @Test("Reverse selections preserve UTF-8 and table reading order")
    func unicodeTableCopy() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), root = Node()
            let right = paragraph("🙂右", at: CGRect(x: 100, y: 0, width: 80, height: 20), in: root, scope: scope)
            let left = paragraph("中文", at: CGRect(x: 0, y: 0.4, width: 80, height: 20), in: root, scope: scope)
            let bottom = paragraph("末尾", at: CGRect(x: 0, y: 30, width: 80, height: 20), in: root, scope: scope)
            scope.select(from: position(bottom, 3), to: position(left, 3))
            #expect(scope.selectedText == "文\t🙂右\n末")
            #expect(scope.range(for: right) == 0..<7)
            scope.selectAll()
            #expect(scope.selectedText == "中文\t🙂右\n末尾")
            scope.clear()
            #expect(scope.position(at: CGPoint(x: -100, y: -100)) == position(left, 0))
        }
    }

    @Test("Large pages keep selection lookup and endpoint updates responsive")
    func largePageSelection() {
        GlobalTestLock.locked {
            let scope = TextSelectionScope(), tree = NodeTree(), root = Node()
            tree.root = root
            var container = root
            for _ in 0..<12 {
                let child = Node()
                container.addChild(child)
                container = child
            }
            let nodes = (0..<1_000).map {
                paragraph("paragraph", at: CGRect(x: 0, y: $0 * 24, width: 100, height: 20), in: container, scope: scope)
            }
            tree.flush()
            let clock = ContinuousClock(), start = clock.now
            scope.selectAll()
            var selectedBytes = 0
            for _ in 0..<3 {
                for node in nodes { selectedBytes += scope.range(for: node)?.count ?? 0 }
            }
            #expect(selectedBytes == 27_000)
            for offset in 1...200 {
                tree.flush()
                scope.extend(to: position(nodes[999], 1 + offset % 8))
                #expect(!nodes[0].renderDirty)
                #expect(!nodes[500].renderDirty)
            }
            #expect(scope.selectedText.split(separator: "\n").count == 1_000)
            let elapsed = start.duration(to: clock.now)
            print("Text selection: 1,000 paragraphs, 3,000 lookups and 200 endpoint updates: \(elapsed)")
            #expect(elapsed < .seconds(3))
        }
    }
}
