import Foundation
import CoveUIRuntime

public struct DataTableNavigation<ID: Hashable & Sendable> {
    public var requestID: AnyHashable?
    public var rowID: ID?
    public var columnID: String?
    public init() {}
}
public struct DataTableOptions<ID: Hashable & Sendable> {
    public var selection: Binding<DataTableSelection<ID>>?
    public var selectionMode: DataTableSelectionMode = .multiple
    public var columnWidths: Binding<[String: Float]>?
    public var columnVisibility: Binding<[String: Bool]>?
    /// Display order by column ID; header drags and Option/Alt+Left/Right write it.
    public var columnOrder: Binding<[String]>?
    public var layout = TableLayout()
    public var frozenColumnCount = 1
    public var navigation = DataTableNavigation<ID>()
    public var isEnabled = true
    public init() {}
}
struct DataTableCellDraft<ID: Hashable & Sendable> {
    let rowID: ID
    let columnID: String
    let identity = UUID()
    let original: TextBuffer
    var buffer: TextBuffer
    var error: String?
    init(rowID: ID, columnID: String, buffer: TextBuffer) {
        self.rowID = rowID; self.columnID = columnID; original = buffer; self.buffer = buffer
    }
}
struct DataTableSession<ID: Hashable & Sendable> {
    var selection = DataTableSelection<ID>()
    var widths: [String: Float] = [:]
    var visibility: [String: Bool] = [:]
    /// Shared with the scrolling body so tests and external controls can read
    /// the live offset without making the outer table scope re-render on every
    /// scroll packet. Writing through the reference does not dirty `@State`.
    let scroll = DataTableScrollMetrics()
    var viewport: ScrollViewport {
        get { scroll.viewport }
        set { scroll.viewport = newValue }
    }
    var offset: CGPoint {
        get { scroll.offset }
        set { scroll.setOffset(newValue) }
    }
    var editing: DataTableCellDraft<ID>?
    var lastNavigation: AnyHashable?
    var order: [String] = []
    var columnDrag: DataTableColumnDrag?
    let focus = DataTableFocusTarget()
    let rowHeights = DataTableRowHeightCache()
}

final class DataTableScrollMetrics: _ObservableObject {
    private let publisher = _ObservablePublisher<DataTableScrollMetrics>()
    private(set) var offset = CGPoint.zero
    var viewport = ScrollViewport(offset: .zero, size: .zero)
    /// Window anchors and counts are the only values that rebuild row/header
    /// views. Raw offsets are applied to the pane nodes directly, so a scroll
    /// packet cannot cause a full table reconciliation.
    private(set) var rowWindowStart = 0
    private(set) var rowWindowCount = 0
    private(set) var columnWindowStart = 0
    private(set) var columnWindowCount = 0
    weak var horizontalPane: Node?
    weak var verticalPane: Node?

    func _registerObserver(_ handler: @escaping () -> Void) -> AnyHashable {
        publisher.register(on: self, handler: handler)
    }
    func _unregisterObserver(_ token: AnyHashable) {
        publisher.unregister(token)
    }

    func setOffset(_ next: CGPoint) {
        guard offset != next else { return }
        offset = next
        horizontalPane?.contentOffset.x = next.x
        verticalPane?.contentOffset.y = next.y
    }

    func updateWindows(row: Range<Int>, column: Range<Int>) {
        var changed = false
        let rowStart = row.lowerBound
        let rowCount = row.count
        if rowWindowStart != rowStart || rowWindowCount != rowCount {
            rowWindowStart = rowStart; rowWindowCount = rowCount; changed = true
        }
        let columnStart = column.lowerBound
        let columnCount = column.count
        if columnWindowStart != columnStart || columnWindowCount != columnCount {
            columnWindowStart = columnStart; columnWindowCount = columnCount; changed = true
        }
        if changed { publisher.send() }
    }
}
struct DataTableColumnDrag: Equatable {
    let columnID: String
    var offset: Float
}

