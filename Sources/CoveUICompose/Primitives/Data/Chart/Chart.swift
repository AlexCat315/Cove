import CoveUIRuntime

/// Line, area, bar, scatter and pie/donut charts with labelled axes, an
/// optional legend and point selection.
///
/// ```swift
/// Chart([.area(revenue, title: "Revenue"), .bar(orders, color: .success, title: "Orders")],
///       x: .categorical(["Jan", "Feb", "Mar"]),
///       thresholds: [ChartThreshold(value: 30, label: "Target")],
///       bands: [ChartBand(from: 20, to: 40, color: .success, label: "Healthy")],
///       annotations: [ChartAnnotation("Launch", index: 1)]) {
///     $0.selection.point = $selected
///     $0.legend.placement = .bottom
///     $0.axis.y.title = "Revenue"
/// }
/// .frame(height: 180)
///
/// Chart([.points(x: latency, y: errors)], x: .numeric) {
///     $0.axis.x.title = "Latency (ms)"
/// }
///
/// Chart([.bar(orders, title: "Orders"),
///        .line(errorRate, color: .error, title: "Error rate", axis: .right)],
///       x: .categorical(months),
///       style: ChartStyle(stacking: .percent)) {
///     $0.axis.secondary.title = "Error %"
/// }
///
/// Chart(slices: [ChartSlice("Desktop", value: 58), ChartSlice("Mobile", value: 31)]) {
///     $0.legend.placement = .bottom
///     $0.pie.center = .init("89", label: "Total")
/// }
/// ```
///
/// The x domain belongs to the chart, so a bar series and a scatter series in
/// one chart read the same axis. Both axes fit their data to round tick values
/// and draw a grid line at each value tick, so labels never read as 11.75. Tick
/// labels reserve their own gutters: the plot shrinks to whatever the labels
/// leave.
///
/// Each side of the value axis fits its own series. `ChartSeries.axis` moves a
/// series to the right-hand axis, which only exists — and only reserves a
/// gutter — when something reads it. `ChartStyle.stacking` accumulates series
/// on each other, with positives rising from zero and negatives falling from
/// it; `.percent` scales every index to 100 so columns compare by share.
///
/// `axis.y.kind` (and `axis.x.kind`) picks linear or logarithmic spacing. A log
/// axis snaps to decades and ticks 1, 2 and 5 × 10ⁿ; values at or below zero
/// have no place on it and clamp to the bottom edge.
///
/// `style.orientation` turns the chart on its side: `.horizontal` puts the
/// categories down the left and the values along the bottom, which is what long
/// category names and ranked bars need. `axis.y` keeps configuring the value
/// axis and `axis.x` the independent one — only the edges they are drawn
/// against move, so a log value axis stays `axis.y.kind = .log` when it turns.
///
/// Bind `viewport.range` to enable wheel zoom, horizontal scroll/Alt-drag pan
/// and +/- keys (0 or double-click resets). Bind `selection.range` to brush an
/// interval by dragging; numeric charts return x values, categories return
/// zero-based indices. Both features follow a horizontal chart's category axis.
/// `animation` animates data changes, including interruptions, and also works
/// through `withAnimation`. Reduced motion snaps to the final values.
///
/// `ChartHistogram` counts samples into bins and hands back the bars plus their
/// edge labels, so a distribution needs no hand-rolled counting.
///
/// A series with more samples than the plot has pixels is drawn at the plot's
/// resolution: each column keeps its highest and lowest sample, so the envelope
/// survives while the stroke stays cheap. The resolved geometry is also cached
/// per size, so a pointer move repaints without rebuilding the stacks.
///
/// Reference geometry annotates the data without replacing it: a threshold is a
/// labelled rule, a band a shaded corridor, an annotation a note pinned to one
/// value, and `ChartSeriesStyle.showsValues` writes each value beside its mark.
///
/// Width follows the parent; the plot takes the height left after the legend
/// (64pt when unconstrained). Selection is interactive only with a binding:
/// hover previews on pointer surfaces, click/tap pins, drag scrubs, arrows
/// step, Escape clears. The callout lists every visible series at the selected
/// x. Legend items toggle series visibility. A non-finite value is a gap: the
/// line and its fill break there.
///
/// A screen reader steps through the values with or without a binding: each x
/// is one stop, announced as its category, every visible series at it, and its
/// position in the series. `accessibility.exposesValues` turns that off, and
/// `ChartDataTable` offers the same data as a table for reading it all at once.
public struct Chart: View {
    private let content: ChartContent
    private let style: ChartStyle
    private let configuration: ChartConfiguration
    @State private var ownHidden: Set<String> = []

