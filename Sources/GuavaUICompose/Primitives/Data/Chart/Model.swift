import GuavaUIRuntime

/// How a series is drawn. Marks can be mixed in one chart; when any series
/// uses bars, every mark is centered on the bar slots so they line up.
public enum ChartMark: Sendable, Equatable {
    case line
    /// A line over a translucent fill down to the baseline (or the series below when stacked).
    case area
    case bar
    /// Unconnected points; with a `.numeric` x domain this is a scatter plot.
    case point
}

/// One plotted value and the x position it belongs to.
///
/// A non-finite `y` is a gap: the line breaks there instead of drawing a value
/// that was never measured.
public struct ChartDatum: Sendable, Equatable {
    public var x: Float
    public var y: Float

    public init(x: Float, y: Float) {
        self.x = x
        self.y = y
    }
}

/// How x positions are read for every series in one chart.
///
/// The domain belongs to the chart, not to a series: mixing a bar series and a
/// scatter series reads both off the same axis.
public enum ChartXDomain: Sendable, Equatable {
    /// Evenly spaced slots, one per datum, with an optional label each. Labels
    /// that would collide are dropped automatically.
    case categorical([String])
    /// Every datum's own `x`, shared across series and ticked on round values.
    case numeric

    public var isNumeric: Bool {
        if case .numeric = self { return true }
        return false
    }
}

/// Which value axis a series reads.
///
/// The left axis fits itself to the series on it and the right axis to its own,
/// so a count and a rate share one plot without either flattening the other.
/// Stacking, thresholds and bands are per axis too.
public enum ChartValueAxis: Sendable, Equatable {
    case left
    case right
}

/// How an axis spaces its values.
public enum ChartScaleKind: Sendable, Equatable {
    /// Even spacing per unit of value.
    case linear
    /// Even spacing per power of ten, for data that spans orders of magnitude.
    /// Ticks land on 1, 2 and 5 × 10ⁿ. Values at or below zero have no place on
    /// the axis and clamp to the bottom edge.
    case log
}

/// Per-series drawing overrides. `ChartStyle` holds the chart-wide defaults.
public struct ChartSeriesStyle: Sendable, Equatable {
    /// Stroke width; nil follows `ChartStyle.lineWidth`.
    public var lineWidth: Float?
    /// Alternating on/off lengths in points. Fewer than two entries draws a
    /// solid stroke.
    public var dash: [Float]
    /// Smooths line and area runs into a monotone cubic instead of straight
    /// segments. Runs whose x is not strictly increasing stay straight.
    public var curve: Bool
    /// Radius of `point` marks.
    public var pointRadius: Float
    /// Alpha of the area fill.
    public var fillOpacity: Float
    /// Writes each value next to its mark, formatted by
    /// `ChartConfiguration.dataLabels.format`. Labels that would collide are
    /// dropped, so a dense series stays readable.
    public var showsValues: Bool

    public init(lineWidth: Float? = nil, dash: [Float] = [], curve: Bool = false,
                pointRadius: Float = 3.5, fillOpacity: Float = 0.2, showsValues: Bool = false) {
        self.lineWidth = lineWidth.map { max($0, 0.5) }
        self.dash = dash.map { max($0, 0) }
        self.curve = curve
        self.pointRadius = max(pointRadius, 0.5)
        self.fillOpacity = min(max(fillOpacity, 0), 1)
        self.showsValues = showsValues
    }
}

public struct ChartSeries: Sendable {
    /// The plotted values, in x order.
    public var data: [ChartDatum]
    public var color: SemanticColorRef
    public var mark: ChartMark
    /// Legend and selection name. An empty title keeps the series out of the legend.
    public var title: String
    /// The value axis this series reads. A chart with any `.right` series grows
    /// a right-hand axis fitted to those series alone.
    public var axis: ChartValueAxis
    public var style: ChartSeriesStyle

    /// A series on evenly spaced slots; each datum's x is its index.
    public init(_ values: [Float], color: SemanticColorRef = .accent,
                mark: ChartMark = .line, title: String = "",
                axis: ChartValueAxis = .left,
                style: ChartSeriesStyle = ChartSeriesStyle()) {
        self.data = values.enumerated().map { ChartDatum(x: Float($0.offset), y: $0.element) }
        self.color = color
        self.mark = mark
        self.title = title
        self.axis = axis
        self.style = style
    }