/// A persistent row model, fixed header, frozen leading columns and two-axis
/// cell window. Only visible rows/columns plus overscan create view nodes.
/// Row selection and cell edit drafts are transient, independent of the model.
public struct DataTable<Record: Sendable, ID: Hashable & Sendable>: View {
    public let model: DataTableModel<Record, ID>
    public private(set) var columns: [TableColumn<Record>]
    public var options = DataTableOptions<ID>()
    public var onActivate: ((Record) -> Void)?
    /// Per-record logical height (scaled with text). `nil` keeps every row at
    /// `options.layout.rowHeight`, which also remains the minimum.
    public var rowHeight: ((Record) -> Float)?
    var textScaler = TextScaler.identity
    /// Every column ID in display order, including hidden columns.
    var displayOrder: [String] = []
    /// Hierarchy keys (TreeTable) get the first chance at row navigation.
    var rowKeyHandler: ((KeyEvent, ID) -> Bool)?
    private var observedRevision: Observed<DataTableModel<Record, ID>, UInt64>
    @State var session = DataTableSession<ID>()
    public init(_ model: DataTableModel<Record, ID>, columns: [TableColumn<Record>], configure: (inout Self) -> Void = { _ in }) {
        self.model = model; self.columns = columns
        observedRevision = Observed(\.revision, on: model)
        configure(&self); options.layout.validate()
        precondition(Set(columns.map(\.id)).count == columns.count, "DataTable column IDs must be unique")
    }
    var geometry: DataTableColumnGeometry {
        let widths = options.columnWidths?.wrappedValue ?? session.widths
        return DataTableColumnGeometry(widths: columns.map { $0.layout.constrained(widths[$0.id] ?? $0.layout.width) }, frozenCount: options.frozenColumnCount)
    }
    var selectionBinding: Binding<DataTableSelection<ID>> {
        options.selection ?? Binding(get: { session.selection }, set: { session.selection = $0 })
    }
    var orderBinding: Binding<[String]> {
        options.columnOrder ?? Binding(get: { session.order }, set: { session.order = $0 })
    }
    private var visibilityBinding: Binding<[String: Bool]> {
        options.columnVisibility ?? Binding(get: { session.visibility }, set: { session.visibility = $0 })
    }
    public var body: some View {
        SurfaceReader { surface in
            TextScalerReader { scaler in
                var table = self
                table.options.layout = options.layout.resolved(in: surface, textScaler: scaler)
                table.textScaler = scaler
                let ordered = DataTableColumnOrder.apply(orderBinding.wrappedValue, to: columns)
                table.displayOrder = ordered.map(\.id)
                if let request = options.navigation.requestID, request != session.lastNavigation,
                   let rowID = options.navigation.rowID, model.index(for: rowID) != nil,
                   let id = options.navigation.columnID, let column = columns.first(where: { $0.id == id }),
                   column.visibility != .required, visibilityBinding.wrappedValue[id] != true {
                    visibilityBinding.wrappedValue[id] = true
                }
                return TableColumns(columns: ordered, widths: options.columnWidths?.wrappedValue ?? session.widths,
                    overrides: visibilityBinding, frozenCount: options.frozenColumnCount,
                    pinnedIDs: Set(session.editing.map { [$0.columnID] } ?? []), isEnabled: options.isEnabled) { plan in
                    var presented = table; presented.columns = plan.indices.map { ordered[$0] }
                    presented.options.frozenColumnCount = plan.frozenCount
                    return presented.content(availableWidth: plan.availableWidth)
                }
            }
        }
    }
    private func content(availableWidth: Float) -> some View {
        let _ = observedRevision.wrappedValue
        if let focused = selectionBinding.wrappedValue.focusedColumnID, !columns.contains(where: { $0.id == focused }) {
            var selection = selectionBinding.wrappedValue
            selection.focusedColumnID = columns.first?.id; selectionBinding.wrappedValue = selection
        }
        if options.navigation.requestID != nil, options.navigation.requestID != session.lastNavigation,
           let id = options.navigation.rowID, let row = model.index(for: id) {
            session.lastNavigation = options.navigation.requestID
            reveal(row: row, column: columns.firstIndex { $0.id == options.navigation.columnID })
        }
        return DataTableScrollingBody(table: self, availableWidth: availableWidth)
            .border(.border, width: 1).cornerRadius(.groupRadius).opacity(options.isEnabled ? 1 : 0.55)
            .accessibility { $0.role = .table; $0.label = "Data table"; $0.value = "\(model.count) rows, \(columns.count) columns"; $0.state.isEnabled = options.isEnabled }
    }
    fileprivate func header(geometry: DataTableColumnGeometry, moving: Range<Int>,
                            viewport: ScrollViewport, model: DataTableScrollMetrics,
                            onWheel: @escaping (MouseWheelEvent) -> Bool) -> AnyView {
        AnyView(Row(alignment: .top, spacing: 0) {
            if geometry.frozenCount > 0 {
                headerCells(in: 0..<geometry.frozenCount, geometry: geometry).frame(width: geometry.frozenWidth).flex(0, shrink: 0)
            }
            DataTableClippedPane(onWheel: onWheel,
                content: DataTableOffsetPane(model: model, axis: .horizontal,
                    content: headerCells(in: moving, geometry: geometry)
                        .frame(width: max(geometry.movingWidth, Float(viewport.size.width)), height: options.layout.headerHeight)))
                .flex(1, shrink: 1).frame(height: options.layout.headerHeight, minWidth: 0)
            Box {}.frame(width: 12).flex(0, shrink: 0)
            // Absolute, so the insertion line never takes part in the header flow.
            if let indicator = columnDropIndicator(geometry: geometry) { indicator }
        }.frame(height: options.layout.headerHeight).background(.surfaceVariant))
    }
    private func headerCells(in range: Range<Int>, geometry: DataTableColumnGeometry) -> AnyView {
        AnyView(Row(alignment: .center, spacing: 0) {
            if range.lowerBound >= geometry.frozenCount, range.lowerBound < geometry.starts.count {
                Box {}.frame(width: geometry.starts[range.lowerBound] - geometry.frozenWidth).flex(0, shrink: 0)
            }
            for index in range {
                let columnID = columns[index].id
                AnyView(DataTableHeaderDragHost(isEnabled: options.isEnabled, onChange: { offset in
                    let next = offset.map { DataTableColumnDrag(columnID: columnID, offset: $0) }
                    if session.columnDrag != next { session.columnDrag = next }
                }, onDrop: { dropColumn(at: index, offset: $0, geometry: geometry) },
                    content: headerCell(columns[index], width: geometry.widths[index])).id(columnID))
            }
            Spacer()
        })
    }
    private func headerCell(_ column: TableColumn<Record>, width: Float) -> some View {
        Row(alignment: .center, spacing: 0) {
            if column.compare == nil {
                Text(column.title, lineLimit: 1).font(.label).padding(horizontal: 10).flex(1, shrink: 1)
            } else {
                Button(isEnabled: options.isEnabled && column.compare != nil, action: { cycleSort(column) }) {
                    Row(alignment: .center, spacing: 5) {
                        Text(column.title, lineLimit: 1).font(.label)
                        if model.sort?.columnID == column.id {
                            Text(model.sort?.direction == .ascending ? "↑" : "↓").font(.caption)
                        }
                    }.padding(horizontal: 10)
                }.buttonStyle(TableHeaderStyle()).flex(1, shrink: 1)
                    .accessibility {
                        $0.label = "Sort by \(column.title)"
                        $0.value = model.sort?.columnID == column.id ? model.sort?.direction.rawValue ?? "" : "Source order"
                        $0.help = model.sorting.isPending ? "Sorting in background" : "Activate to cycle ascending, descending and source order"
                    }
            }
            if column.layout.isResizable {
                TableResizeGrip(width: widthBinding(column), layout: column.layout, isEnabled: options.isEnabled)
                    .accessibilityLabel("Resize \(column.title) column")
            }
        }.frame(width: width, height: options.layout.headerHeight).flex(0, shrink: 0).clipped()
            .opacity(session.columnDrag?.columnID == column.id ? 0.5 : 1)
            .modifier(TableColumnIdentity(id: column.id))
    }
    fileprivate func bodyViewport(geometry: DataTableColumnGeometry,
                                  metrics: DataTableRowMetrics, viewport: ScrollViewport,
                                  model: DataTableScrollMetrics,
                                  onScroll: @escaping (CGPoint) -> Void,
                                  onViewportChange: @escaping (ScrollViewport) -> Void,
                                  onWheel: @escaping (MouseWheelEvent) -> Bool) -> AnyView {
        let height = metrics.total
        return AnyView(Box(direction: .row, alignItems: .stretch, spacing: 0) {
            if geometry.frozenCount > 0 {
                DataTableClippedPane(onWheel: onWheel,
                    content: DataTableOffsetPane(model: model, axis: .vertical,
                        content: DataTableFrozenRows(table: self, metrics: metrics, geometry: geometry,
                                                     frozenCount: geometry.frozenCount, model: model,
                                                     width: geometry.frozenWidth, height: height)))
                    .frame(width: geometry.frozenWidth, minHeight: 0).flex(0, shrink: 0).background(.surface)
            }
            ScrollView(.both, scrollbarGutter: .stable,
                scrollOffset: Binding(get: { model.offset }, set: {
                    model.setOffset($0); onScroll($0)
                }),
                onViewportChange: onViewportChange) {
                DataTableMovingRows(table: self, metrics: metrics, geometry: geometry, model: model)
                    .frame(width: max(geometry.movingWidth, Float(viewport.size.width)), height: height)
            }.flex(1, shrink: 1).frame(minWidth: 0, minHeight: 0)
        }.frame(minHeight: 0))
    }
    fileprivate func rowWindow(_ rows: Range<Int>, columns range: Range<Int>, geometry: DataTableColumnGeometry,
                           metrics: DataTableRowMetrics) -> AnyView {
        if range.isEmpty { return AnyView(Box {}.frame(height: metrics.total)) }
        return AnyView(Box(direction: .column, alignItems: .stretch, spacing: 0) {
            Box {}.frame(height: metrics.top(rows.lowerBound)).flex(0, shrink: 0)
            for index in rows {
                AnyView(row(index, columns: range, geometry: geometry, height: metrics.height(index)).id(model.rowID(at: index)))
            }
            Box {}.frame(height: max(0, metrics.total - metrics.top(rows.upperBound))).flex(0, shrink: 0)
        })
    }
    private func row(_ index: Int, columns range: Range<Int>, geometry: DataTableColumnGeometry, height: Float) -> some View {
        let id = model.rowID(at: index), record = model.record(at: index), selection = selectionBinding.wrappedValue
        return DataTableRowHost(index: index, isSelected: selection.selectedIDs.contains(id), isStriped: options.layout.isStriped,
            isEnabled: options.isEnabled, isFrozenLane: range.upperBound <= geometry.frozenCount && geometry.frozenCount < geometry.widths.count,
            content: Row(alignment: .center, spacing: 0) {
                if range.lowerBound >= geometry.frozenCount, range.lowerBound < geometry.starts.count {
                    Box {}.frame(width: geometry.starts[range.lowerBound] - geometry.frozenWidth).flex(0, shrink: 0)
                }
                for columnIndex in range {
                    let column = columns[columnIndex]
                    AnyView(DataTableCellHost(title: column.title,
                        isActive: selection.focusedID == id && selection.focusedColumnID == column.id,
                        isEnabled: options.isEnabled, onSelect: { select(row: index, column: columnIndex, modifiers: $0) },
                        onEdit: {
                            if column.textEditing != nil { beginEditing(row: index, column: columnIndex) }
                            else { onActivate?(record) }
                        },
                        content: cell(record: record, rowID: id, column: column))
                        .frame(width: geometry.widths[columnIndex], height: height).flex(0, shrink: 0).clipped()
                        .modifier(TableColumnIdentity(id: column.id)).id(column.id))
                }
                Spacer()
            }).frame(height: height).flex(0, shrink: 0)
    }
    var rowMetrics: DataTableRowMetrics {
        session.rowHeights.metrics(model: model, minimum: options.layout.rowHeight, scaler: textScaler, rowHeight: rowHeight)
    }

