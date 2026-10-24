internal import Foundation
import GuavaUIRuntime

/// The two screen directions a chart reads in.
///
/// Everything below is written once and projected through these helpers: a
/// mark has a position *across* the categories and a position *along* the
/// value axis, and only `point(across:along:)` decides which screen coordinate
/// each one lands on. That is what keeps a horizontal chart from being a second
/// copy of the renderer.
extension ChartOrientation {
    /// Plot extent across the categories.
    func breadth(_ plot: UIRect) -> Float { self == .vertical ? plot.width : plot.height }
    /// Plot extent along the values.
    func length(_ plot: UIRect) -> Float { self == .vertical ? plot.height : plot.width }
    /// The plot's origin along the across axis.
    func acrossOrigin(_ plot: UIRect) -> Float { self == .vertical ? plot.minX : plot.minY }
    /// Screen coordinate of a value on the value axis.
    func along(_ scale: ChartScale, _ value: Float, in plot: UIRect) -> Float {
        self == .vertical ? scale.y(value, in: plot) : scale.x(value, in: plot)
    }
    /// Screen coordinate of a value on the independent axis.
    func across(_ scale: ChartScale, _ value: Float, in plot: UIRect) -> Float {
        self == .vertical ? scale.x(value, in: plot) : scale.y(value, in: plot)
    }
    /// Screen point for a position across the categories and one along the
    /// value axis. The one place the two directions swap.
    func point(across: Float, along: Float) -> (x: Float, y: Float) {
        self == .vertical ? (across, along) : (along, across)
    }
    /// A screen point taken apart into its two chart directions.
    func axes(of point: (x: Float, y: Float)) -> (across: Float, along: Float) {
        self == .vertical ? (point.x, point.y) : (point.y, point.x)
    }
}

/// Plot coordinates for one chart, shared by drawing, hit-testing and the
/// selection callout so all three agree on where a value is.
struct ChartGeometry {
    struct Mark {
        let series: Int
        let index: Int
        let value: Float
        /// Screen position of the value.
        let x: Float
        let y: Float
        /// Screen coordinate along the value axis where the mark rests: the
        /// zero line, or the segment below it when stacked.
        let base: Float
        /// Bar rectangle in screen space; nil for other marks.
        let bar: UIRect?
        /// The bar end away from the baseline is exposed (not covered by a stacked segment).
        let isOuterEnd: Bool
    }

    let plot: UIRect
    /// Both value axes: a mark resolves its value through the one its series reads.
    let axes: ChartValueAxes
    /// Marks per visible series, in series order. Hidden series have none.
    let marks: [[Mark]]
    let hasBars: Bool
    let sampleCount: Int
    /// A numeric x domain hit-tests by true distance.
    let isScatter: Bool
    /// Which screen direction the value axis runs.
    let orientation: ChartOrientation
    let window: ChartWindow?

    var domain: ClosedRange<Float> { axes.primaryScale.domain }