    /// A series on explicit x positions, for a `.numeric` x domain. A line or
    /// area series is drawn in data order, so sort by x first.
    public init(points: [ChartDatum], color: SemanticColorRef = .accent,
                mark: ChartMark = .line, title: String = "",
                axis: ChartValueAxis = .left,
                style: ChartSeriesStyle = ChartSeriesStyle()) {
        self.data = points
        self.color = color
        self.mark = mark
        self.title = title
        self.axis = axis
        self.style = style
    }

    public static func line(_ values: [Float], color: SemanticColorRef = .accent, title: String = "",
                            axis: ChartValueAxis = .left,
                            style: ChartSeriesStyle = ChartSeriesStyle()) -> ChartSeries {
        ChartSeries(values, color: color, mark: .line, title: title, axis: axis, style: style)
    }

    public static func area(_ values: [Float], color: SemanticColorRef = .accent, title: String = "",
                            axis: ChartValueAxis = .left,
                            style: ChartSeriesStyle = ChartSeriesStyle()) -> ChartSeries {
        ChartSeries(values, color: color, mark: .area, title: title, axis: axis, style: style)
    }

    public static func bar(_ values: [Float], color: SemanticColorRef = .accent, title: String = "",
                           axis: ChartValueAxis = .left,
                           style: ChartSeriesStyle = ChartSeriesStyle()) -> ChartSeries {
        ChartSeries(values, color: color, mark: .bar, title: title, axis: axis, style: style)
    }

    /// A scatter series; `x` and `y` are paired by index and truncated to the shorter one.
    public static func points(x: [Float], y: [Float], color: SemanticColorRef = .accent,
                              title: String = "", axis: ChartValueAxis = .left,
                              style: ChartSeriesStyle = ChartSeriesStyle()) -> ChartSeries {
        let count = min(x.count, y.count)
        let points = (0..<count).map { ChartDatum(x: x[$0], y: y[$0]) }
        return ChartSeries(points: points, color: color, mark: .point, title: title, axis: axis, style: style)
    }

    /// A scatter series over prepared points.
    public static func points(_ points: [ChartDatum], color: SemanticColorRef = .accent,
                              title: String = "", axis: ChartValueAxis = .left,
                              style: ChartSeriesStyle = ChartSeriesStyle()) -> ChartSeries {
        ChartSeries(points: points, color: color, mark: .point, title: title, axis: axis, style: style)
    }
}

/// One part of a pie or donut chart. Without a color, slices take the
/// categorical palette in order.
public struct ChartSlice: Sendable {
    public var value: Float
    public var title: String
    public var color: SemanticColorRef?

    public init(_ title: String, value: Float, color: SemanticColorRef? = nil) {
        self.title = title
        self.value = value
        self.color = color
    }
}

public struct ChartThreshold: Sendable {
    public var value: Float
    public var color: SemanticColorRef
    /// Drawn at the right end of the rule. Empty draws no text.
    public var label: String
    /// The value axis the rule belongs to; a rule on the right axis measures
    /// against that axis, not the left one.
    public var axis: ChartValueAxis

    public init(value: Float, color: SemanticColorRef = .warning, label: String = "",
                axis: ChartValueAxis = .left) {
        self.value = value
        self.color = color
        self.label = label
        self.axis = axis
    }
}

/// A shaded range on the value axis: a target corridor, a healthy band.
public struct ChartBand: Sendable {
    public var from: Float
    public var to: Float
    public var color: SemanticColorRef
    /// Drawn at the top-left corner of the band. Empty draws no text.
    public var label: String
    /// The value axis the band belongs to.
    public var axis: ChartValueAxis

    public init(from: Float, to: Float, color: SemanticColorRef = .accent, label: String = "",
                axis: ChartValueAxis = .left) {
        self.from = min(from, to)
        self.to = max(from, to)
        self.color = color
        self.label = label
        self.axis = axis
    }
}

/// A short note pinned to one value, drawn as a pill above the mark.
public struct ChartAnnotation: Sendable {
    public var text: String
    /// The series whose mark the note points at; 0 for a single-series chart.
    public var series: Int
    public var index: Int
    public var color: SemanticColorRef