    private func widthBinding(_ column: TableColumn<Record>) -> Binding<Float> {
        Binding(get: { column.layout.constrained((options.columnWidths?.wrappedValue ?? session.widths)[column.id] ?? column.layout.width) }, set: { width in
            if let binding = options.columnWidths { binding.wrappedValue[column.id] = column.layout.constrained(width) }
            else { session.widths[column.id] = column.layout.constrained(width) }
        })
    }
    private func cycleSort(_ column: TableColumn<Record>) {
        guard commitEditing() else { return }
        let next: TableSort?
        if model.sort?.columnID != column.id { next = TableSort(column.id) }
        else if model.sort?.direction == .ascending { next = TableSort(column.id, direction: .descending) }
        else { next = nil }
        model.setSort(next, compare: column.compare)
    }
    func select(row: Int, column: Int, modifiers: KeyModifiers) {
        var selection = selectionBinding.wrappedValue
        selection.select(index: row, in: model, mode: options.selectionMode, modifiers: modifiers)
        selection.focusedColumnID = columns[column].id; selectionBinding.wrappedValue = selection
        session.focus.focus()
    }
    func reveal(row: Int, column: Int?) {
        let metrics = rowMetrics
        let top = CGFloat(metrics.top(row)), bottom = top + CGFloat(metrics.height(row))
        let visibleHeight = max(CGFloat(metrics.height(row)), session.viewport.size.height)
        if top < session.offset.y { session.offset.y = top }
        else if bottom > session.offset.y + visibleHeight { session.offset.y = max(0, bottom - visibleHeight) }
        let geometry = geometry
        if let column, column >= geometry.frozenCount {
            let left = CGFloat(geometry.starts[column] - geometry.frozenWidth), right = left + CGFloat(geometry.widths[column])
            let visibleWidth = max(CGFloat(geometry.widths[column]), session.viewport.size.width)
            if left < session.offset.x { session.offset.x = left }
            else if right > session.offset.x + visibleWidth { session.offset.x = max(0, right - visibleWidth) }
        }
    }
    fileprivate func handleKey(_ event: KeyEvent, viewport: ScrollViewport,
                           reveal: (Int, Int?) -> Void) -> Bool {
        guard options.isEnabled, model.count > 0, !columns.isEmpty else { return false }
        var selection = selectionBinding.wrappedValue
        let current = selection.focusedID.flatMap(model.index(for:))
        let column = selection.focusedColumnID.flatMap { id in columns.firstIndex { $0.id == id } } ?? 0
        if column == 0, let current, let rowKeyHandler, rowKeyHandler(event, model.rowID(at: current)) { return true }
        if event.modifiers.contains(.alt), event.scancode == Scancode.arrowLeft || event.scancode == Scancode.arrowRight {
            return moveColumn(at: column, by: event.scancode == Scancode.arrowLeft ? -1 : 1)
        }
        if event.scancode == Scancode.a, !event.modifiers.isDisjoint(with: [.gui, .ctrl]), options.selectionMode == .multiple {
            selection.selectedIDs = Set((0..<model.count).map { model.rowID(at: $0) }); selectionBinding.wrappedValue = selection; return true
        }
        let row: Int, nextColumn: Int
        switch event.scancode {
        case Scancode.arrowDown: row = min(model.count - 1, (current ?? -1) + 1); nextColumn = column
        case Scancode.arrowUp: row = max(0, (current ?? model.count) - 1); nextColumn = column
        case Scancode.home: row = 0; nextColumn = column
        case Scancode.end: row = model.count - 1; nextColumn = column
        case Scancode.pageUp:
            let metrics = rowMetrics, from = current ?? 0
            row = min(max(0, from - 1), metrics.row(at: metrics.top(from) - Float(viewport.size.height))); nextColumn = column
        case Scancode.pageDown:
            let metrics = rowMetrics, from = current ?? 0
            row = max(min(model.count - 1, from + 1), min(model.count - 1, metrics.row(at: metrics.top(from) + Float(viewport.size.height)))); nextColumn = column
        case Scancode.arrowLeft: row = current ?? 0; nextColumn = max(0, column - 1)
        case Scancode.arrowRight: row = current ?? 0; nextColumn = min(columns.count - 1, column + 1)
        case Scancode.f2:
            guard let current else { return false }; beginEditing(row: current, column: column); return true
        case Scancode.return, Scancode.keypadEnter:
            guard let current else { return false }
            if columns[column].textEditing != nil { beginEditing(row: current, column: column) }
            else { onActivate?(model.record(at: current)) }; return true
        case Scancode.escape: selectionBinding.wrappedValue = DataTableSelection(); return true
        default: return false
        }
        selection.select(index: row, in: model, mode: options.selectionMode, modifiers: event.modifiers)
        selection.focusedColumnID = columns[nextColumn].id; selectionBinding.wrappedValue = selection
        reveal(row, nextColumn); return true
    }
}