    init(series: [ChartSeries], axes: ChartValueAxes, xDomain: ChartXDomain, xScale: ChartScale?,
         barSpacing: Float, orientation: ChartOrientation = .vertical, plot: UIRect, window: ChartWindow? = nil) {
        self.plot = plot
        self.axes = axes
        self.orientation = orientation
        self.window = window
        let shown = axes.shown
        let slotCount = axes.sampleCount
        hasBars = axes.hasBars
        sampleCount = slotCount
        isScatter = xDomain.isNumeric
        let stacked = axes.isStacked
        let numericX = xDomain.isNumeric ? xScale : nil

        let barSeries = shown.filter { series[$0].mark == .bar }
        let groups = stacked ? 1 : max(1, barSeries.count)
        let slot = window.flatMap { window -> Float? in
            guard !xDomain.isNumeric else { return nil }
            return orientation.breadth(plot) / max(0.001, window.range.upperBound - window.range.lowerBound)
        } ?? numericX.map {
            Self.numericSlot(series: series, shown: shown, scale: $0, plot: plot, orientation: orientation, window: window)
        } ?? (orientation.breadth(plot) / Float(max(1, sampleCount)))
        // Wide categories breathe (a quarter of the slot, at most 24pt); dense
        // ones keep the authored spacing. Bars inside a group sit 2pt apart.
        let categoryGap = max(barSpacing, min(slot * 0.25, 24))
        let innerGap: Float = groups > 1 ? 2 : 0
        let thickness = max(1, (slot - categoryGap - Float(groups - 1) * innerGap) / Float(groups))
        let origin = orientation.acrossOrigin(plot)

        var marks = [[Mark]](repeating: [], count: series.count)
        for index in shown {
            let item = series[index]
            let stack = axes.stack(item.axis)
            let scale = axes.scale(item.axis)
            let group = stacked ? 0 : (barSeries.firstIndex(of: index) ?? 0)
            // Only bars take a group offset; lines keep the slot center.
            let groupOffset = (Float(group) - Float(groups - 1) / 2) * (thickness + innerGap)
            marks[index] = item.data.indices.map { i in
                let datum = item.data[i]
                let center: Float
                if let window {
                    center = window.position(xDomain.isNumeric ? datum.x : Float(i), plot: plot, orientation: orientation)
                } else if let numericX {
                    center = orientation.across(numericX, datum.x, in: plot)
                } else if item.mark == .bar || stack.hasBars {
                    center = origin + (Float(i) + 0.5) * slot
                } else {
                    center = Self.across(index: i, count: slotCount, hasBars: false,
                                         plot: plot, orientation: orientation)
                }
                let across = item.mark == .bar ? center + groupOffset : center
                guard datum.y.isFinite, !xDomain.isNumeric || (datum.x.isFinite && (window?.kind != .log || datum.x > 0)) else {
                    return Mark(series: index, index: i, value: datum.y, x: .nan, y: .nan,
                                base: .nan, bar: nil, isOuterEnd: false)
                }
                let along = orientation.along(scale, stack.tops[index][i], in: plot)
                let base = orientation.along(scale, stack.bases[index][i], in: plot)
                let point = orientation.point(across: across, along: along)
                var bar: UIRect?
                if item.mark == .bar {
                    // Two opposite corners of the bar, whichever way it runs.
                    let a = orientation.point(across: across - thickness / 2, along: min(along, base))
                    let b = orientation.point(across: across + thickness / 2, along: max(along, base))
                    bar = UIRect(x: min(a.x, b.x), y: min(a.y, b.y),
                                 width: max(1, abs(b.x - a.x)), height: max(1, abs(b.y - a.y)))
                }
                return Mark(series: index, index: i, value: datum.y, x: point.x, y: point.y,
                            base: base, bar: bar,
                            isOuterEnd: stack.isOuterEnd(series: index, at: i, value: datum.y))
            }
        }
        self.marks = marks
    }

    /// Bars on a numeric axis are as thick as the closest pair of x values.
    private static func numericSlot(series: [ChartSeries], shown: [Int], scale: ChartScale,
                                    plot: UIRect, orientation: ChartOrientation, window: ChartWindow?) -> Float {
        let xs = shown.flatMap { series[$0].data.map(\.x) }.filter(\.isFinite).sorted()
        var gap = Float.greatestFiniteMagnitude
        if xs.count > 1 {
            for i in 1..<xs.count {
                let a = window?.position(xs[i], plot: plot, orientation: orientation) ?? orientation.across(scale, xs[i], in: plot)
                let b = window?.position(xs[i - 1], plot: plot, orientation: orientation) ?? orientation.across(scale, xs[i - 1], in: plot)
                let delta = abs(a - b)
                if delta > 0.001 { gap = min(gap, delta) }
            }
        }
        return gap < Float.greatestFiniteMagnitude ? gap : orientation.breadth(plot) / Float(max(1, xs.count))
    }

