private import Foundation
private import CoveUIScene
import CoveUIText
import CoveUIImaging
@_spi(CustomLayout) public import CoveUIScene

/// Inset grouped rows: an optional caption, then the rows on one rounded
/// surface, separated by hairlines that start at the row content. The shape
/// phones use for settings and detail screens; touch surfaces get taller rows
/// and rounder corners.
public struct ListGroup<Content: View>: View {
    private let title: String?
    private let footer: String?
    private let content: Content

    public init(_ title: String? = nil, footer: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.footer = footer
        self.content = content()
    }

    public var body: some View {
        SurfaceReader { surface in
            ThemeReader { theme in
                let touch = surface.input.contains(.touch)
                Box(direction: .column, alignItems: .stretch, spacing: 6) {
                    if let title {
                        Text(title).font(.caption).foregroundColor(.onSurfaceMuted)
                            .padding(EdgeInsets(leading: 16, trailing: 16))
                    }
                    // The surface sits on its own node: the rows node paints the separators.
                    Box(direction: .column, alignItems: .stretch, spacing: 0) {
                        ListGroupRows(inset: 16) { content }.debugName("list-group-rows")
                    }.frame(width: .percent(100), minWidth: 0)
                        .background(.surfaceRaised).cornerRadius(touch ? theme.radius.lg : theme.radius.md)
                    if let footer {
                        Text(footer).font(.caption).foregroundColor(.onSurfaceMuted)
                            .padding(EdgeInsets(leading: 16, trailing: 16))
                    }
                }.frame(width: .percent(100), minWidth: 0).debugName("list-group")
            }
        }
    }
}

/// Stacks rows and paints the separators between them.
private struct ListGroupRows<Content: View>: _PrimitiveView {
    let inset: Float
    @ViewBuilder let content: Content

    func _makeNode() -> Node { Node() }
    func _makeLayoutNode() -> LayoutNode? { LayoutNode() }
    func _updateLayout(_ layout: LayoutNode) {
        layout.flexDirection = .column
        layout.alignItems = .stretch
        layout.setWidthPercent(100)
        layout.setPadding(inset, edge: .left)
        layout.setPadding(inset, edge: .right)
        layout.minWidth = 0
    }
    func _updateNode(_ node: Node) {
        let inset = inset
        // Separator positions come from the finished layout; keying the paint on
        // them repaints when rows move, appear or change height.
        node.layoutDidUpdate = { node in
            let bottoms = node.children.filter(\.isLayoutVisible).dropLast()
                .map { Float($0.absoluteFrame.maxY - node.absoluteFrame.minY) }
            node.updateOverlayDraw(identity: bottoms) { [weak node] list, origin in
                guard let node else { return }
                let color = node.theme.colors.divider.multipliedAlpha(node.opacity)
                for bottom in bottoms {
                    list.addRect(.init(x: Float(origin.x) + inset, y: Float(origin.y) + bottom - 0.5,
                                       width: max(0, Float(node.frame.width) - inset), height: 0.5), color: color)
                }
            }
        }
    }
    var _children: [any View] { [content] }
}