/// Owns the live scroll offset for a DataTable. Raw offsets are applied to
/// pane nodes directly; only the virtualised row/column windows rebuild, so a
/// scroll packet cannot reconcile the whole table or the surrounding page.
private struct DataTableScrollingBody<Record: Sendable, ID: Hashable & Sendable>: View {
    let table: DataTable<Record, ID>
    let availableWidth: Float
    @State private var viewport = ScrollViewport(offset: .zero, size: .zero)
    /// Wheel input over the frozen lane/header cannot reach the moving
    /// ScrollView directly. Bumping this state re-evaluates the binding so the
    /// scroll view adopts the offset the pane just applied.
    @State private var externalWheelAdoption = 0

    private var model: DataTableScrollMetrics { table.session.scroll }

    var body: some View {
        model.viewport = viewport
        refreshWindows(offset: model.offset)
        return DataTableInputHost(focus: table.session.focus, isEnabled: table.options.isEnabled,
            onKey: { table.handleKey($0, viewport: viewport, reveal: reveal) },
            content: Box(direction: .column, alignItems: .stretch, spacing: 0) {
                DataTableHeaderScroller(table: table, geometry: table.geometry, viewport: viewport,
                                        model: model, onWheel: scrollWheel).flex(0, shrink: 0)
                Divider()
                if table.model.count == 0 {
                    EmptyState(table.options.layout.emptyTitle, message: "Change your filters or add a record.", action: { EmptyView() }).flex(1)
                } else {
                    table.bodyViewport(geometry: table.geometry, metrics: table.rowMetrics, viewport: viewport,
                                       model: model,
                                       onScroll: { next in
                                           refreshWindows(offset: next)
                                       },
                                       onViewportChange: { next in
                                           model.viewport = next
                                           if viewport.size != next.size {
                                               viewport = ScrollViewport(offset: .zero, size: next.size)
                                               refreshWindows(offset: model.offset)
                                           }
                                       },
                                       onWheel: scrollWheel)
                        .flex(1, shrink: 1, basis: 0).frame(minHeight: 0)
                }
                if let message = table.session.editing?.error {
                    Text(message).font(.caption).foregroundColor(.error).padding(horizontal: 10, vertical: 5)
                        .accessibility { $0.role = .status }
                }
            }.flex(1, shrink: 1, basis: 0).frame(minWidth: 0, minHeight: 0))
    }