    public init(_ text: String, series: Int = 0, index: Int, color: SemanticColorRef = .accent) {
        self.text = text
        self.series = series
        self.index = index
        self.color = color
    }
}

public struct ChartMarker: Sendable {
    public var index: Int
    public var color: SemanticColorRef
    public var width: Float

    public init(index: Int, color: SemanticColorRef = .accent, width: Float = 1) {
        self.index = index
        self.color = color
        self.width = max(width, 1)
    }
}

/// Whether bar and area series accumulate per index.
public enum ChartStacking: Sendable, Equatable {
    case none
    /// Series accumulate on each other: positives rise from zero, negatives
    /// fall from it, so a bar can cross the baseline.
    case stacked
    /// Like `stacked`, but each index is scaled so its taller side reaches 100.
    /// The axis then reads 0…100 (or −100…100 with negatives), and every
    /// series takes part — a line in a percent chart is its own share.
    case percent
}

/// Which screen direction the value axis runs.
///
/// A vertical chart puts categories across the bottom and values up the side.
/// A horizontal one puts categories down the left and values along the bottom,
/// which is what long category names and ranked bars need.
///
/// Orientation is a drawing decision, not a config change: `axis.y` still
/// configures the value axis and `axis.x` the independent one, whichever edge
/// each is drawn against. A log value axis stays `axis.y.kind = .log` when the
/// chart turns.
public enum ChartOrientation: Sendable, Equatable {
    case vertical
    case horizontal
}

public struct ChartStyle: Sendable {
    public var minValue: Float?
    public var maxValue: Float?
    /// Draws a line at every value-axis tick. The tick values come from
    /// `ChartConfiguration.axis.y.tickCount`, so grid lines land on round numbers.
    public var showsGrid: Bool
    public var lineWidth: Float
    public var barSpacing: Float
    public var contentInset: Float
    public var background: SemanticColorRef?
    public var gridColor: SemanticColorRef
    public var stacking: ChartStacking
    /// Donut hole as a fraction of the radius; 0 draws a full pie.
    public var innerRadiusRatio: Float
    /// Which way the value axis runs. Ignored by a pie.
    public var orientation: ChartOrientation

    public init(minValue: Float? = nil,
                maxValue: Float? = nil,
                showsGrid: Bool = true,
                lineWidth: Float = 2,
                barSpacing: Float = 2,
                contentInset: Float = 4,
                background: SemanticColorRef? = nil,
                gridColor: SemanticColorRef = .divider,
                stacking: ChartStacking = .none,
                innerRadiusRatio: Float = 0.6,
                orientation: ChartOrientation = .vertical) {
        self.minValue = minValue
        self.maxValue = maxValue
        self.showsGrid = showsGrid
        self.lineWidth = max(lineWidth, 0.5)
        self.barSpacing = max(barSpacing, 0)
        self.contentInset = max(contentInset, 0)
        self.background = background
        self.gridColor = gridColor
        self.stacking = stacking
        self.innerRadiusRatio = min(max(innerRadiusRatio.isFinite ? innerRadiusRatio : 0, 0), 0.9)
        self.orientation = orientation
    }

    /// A compact trend line: no grid, a thin stroke and a small inset. Pair it
    /// with `axis.x.isLabeled = false` and `axis.y.isLabeled = false`.
    public static var sparkline: ChartStyle { ChartStyle(showsGrid: false, lineWidth: 1.5, contentInset: 3) }
}

/// A selected value: the series index (or 0 for a pie) and the value index (or slice).
public struct ChartPoint: Hashable, Sendable {
    public var series: Int
    public var index: Int

    public init(series: Int, index: Int) {
        self.series = series
        self.index = index
    }
}

/// Interactive behavior, configured in `Chart(...) { $0.selection.point = $point }`.
public struct ChartConfiguration {
    public struct Selection {
        /// Enables hover, tap/drag scrubbing, keyboard and assistive selection.
        public var point: Binding<ChartPoint?>?
        /// Drag to brush an interval on the independent axis. Numeric charts
        /// return x values; categorical charts return zero-based indices.
        /// Supplying this binding makes a primary drag brush instead of scrub.
        public var range: Binding<ClosedRange<Float>?>?
        /// Formats values in the callout and accessibility value.
        public var valueText: (Float) -> String = Selection.defaultText

