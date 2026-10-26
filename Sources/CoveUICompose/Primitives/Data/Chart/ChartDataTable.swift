import CoveUIRuntime

/// A chart's data as a table: one row per x position, one column per series.
///
/// The plot is a picture of this table, and a picture is the one thing a
/// screen reader cannot describe. Here the container carries the table role,
/// each row a row role and each value a cell role, so a reader can move by row
/// and column and always hear which series a number belongs to.
///
/// ```swift
/// Chart(series, x: .categorical(months)).frame(height: 180)
/// ChartDataTable(series, x: .categorical(months), label: "Revenue by month")
/// ```
///
/// It is a view of its own rather than a chart option because a table needs
/// room the plot does not have: place it under the chart, beside it, or behind
/// a "Show data" disclosure. `maximumRows` bounds the nodes it creates; past
/// that the table says how many rows it left out rather than growing without
/// limit.
///
/// A numeric x axis gives each series its own x positions, so there is no
/// shared row to put them on. Those charts — and only those — list one datum
/// per row, naming its series and both of its values.
public struct ChartDataTable: View {
    /// One row: what it is about, and one cell per column.
    struct Entry: Equatable {
        let label: String
        let cells: [String]
    }

    private let label: String
    private let axisLabel: String
    private let columns: [String]
    private let rows: [Entry]
    /// Rows the table did not have room for.
    private let omitted: Int

    /// Series over an evenly spaced or numeric x axis.
    ///
    /// - Parameters:
    ///   - x: the chart's x domain; a numeric one pairs rows by the first
    ///     series' own x values.
    ///   - label: the table's accessible name.
    ///   - axisLabel: heading over the label column. Empty leaves it blank.
    ///   - maximumRows: rows to build before the table starts counting what it
    ///     left out.
    ///   - format: a value as text; the chart's default formatter by default.
    public init(_ series: [ChartSeries],
                x: ChartXDomain = .categorical([]),
                label: String = "Data",
                axisLabel: String = "",
                maximumRows: Int = 100,
                format: ((Float) -> String)? = nil) {
        let text = format ?? ChartConfiguration.Selection.defaultText
        let limit = max(1, maximumRows)
        let shown = series.indices.filter { !series[$0].data.isEmpty }
        self.label = label

        // One row per datum when the series do not share their x positions:
        // a wide table would pair values that were never measured together.
        if x.isNumeric, shown.count > 1 {
            let entries = shown.flatMap { index in
                series[index].data.map { datum in
                    Entry(label: Self.name(series[index].title, index: index),
                          cells: [text(datum.x), datum.y.isFinite ? text(datum.y) : Self.gap])
                }
            }
            self.axisLabel = axisLabel.isEmpty ? "Series" : axisLabel
            self.columns = ["X", "Value"]
            self.rows = Array(entries.prefix(limit))
            self.omitted = max(0, entries.count - limit)
            return
        }

        let first = shown.first.map { series[$0] }
        let count = shown.map { series[$0].data.count }.max() ?? 0
        self.axisLabel = axisLabel
        self.columns = shown.map { Self.name(series[$0].title, index: $0) }
        self.rows = (0..<min(count, limit)).map { position in
            Entry(label: Self.position(x, index: position, series: first, text: text),
                  cells: shown.map { index in
                      let data = series[index].data
                      guard data.indices.contains(position) else { return Self.gap }
                      return data[position].y.isFinite ? text(data[position].y) : Self.gap
                  })
        }
        self.omitted = max(0, count - limit)
    }

    /// Pie and donut slices, with each slice's share of the total.
    public init(slices: [ChartSlice], label: String = "Data", maximumRows: Int = 100,
                format: ((Float) -> String)? = nil) {
        let text = format ?? ChartConfiguration.Selection.defaultText
        let limit = max(1, maximumRows)
        let total = slices.reduce(Float(0)) { $0 + max(0, $1.value.isFinite ? $1.value : 0) }
        self.label = label
        self.axisLabel = ""
        self.columns = ["Value", "Share"]
        self.rows = slices.prefix(limit).enumerated().map { position, slice in
            let share = total > 0 ? Int((max(0, slice.value) / total * 100).rounded()) : 0
            return Entry(label: slice.title.isEmpty ? "Slice \(position + 1)" : slice.title,
                         cells: [text(slice.value), total > 0 ? "\(share)%" : Self.gap])
        }
        self.omitted = max(0, slices.count - limit)
    }

    public var body: some View {
        Box(direction: .column, alignItems: .stretch) {
            ChartDataTableRow(label: axisLabel, columns: columns, cells: columns, isHeader: true)
                .background(.surfaceVariant)
            Divider()
            for row in rows {
                ChartDataTableRow(label: row.label, columns: columns, cells: row.cells, isHeader: false)
            }
            if omitted > 0 {
                Text("\(omitted) more \(omitted == 1 ? "row" : "rows") not shown")
                    .font(.caption).foregroundColor(.onSurfaceMuted)
                    .padding(horizontal: 8, vertical: 6)
                    .accessibility { $0.role = .status }
            }
        }
        .accessibility {
            $0.role = .table
            $0.label = label
        }
        .debugName("chart-data-table")
    }

    /// A value that was never measured, or a slot the series does not reach.
    private static let gap = "—"

    private static func name(_ title: String, index: Int) -> String {
        title.isEmpty ? "Series \(index + 1)" : title
    }

    /// What the x axis calls this position: the category name, or the value on
    /// a numeric axis.
    private static func position(_ x: ChartXDomain, index: Int, series: ChartSeries?,
                                 text: (Float) -> String) -> String {
        switch x {
        case .categorical(let labels):
            return labels.indices.contains(index) ? labels[index] : "\(index + 1)"
        case .numeric:
            guard let series, series.data.indices.contains(index) else { return "\(index + 1)" }
            return text(series.data[index].x)
        }
    }
}

/// One table row: the label column, then a cell per series. The header reuses
/// the same row so the columns line up without a second layout.
private struct ChartDataTableRow: View {
    let label: String
    /// One name per value column, so a cell can say which series it came from.
    let columns: [String]
    let cells: [String]
    let isHeader: Bool

    var body: some View {
        Row(alignment: .center, spacing: 8) {
            for item in items {
                ChartDataTableCell(text: item.text, column: item.column, isHeader: isHeader)
            }
        }
        .padding(horizontal: 8, vertical: 5)
        .accessibility {
            $0.role = .row
            $0.label = isHeader ? "Columns" : label
        }
    }

    /// The label column first, then one cell per series.
    private var items: [(text: String, column: String?)] {
        var items: [(text: String, column: String?)] = [(label, nil)]
        for (index, cell) in cells.enumerated() {
            items.append((cell, columns.indices.contains(index) ? columns[index] : nil))
        }
        return items
    }
}

/// A header cell is just a heading; a body cell is a value that names the
/// column it came from, which is what a screen reader reads before it.
private struct ChartDataTableCell: View {
    let text: String
    /// The series this cell belongs to. Nil for the label column.
    let column: String?
    let isHeader: Bool

    var body: some View {
        Text(text, alignment: column == nil ? .leading : .trailing)
            .lineLimit(1)
            .font(isHeader ? .label : .caption)
            .foregroundColor(isHeader ? .onSurface : .onSurfaceVariant)
            .flex(column == nil ? 1.2 : 1, shrink: 1)
            .accessibility {
                guard !isHeader, let column else { return }
                $0.role = .cell
                $0.label = column
                $0.value = text
            }
    }
}