    private var movingWidth: Float {
        table.model.count == 0
            ? availableWidth - table.geometry.frozenWidth
            : Float(viewport.size.width)
    }

    private func refreshWindows(offset: CGPoint) {
        let geometry = table.geometry
        let metrics = table.rowMetrics
        let visibleRows = metrics.visibleRange(offset: Float(offset.y), viewport: Float(viewport.size.height), overscan: 3)
        let visibleColumns = geometry.visibleColumns(offset: Float(offset.x), width: movingWidth)
        model.updateWindows(row: window(visibleRows, start: model.rowWindowStart,
                                        count: model.rowWindowCount, total: metrics.count, slack: 4),
                            column: window(visibleColumns, start: model.columnWindowStart,
                                           count: model.columnWindowCount, total: geometry.widths.count, slack: 0))
    }

    /// Keep the existing window while the visible rows/columns remain inside
    /// it. `slack` trades rebuild frequency for a slightly larger pane; four
    /// extra rows hide a row's worth of scrolling without materialising a
    /// second screen of cells.
    private func window(_ visible: Range<Int>, start: Int, count: Int, total: Int, slack: Int) -> Range<Int> {
        if count > 0, visible.lowerBound >= start, visible.upperBound <= start + count {
            return start..<min(total, start + count)
        }
        let lower = max(0, min(visible.lowerBound, max(0, total - 1)))
        return lower..<min(total, lower + max(1, visible.count + slack))
    }

