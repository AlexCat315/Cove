import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Virtualizes explicitly sized, variable-height rows. The height closure
/// receives the element, available content width in logical points, and the
/// inherited text scaler. It must return the complete row height; this view
/// does not estimate or automatically measure offscreen content.
///
/// Heights are cached until data, width, spacing, or text scaling changes.
/// Scrolling locates rows in O(log n) and constructs only the viewport plus
/// overscan. IDs must be unique and stable. Keep persistent row data in the
/// model, since fully offscreen views are unmounted.
struct VariableVirtualStack<Data: RandomAccessCollection, ID: Hashable, Content: View>: View {
    private let data: Data
    private let id: KeyPath<Data.Element, ID>
    private let rowHeight: (Data.Element, Float, TextScaler) -> Float
    private let spacing: Float
    private let overscan: Int
    private let preload: Float
    private let scrollToIndex: Int?
    private let content: (Data.Element) -> Content
    private let generation = UUID()
    @State private var metrics = VariableRowMetrics<ID>()
    @State private var viewport = ScrollViewport(offset: .zero, size: .zero)
    @State private var window = VirtualWindow()
    @State private var previousTarget: Int?

    init(_ data: Data, id: KeyPath<Data.Element, ID>,
                rowHeight: @escaping (Data.Element, Float, TextScaler) -> Float,
                spacing: Float = 0, overscan: Int = 3, preload: Float = 0.3, scrollToIndex: Int? = nil,
                @ViewBuilder content: @escaping (Data.Element) -> Content) {
        self.data = data
        self.id = id
        self.rowHeight = rowHeight
        self.spacing = spacing.isFinite ? max(0, spacing) : 0
        self.overscan = max(0, overscan)
        self.preload = preload
        self.scrollToIndex = scrollToIndex
        self.content = content
    }

    var body: some View {
        SurfaceReader { surface in
            TextScalerReader { scaler in scrollView(scaler: scaler, gutter: ScrollView<EmptyView>.stableGutterWidth(on: surface)) }
        }
    }

    private func scrollView(scaler: TextScaler, gutter: Float) -> some View {
        let height = Double(viewport.size.height)
        let width = max(0, Float(viewport.size.width) - gutter)
        var nextOffset = Double(window.offset.y)
        // Wait for a real width rather than measuring the entire collection
        // twice during initial mount (first at zero, then at the viewport).
        if width > 0 {
            nextOffset = metrics.synchronize(
                data, id: id, generation: generation, width: width,
                scaler: scaler, spacing: spacing, offset: nextOffset, viewport: height,
                rowHeight: rowHeight)
        }
        let rows = metrics.rows
        if scrollToIndex == nil, previousTarget != nil { previousTarget = nil }
        if let target = scrollToIndex, target != previousTarget, height > 0, rows.count > 0 {
            nextOffset = rows.reveal(target, offset: nextOffset, viewport: height)
            previousTarget = target
        }
        if Double(window.offset.y) != nextOffset { window.offset.y = CGFloat(nextOffset) }
        let visible = rows.visibleRange(offset: rows.clamp(nextOffset, viewport: height),
                                        viewport: height, overscan: 0)
        let prepared = VirtualWindow.preparation(offset: rows.clamp(nextOffset, viewport: height), viewport: height, preload: preload)
        let range = rows.visibleRange(offset: prepared.offset, viewport: prepared.height, overscan: overscan)
        window.record(range: range, visible: visible, count: rows.count, size: viewport.size)
        let before = rows.top(of: range.lowerBound)
        let after = max(0, rows.total - rows.top(of: range.upperBound))
        return ScrollView(.vertical, scrollbarGutter: .stable,
                          scrollOffset: Binding(get: { window.offset }, set: { if window.offset != $0 { window.offset = $0 } }),
                          onViewportChange: { next in
                              window.offset = next.offset
                              let visible = rows.visibleRange(offset: rows.clamp(Double(next.offset.y), viewport: Double(next.size.height)),
                                                              viewport: Double(next.size.height), overscan: 0)
                              if window.needsRefill(visible: visible, size: next.size) { viewport = next }
                          }) {
            Box(direction: .column, alignItems: .stretch, spacing: 0) {
                Box {}.frame(height: Float(before)).flex(0, shrink: 0)
                range.map { row(at: $0, rows: rows, width: width) }
                Box {}.frame(height: Float(after)).flex(0, shrink: 0)
            }
            .frame(height: Float(rows.total), minWidth: 0)
            .flex(0, shrink: 0)
        }
    }

    private func row(at index: Int, rows: VariableRowIndex<ID>, width: Float) -> AnyView {
        let element = data[data.index(data.startIndex, offsetBy: index)]
        let gap = index + 1 < rows.count ? Float(rows.spacing) : 0
        return AnyView(MemoizedView(key: VirtualRowKey(generation: generation, index: index,
                                                         extent: Float(rows.extent(at: index)), width: width)) {
            Box(direction: .column, alignItems: .stretch) {
            content(element)
        }
        .frame(height: Float(rows.extent(at: index)), minWidth: 0)
        .padding(EdgeInsets(bottom: gap))
        .flex(0, shrink: 0)
        .clipped()
        }.id(element[keyPath: id]))
    }
}