    public init(_ series: [ChartSeries],
                x: ChartXDomain = .categorical([]),
                thresholds: [ChartThreshold] = [],
                bands: [ChartBand] = [],
                annotations: [ChartAnnotation] = [],
                markers: [ChartMarker] = [],
                style: ChartStyle = ChartStyle(),
                configure: (inout ChartConfiguration) -> Void = { _ in }) {
        self.init(.cartesian(series: series,
                             guides: ChartGuides(thresholds: thresholds, bands: bands,
                                                 annotations: annotations, markers: markers),
                             x: x),
                  style: style, configure: configure)
    }

    public init(slices: [ChartSlice],
                style: ChartStyle = ChartStyle(showsGrid: false),
                configure: (inout ChartConfiguration) -> Void = { _ in }) {
        self.init(.pie(slices), style: style, configure: configure)
    }

    private init(_ content: ChartContent, style: ChartStyle, configure: (inout ChartConfiguration) -> Void) {
        var configuration = ChartConfiguration()
        configure(&configuration)
        self.content = content
        self.style = style
        self.configuration = configuration
    }

    public var body: some View {
        let hidden = hiddenBinding
        let visible = (0..<content.count).map { index in
            let title = content.title(index)
            return title.isEmpty || !hidden.wrappedValue.contains(title)
        }
        let point = configuration.selection.point
        let selection = point?.wrappedValue.flatMap { visible.indices.contains(visibleIndex($0)) && visible[visibleIndex($0)] ? $0 : nil }
        Box(direction: .column, alignItems: .stretch, spacing: 10) {
            ChartPlot(content: content, visible: visible, style: style, axis: configuration.axis,
                      pie: configuration.pie,
                      format: configuration.dataLabels.format ?? configuration.selection.valueText,
                      selection: selection,
                      summary: summary(visible: visible, selection: selection),
                      callout: { self.callout(for: $0, visible: visible) },
                      accessibility: configuration.accessibility,
                      viewport: configuration.viewport, visibleRange: configuration.viewport.range?.wrappedValue,
                      brush: configuration.selection.range, brushedRange: configuration.selection.range?.wrappedValue,
                      animation: configuration.animation,
                      input: ChartPlotInput(isEnabled: point != nil) { next in
                          if point?.wrappedValue != next { point?.wrappedValue = next }
                      })
                .flex(1, shrink: 1)
            if configuration.legend.placement == .bottom {
                ChartLegend(items: legendItems, hidden: hidden).flex(0, shrink: 0)
            }
        }
        .frame(minWidth: 0)
    }

    /// The legend index a selection belongs to: its series, or its slice.
    private func visibleIndex(_ point: ChartPoint) -> Int {
        if case .pie = content { return point.index }
        return point.series
    }

    private var hiddenBinding: Binding<Set<String>> {
        if let binding = configuration.legend.hiddenSeries { return binding }
        return $ownHidden
    }

    private var legendItems: [ChartLegendItem] {
        switch content {
        case .cartesian(let series, _, _):
            series.compactMap { $0.title.isEmpty ? nil : ChartLegendItem(title: $0.title, color: $0.color) }
        case .pie(let slices):
            slices.indices.map { ChartLegendItem(title: slices[$0].title, color: slices[$0].resolvedColor(at: $0)) }
        }
    }