    private func reveal(row: Int, column: Int?) {
        let metrics = table.rowMetrics
        var next = model.offset
        let top = CGFloat(metrics.top(row)), bottom = top + CGFloat(metrics.height(row))
        let visibleHeight = max(CGFloat(metrics.height(row)), viewport.size.height)
        if top < next.y { next.y = top }
        else if bottom > next.y + visibleHeight { next.y = max(0, bottom - visibleHeight) }
        if let column, column >= table.geometry.frozenCount {
            let geometry = table.geometry
            let left = CGFloat(geometry.starts[column] - geometry.frozenWidth)
            let right = left + CGFloat(geometry.widths[column])
            let visibleWidth = max(CGFloat(geometry.widths[column]), viewport.size.width)
            if left < next.x { next.x = left }
            else if right > next.x + visibleWidth { next.x = max(0, right - visibleWidth) }
        }
        model.setOffset(next)
        refreshWindows(offset: next)
    }

    private func scrollWheel(_ event: MouseWheelEvent) -> Bool {
        let geometry = table.geometry
        let metrics = table.rowMetrics
        let old = model.offset
        let delta = event.displacement(lineStep: 30, pageWidth: Float(viewport.size.width),
                                       pageHeight: Float(viewport.size.height))
        let maximumX = max(0, CGFloat(geometry.movingWidth) - viewport.size.width)
        let maximumY = max(0, CGFloat(metrics.total) - viewport.size.height)
        let next = CGPoint(x: max(0, min(maximumX, old.x - CGFloat(delta.x))),
                           y: max(0, min(maximumY, old.y - CGFloat(delta.y))))
        model.setOffset(next)
        refreshWindows(offset: next)
        externalWheelAdoption &+= 1
        return (delta.x != 0 && maximumX > 0) || (delta.y != 0 && maximumY > 0)
    }
}