    /// Where a sample index sits across the categories.
    static func across(index: Int, count: Int, hasBars: Bool, plot: UIRect,
                       orientation: ChartOrientation = .vertical) -> Float {
        let origin = orientation.acrossOrigin(plot), length = orientation.breadth(plot)
        if hasBars { return origin + (Float(index) + 0.5) * length / Float(max(1, count)) }
        guard count > 1 else { return origin + length / 2 }
        return origin + Float(index) / Float(count - 1) * length
    }

    /// The screen coordinate along the value axis for a value.
    func along(_ value: Float, axis: ChartValueAxis = .left) -> Float {
        orientation.along(axes.scale(axis), value, in: plot)
    }

    /// A mark's screen coordinate across the categories.
    func across(_ mark: Mark) -> Float { orientation.axes(of: (mark.x, mark.y)).across }
    /// A mark's screen coordinate along the value axis.
    func along(_ mark: Mark) -> Float { orientation.axes(of: (mark.x, mark.y)).along }
    /// The screen point a mark's base sits at.
    func basePoint(_ mark: Mark) -> (x: Float, y: Float) {
        orientation.point(across: across(mark), along: mark.base)
    }

    /// The across coordinate a sample index sits at, from the first visible series.
    func across(index: Int) -> Float? {
        for marks in marks where marks.indices.contains(index) {
            let value = across(marks[index])
            if value.isFinite { return value }
        }
        return nil
    }

    /// The mark nearest a screen point: the closest column, then the series
    /// whose value is closest along the value axis (a numeric axis uses true
    /// distance). Gaps take no part.
    func nearest(x: Float, y: Float) -> Mark? {
        let all = marks.flatMap { $0 }.filter {
            $0.x.isFinite && $0.y.isFinite && across($0) >= orientation.acrossOrigin(plot)
                && across($0) <= orientation.acrossOrigin(plot) + orientation.breadth(plot)
        }
        guard !all.isEmpty else { return nil }
        if isScatter {
            return all.min { hypotf($0.x - x, $0.y - y) < hypotf($1.x - x, $1.y - y) }
        }
        let target = orientation.axes(of: (x: x, y: y))
        let column = all.min { abs(across($0) - target.across) < abs(across($1) - target.across) }!
        let candidates = all.filter { abs(across($0) - across(column)) < 0.5 }
        return candidates.min { abs(along($0) - target.along) < abs(along($1) - target.along) }
    }

    func mark(_ point: ChartPoint) -> Mark? {
        guard marks.indices.contains(point.series), marks[point.series].indices.contains(point.index) else { return nil }
        return marks[point.series][point.index]
    }
}

extension ChartGeometry {
    /// Crop before downsampling so zooming reveals detail instead of keeping
    /// the full dataset's coarse buckets. Crossing segments retain both ends.
    func visibleRun(_ run: [Mark]) -> [Mark] {
        let lower = orientation.acrossOrigin(plot), upper = lower + orientation.breadth(plot)
        func inside(_ mark: Mark) -> Bool { across(mark) >= lower && across(mark) <= upper }
        return run.indices.compactMap { i in
            if inside(run[i]) { return run[i] }
            for adjacent in [i - 1, i + 1] where run.indices.contains(adjacent) {
                let a = across(run[i]), b = across(run[adjacent])
                if min(a, b) <= upper && max(a, b) >= lower { return run[i] }
            }
            return nil
        }
    }