    /// The callout for one point. On a cartesian chart it lists every visible
    /// series at that x, so a reader compares them without moving the pointer.
    private func callout(for point: ChartPoint, visible: [Bool]) -> ChartCallout? {
        let format = configuration.dataLabels.format ?? configuration.selection.valueText
        switch content {
        case .cartesian(let series, _, let x):
            guard series.indices.contains(point.series),
                  series[point.series].data.indices.contains(point.index) else { return nil }
            let rows = series.indices.compactMap { index -> ChartCallout.Row? in
                guard visible.indices.contains(index), visible[index],
                      series[index].data.indices.contains(point.index) else { return nil }
                let value = series[index].data[point.index].y
                guard value.isFinite else { return nil }
                let name = series[index].title.isEmpty ? "Series \(index + 1)" : series[index].title
                return ChartCallout.Row(title: name, value: format(value), color: series[index].color)
            }
            guard !rows.isEmpty else { return nil }
            return ChartCallout(point: point, title: categoryLabel(x, index: point.index), rows: rows)
        case .pie(let slices):
            guard slices.indices.contains(point.index) else { return nil }
            let slice = slices[point.index]
            let total = slices.map { max(0, $0.value) }.reduce(0, +)
            var detail = format(slice.value)
            if total > 0 { detail += " · \(Int((slice.value / total * 100).rounded()))%" }
            let title = slice.title.isEmpty ? "Slice \(point.index + 1)" : slice.title
            // One number: the name is already the callout's title.
            return ChartCallout(point: point, title: title,
                                rows: [ChartCallout.Row(title: "", value: detail,
                                                        color: slice.resolvedColor(at: point.index))])
        }
    }

    /// What the x axis calls this position: the category name, the value on a
    /// numeric axis, or a positional fallback.
    private func categoryLabel(_ domain: ChartXDomain, index: Int) -> String {
        switch domain {
        case .categorical(let labels):
            return labels.indices.contains(index) ? labels[index] : "Point \(index + 1)"
        case .numeric:
            guard case .cartesian(let series, _, _) = content, series.indices.contains(0),
                  series[0].data.indices.contains(index) else { return "Point \(index + 1)" }
            let x = series[0].data[index].x
            return configuration.axis.x.format?(x) ?? configuration.selection.valueText(x)
        }
    }

    private func summary(visible: [Bool], selection: ChartPoint?) -> (label: String, value: String) {
        let format = configuration.dataLabels.format ?? configuration.selection.valueText
        if let selection, let callout = callout(for: selection, visible: visible) {
            let detail = callout.rows
                .map { $0.title.isEmpty ? $0.value : "\($0.title) \($0.value)" }
                .joined(separator: ", ")
            return (configuration.label, callout.title.isEmpty ? detail : "\(callout.title): \(detail)")
        }
        switch content {
        case .cartesian(let series, _, _):
            let shown = series.indices.filter { visible[$0] && !series[$0].data.isEmpty }
            let parts = shown.map { index -> String in
                let values = series[index].data.map(\.y).filter(\.isFinite)
                let name = series[index].title.isEmpty ? "Series \(index + 1)" : series[index].title
                return "\(name) \(format(values.min() ?? 0)) to \(format(values.max() ?? 0))"
            }
            return (configuration.label, parts.isEmpty ? "No data" : parts.joined(separator: "; "))
        case .pie(let slices):
            let parts = slices.indices.filter { visible[$0] }.map { "\(slices[$0].title) \(format(slices[$0].value))" }
            return (configuration.label, parts.isEmpty ? "No data" : parts.joined(separator: "; "))
        }
    }
}

extension ChartContent {
    var isPie: Bool {
        if case .pie = self { return true }
        return false
    }
}

/// The floating label for the selected value: the x it belongs to and one row
/// per visible series.
struct ChartCallout {
    struct Row {
        let title: String
        let value: String
        let color: SemanticColorRef
    }
    let point: ChartPoint
    let title: String
    let rows: [Row]
}