private struct DataTableHeaderScroller<Record: Sendable, ID: Hashable & Sendable>: View {
    let table: DataTable<Record, ID>
    let geometry: DataTableColumnGeometry
    let viewport: ScrollViewport
    let model: DataTableScrollMetrics
    let onWheel: (MouseWheelEvent) -> Bool
    @Observed<DataTableScrollMetrics, Int> private var start: Int
    @Observed<DataTableScrollMetrics, Int> private var count: Int

    init(table: DataTable<Record, ID>, geometry: DataTableColumnGeometry, viewport: ScrollViewport,
         model: DataTableScrollMetrics, onWheel: @escaping (MouseWheelEvent) -> Bool) {
        self.table = table; self.geometry = geometry; self.viewport = viewport
        self.model = model; self.onWheel = onWheel
        _start = Observed(\.columnWindowStart, on: model)
        _count = Observed(\.columnWindowCount, on: model)
    }

    var body: some View {
        let end = min(geometry.widths.count, max(start + 1, start + count))
        let moving = max(geometry.frozenCount, start)..<end
        return table.header(geometry: geometry, moving: moving, viewport: viewport,
                            model: model, onWheel: onWheel)
    }
}

private struct DataTableFrozenRows<Record: Sendable, ID: Hashable & Sendable>: View {
    let table: DataTable<Record, ID>
    let metrics: DataTableRowMetrics
    let geometry: DataTableColumnGeometry
    let frozenCount: Int
    let model: DataTableScrollMetrics
    let width: Float
    let height: Float
    @Observed<DataTableScrollMetrics, Int> private var start: Int
    @Observed<DataTableScrollMetrics, Int> private var count: Int