        /// Whole numbers without decimals, the rest to two places.
        ///
        /// The chart, its data labels and `ChartDataTable` all start here, so
        /// one value never reads two ways in the same chart.
        public static func defaultText(_ value: Float) -> String {
            value.rounded() == value && abs(value) < 1e9 ? String(Int(value)) : String(format: "%.2f", value)
        }
    }

    public struct Legend {
        public enum Placement: Sendable, Equatable { case hidden, bottom }
        public var placement: Placement = .hidden
        /// Titles of hidden series. Legend items toggle membership; without a
        /// binding the chart keeps the hidden set itself.
        public var hiddenSeries: Binding<Set<String>>?
    }

    /// Value labels written next to each mark. A series opts in with
    /// `ChartSeriesStyle.showsValues`.
    public struct DataLabels {
        /// Formats a value. Without it the selection formatter is used, so a
        /// label and the callout read the same number.
        public var format: ((Float) -> String)?

        public init() {}
    }

    /// Pie and donut behavior.
    public struct Pie {
        /// Text in the donut hole.
        public struct Center: Equatable, Sendable {
            public var value: String
            public var label: String

            public init(_ value: String, label: String = "") {
                self.value = value
                self.label = label
            }
        }

        /// Where the first slice starts, in radians clockwise from 12 o'clock.
        public var startAngle: Float = 0
        /// Draws the largest slice first. Each slice keeps its own color and
        /// title, so only the order changes.
        public var sortsByValue = false
        /// Writes each slice's title and share outside the ring, joined to its
        /// slice by a leader line.
        public var showsLabels = false
        /// Content drawn in the hole; ignored when `innerRadiusRatio` is 0.
        public var center: Center?

        public init() {}
    }

    /// The axis chrome: tick labels and titles around the plot.
    public struct Axis {
        public struct X {
            public var title: String = ""
            /// Requested tick intervals for a `.numeric` domain. Category axes
            /// tick once per label instead.
            public var tickCount: Int = 5
            /// Draws the labels under the plot. Labels also reserve the bottom
            /// gutter, so a sparkline turns this off.
            public var isLabeled: Bool = true
            /// Linear or logarithmic spacing. Only a numeric domain reads it.
            public var kind: ChartScaleKind = .linear
            /// Formats numeric tick labels. Without it the scale picks the
            /// precision that renders the tick step exactly.
            public var format: ((Float) -> String)?
        }

        public struct Y {
            public var title: String = ""
            /// Requested number of tick intervals. Ticks land on round values,
            /// so the count is approximate; zero draws no ticks or grid.
            public var tickCount: Int = 4
            /// Draws the tick labels in the left gutter.
            public var isLabeled: Bool = true
            /// Linear or logarithmic spacing.
            public var kind: ChartScaleKind = .linear
            /// Formats tick labels. Without it the scale picks the precision
            /// that renders the tick step exactly.
            public var format: ((Float) -> String)?
        }

        public var x = X()
        public var y = Y()
        /// The right-hand value axis. It is only drawn — and only reserves a
        /// gutter — when some series reads it.
        public var secondary = Y()

        public init() {}
    }

    /// What a reader who cannot see the plot is given instead.
    public struct Accessibility {
        /// Gives every x position its own stop: a screen reader steps through
        /// the values and hears each visible series at each one. Off leaves the
        /// chart a single sentence describing the range.
        ///
        /// This needs no selection binding — a read-only chart still reads
        /// value by value, it just cannot be clicked.
        public var exposesValues = true

        public init() {}
    }

    public var selection = Selection()
    public var legend = Legend()
    public var axis = Axis()
    public var dataLabels = DataLabels()
    public var pie = Pie()
    public var accessibility = Accessibility()
    public var viewport = Viewport()
    /// Data changes interpolate using this animation, or the surrounding
    /// `withAnimation` transaction. Reduced motion always reaches the target immediately.
    public var animation: Animation?
    /// Accessible name of the chart.
    public var label = "Chart"

    public init() {}
}

extension ChartSlice {
    /// Distinct hues first; `info` is a second blue in the default themes, so it comes last.
    static let palette: [SemanticColorRef] = [.accent, .success, .warning, .accentSecondary, .error, .info]
    func resolvedColor(at index: Int) -> SemanticColorRef { color ?? Self.palette[index % Self.palette.count] }
}
