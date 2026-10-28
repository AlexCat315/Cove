import CoveUIScene
import CoveUIText
import CoveUIImaging

struct ResolvedLayoutDirection<Content: View>: View {
    let content: (Direction) -> Content
    @State private var direction = Direction.ltr
    init(@ViewBuilder content: @escaping (Direction) -> Content) { self.content = content }
    var body: some View {
        LayoutDirectionObserver(content: content(direction), onChange: { value in
            if direction != value { direction = value }
        })
    }
}

private struct LayoutDirectionObserver<Content: View>: _PrimitiveView {
    let content: Content
    let onChange: (Direction) -> Void
    func _makeNode() -> Node { let node = Node(); node.isHitTestable = false; return node }
    func _makeLayoutNode() -> LayoutNode? { let layout = LayoutNode(); layout.alignItems = .stretch; return layout }
    func _updateNode(_ node: Node) {
        node.layoutDidUpdate = { node in
            // Direction modifiers have committed by layout write-back. Reading
            // during composition would capture the previous frame's direction.
            var parent = node.parent
            var direction = Direction.ltr
            while let current = parent {
                if let value = current.layoutNode?.direction, value != .inherit { direction = value; break }
                parent = current.parent
            }
            onChange(direction)
        }
    }
    var _children: [any View] { [content] }
}