    /// Reduces a run to what the plot can actually resolve.
    ///
    /// Each pixel column across the categories keeps its lowest and highest
    /// value, in run order, so the drawn envelope is unchanged while the stroke
    /// costs one segment per column instead of one per sample. Picking extremes
    /// rather than every n-th sample is what keeps a spike from vanishing
    /// between two samples. A run that already fits comes back untouched.
    static func decimated(_ marks: [Mark], orientation: ChartOrientation = .vertical,
                          length: Float) -> [Mark] {
        guard marks.count > 2, length >= 1 else { return marks }
        let columns = max(1, Int(length.rounded(.up)))
        guard marks.count > columns * 2 else { return marks }
        // Rounding the bucket up keeps the bucket count at or below the column
        // count, so the result never exceeds two points per pixel.
        let bucket = max(2, Int((Float(marks.count) / Float(columns)).rounded(.up)))
        func value(_ mark: Mark) -> Float { orientation == .vertical ? mark.y : mark.x }
        var result: [Mark] = []
        result.reserveCapacity(columns * 2 + 2)
        result.append(marks[0])
        var start = 1
        while start < marks.count - 1 {
            let end = min(start + bucket, marks.count - 1)
            var low = start, high = start
            for i in start..<end {
                if value(marks[i]) < value(marks[low]) { low = i }
                if value(marks[i]) > value(marks[high]) { high = i }
            }
            // Emitted in run order, which is the order the stroke needs.
            result.append(marks[min(low, high)])
            if low != high { result.append(marks[max(low, high)]) }
            start = end
        }
        result.append(marks[marks.count - 1])
        return result
    }
}

/// Angles for a pie or donut, starting at 12 o'clock and running clockwise.
struct PieGeometry {
    struct Wedge {
        let index: Int
        let value: Float
        let start: Float
        let end: Float
    }
    let center: (x: Float, y: Float)
    let radius: Float
    let innerRadius: Float
    let wedges: [Wedge]
    let total: Float

    init(slices: [ChartSlice], visible: [Bool], innerRatio: Float,
         startAngle: Float = 0, sortsByValue: Bool = false, plot: UIRect) {
        center = (plot.minX + plot.width / 2, plot.minY + plot.height / 2)
        radius = max(0, min(plot.width, plot.height) / 2 - 4)
        innerRadius = radius * innerRatio
        let values = slices.indices.map { visible[$0] && slices[$0].value.isFinite ? max(0, slices[$0].value) : 0 }
        total = values.reduce(0, +)
        // Ordering only changes the sweep order; each wedge keeps its slice
        // index so colors, labels and hit-testing stay attached to the slice.
        var order = Array(values.indices).filter { values[$0] > 0 }
        if sortsByValue { order.sort { values[$0] > values[$1] } }
        var angle = -Float.pi / 2 + (startAngle.isFinite ? startAngle : 0)
        var wedges: [Wedge] = []
        for index in order where total > 0 {
            let sweep = values[index] / total * 2 * .pi
            wedges.append(Wedge(index: index, value: values[index], start: angle, end: angle + sweep))
            angle += sweep
        }
        self.wedges = wedges
    }

    func wedge(atX x: Float, y: Float) -> Wedge? {
        let dx = x - center.x, dy = y - center.y
        let distance = (dx * dx + dy * dy).squareRoot()
        guard distance <= radius + 6, distance >= innerRadius - 2 else { return nil }
        var angle = atan2f(dy, dx)
        if angle < -Float.pi / 2 { angle += 2 * .pi }
        return wedges.first { angle >= $0.start && angle < $0.end }
    }

    /// Point on the middle of a wedge's ring, for the callout anchor.
    func anchor(of wedge: Wedge) -> (x: Float, y: Float) {
        let middle = (wedge.start + wedge.end) / 2, r = (radius + innerRadius) / 2
        return (center.x + cosf(middle) * r, center.y + sinf(middle) * r)
    }

    /// Where the ring meets a wedge's mid-angle; the start of a leader line.
    func ringPoint(of wedge: Wedge) -> (x: Float, y: Float) {
        let middle = (wedge.start + wedge.end) / 2
        return (center.x + cosf(middle) * radius, center.y + sinf(middle) * radius)
    }

    /// Where a slice's label sits: outside the ring at the mid-angle.
    func labelAnchor(of wedge: Wedge) -> (x: Float, y: Float) {
        let middle = (wedge.start + wedge.end) / 2, r = radius + 8
        return (center.x + cosf(middle) * r, center.y + sinf(middle) * r)
    }
}