    init(table: DataTable<Record, ID>, metrics: DataTableRowMetrics, geometry: DataTableColumnGeometry,
         frozenCount: Int, model: DataTableScrollMetrics, width: Float, height: Float) {
        self.table = table; self.metrics = metrics; self.geometry = geometry
        self.frozenCount = frozenCount; self.model = model; self.width = width; self.height = height
        _start = Observed(\.rowWindowStart, on: model)
        _count = Observed(\.rowWindowCount, on: model)
    }

    var body: some View {
        let end = min(metrics.count, max(start + 1, start + count))
        return table.rowWindow(max(0, start)..<end, columns: 0..<min(frozenCount, geometry.widths.count),
                               geometry: geometry, metrics: metrics)
            .frame(width: width, height: height)
    }
}

private struct DataTableMovingRows<Record: Sendable, ID: Hashable & Sendable>: View {
    let table: DataTable<Record, ID>
    let metrics: DataTableRowMetrics
    let geometry: DataTableColumnGeometry
    let model: DataTableScrollMetrics
    @Observed<DataTableScrollMetrics, Int> private var rowStart: Int
    @Observed<DataTableScrollMetrics, Int> private var rowCount: Int
    @Observed<DataTableScrollMetrics, Int> private var columnStart: Int
    @Observed<DataTableScrollMetrics, Int> private var columnCount: Int

    init(table: DataTable<Record, ID>, metrics: DataTableRowMetrics, geometry: DataTableColumnGeometry,
         model: DataTableScrollMetrics) {
        self.table = table; self.metrics = metrics; self.geometry = geometry; self.model = model
        _rowStart = Observed(\.rowWindowStart, on: model)
        _rowCount = Observed(\.rowWindowCount, on: model)
        _columnStart = Observed(\.columnWindowStart, on: model)
        _columnCount = Observed(\.columnWindowCount, on: model)
    }

    var body: some View {
        let rowEnd = min(metrics.count, max(rowStart + 1, rowStart + rowCount))
        let columnEnd = min(geometry.widths.count, max(columnStart + 1, columnStart + columnCount))
        return table.rowWindow(max(0, rowStart)..<rowEnd,
                               columns: max(geometry.frozenCount, columnStart)..<columnEnd,
                               geometry: geometry, metrics: metrics)
    }
}

/// Translates its content through `contentOffset` without rebuilding it.
/// Header and frozen-lane scrolling therefore moves existing nodes; only the
/// virtual window changes rebuild content.
private struct DataTableOffsetPane<Content: View>: _PrimitiveView {
    enum Axis { case horizontal, vertical }
    let model: DataTableScrollMetrics
    let axis: Axis
    let content: Content

    func _makeNode() -> Node { Node() }
    func _makeLayoutNode() -> LayoutNode? {
        let layout = LayoutNode()
        layout.alignItems = .stretch
        return layout
    }
    func _updateNode(_ node: Node) {
        switch axis {
        case .horizontal:
            model.horizontalPane = node
            node.contentOffset.x = model.offset.x
        case .vertical:
            model.verticalPane = node
            node.contentOffset.y = model.offset.y
        }
    }
    var _children: [any View] { [content] }
}
