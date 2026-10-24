internal import Foundation
import GuavaUIRuntime

/// Plot coordinates for one chart, shared by drawing, hit-testing and the
/// selection callout so all three agree on where a value is.
struct ChartGeometry {
    struct Mark {
        let series: Int
        let index: Int
        let value: Float
        /// Center x and the y of the value (top of a bar or stacked segment).
        let x: Float
        let y: Float
        /// The y the mark rests on: the zero line, or the segment below when stacked.
        let baseY: Float
        /// Bar column; empty for other marks.
        let bar: UIRect?
        /// The bar end away from the baseline is exposed (not covered by a stacked segment).
        let isOuterEnd: Bool
    }

    let plot: UIRect
    /// Both value axes: a mark resolves its y through the one its series reads.
    let axes: ChartValueAxes
    /// Marks per visible series, in series order. Hidden series have none.
    let marks: [[Mark]]
    let hasBars: Bool
    let sampleCount: Int
    /// A numeric x domain hit-tests by true distance.
    let isScatter: Bool

    var domain: ClosedRange<Float> { axes.primaryScale.domain }

    init(series: [ChartSeries], axes: ChartValueAxes, xDomain: ChartXDomain, xScale: ChartScale?,
         barSpacing: Float, plot: UIRect) {
        self.plot = plot
        self.axes = axes
        let shown = axes.shown
        let slotCount = axes.sampleCount
        hasBars = axes.hasBars
        sampleCount = slotCount
        isScatter = xDomain.isNumeric
        let stacked = axes.isStacked
        let numericX = xDomain.isNumeric ? xScale : nil

        let barSeries = shown.filter { series[$0].mark == .bar }
        let groups = stacked ? 1 : max(1, barSeries.count)
        let slot = numericX.map { scale in Self.numericSlot(series: series, shown: shown, scale: scale, plot: plot) }
            ?? (plot.width / Float(max(1, sampleCount)))
        // Wide categories breathe (a quarter of the slot, at most 24pt); dense
        // ones keep the authored spacing. Bars inside a group sit 2pt apart.
        let categoryGap = max(barSpacing, min(slot * 0.25, 24))
        let innerGap: Float = groups > 1 ? 2 : 0
        let barWidth = max(1, (slot - categoryGap - Float(groups - 1) * innerGap) / Float(groups))

        var marks = [[Mark]](repeating: [], count: series.count)
        for index in shown {
            let item = series[index]
            let stack = axes.stack(item.axis)
            let scale = axes.scale(item.axis)
            let group = stacked ? 0 : (barSeries.firstIndex(of: index) ?? 0)
            // Only bars take a group offset; lines keep the slot center.
            let groupOffset = (Float(group) - Float(groups - 1) / 2) * (barWidth + innerGap)
            marks[index] = item.data.indices.map { i in
                let datum = item.data[i]
                let baseX: Float
                if let numericX {
                    baseX = numericX.x(datum.x, in: plot)
                } else if item.mark == .bar || stack.hasBars {
                    baseX = plot.minX + (Float(i) + 0.5) * slot
                } else {
                    baseX = Self.x(index: i, count: slotCount, hasBars: false, plot: plot)
                }
                let x = item.mark == .bar ? baseX + groupOffset : baseX
                guard datum.y.isFinite else {
                    return Mark(series: index, index: i, value: datum.y, x: x,
                                y: .nan, baseY: .nan, bar: nil, isOuterEnd: false)
                }
                let y = scale.y(stack.tops[index][i], in: plot)
                let baseY = scale.y(stack.bases[index][i], in: plot)
                var bar: UIRect?
                if item.mark == .bar {
                    bar = UIRect(x: x - barWidth / 2, y: min(y, baseY),
                                 width: barWidth, height: max(1, abs(baseY - y)))
                }
                return Mark(series: index, index: i, value: datum.y, x: x,
                            y: y, baseY: baseY, bar: bar,
                            isOuterEnd: stack.isOuterEnd(series: index, at: i, value: datum.y))
            }
        }
        self.marks = marks
    }

    /// Bars on a numeric axis are as wide as the closest pair of x values.
    private static func numericSlot(series: [ChartSeries], shown: [Int], scale: ChartScale,
                                    plot: UIRect) -> Float {
        let xs = shown.flatMap { series[$0].data.map(\.x) }.filter(\.isFinite).sorted()
        var gap = Float.greatestFiniteMagnitude
        if xs.count > 1 {
            for i in 1..<xs.count {
                let delta = scale.x(xs[i], in: plot) - scale.x(xs[i - 1], in: plot)
                if delta > 0.001 { gap = min(gap, delta) }
            }
        }
        return gap.isFinite ? gap : plot.width / Float(max(1, xs.count))
    }

    static func x(index: Int, count: Int, hasBars: Bool, plot: UIRect) -> Float {
        if hasBars { return plot.minX + (Float(index) + 0.5) * plot.width / Float(max(1, count)) }
        guard count > 1 else { return plot.minX + plot.width / 2 }
        return plot.minX + Float(index) / Float(count - 1) * plot.width
    }

    func y(_ value: Float, axis: ChartValueAxis = .left) -> Float {
        axes.scale(axis).y(value, in: plot)
    }

    /// The x a sample index sits at, from the first visible series.
    func columnX(_ index: Int) -> Float? {
        for marks in marks where marks.indices.contains(index) {
            let x = marks[index].x
            if x.isFinite { return x }
        }
        return nil
    }

    /// The mark nearest a plot-space point: the closest x column, then the
    /// series whose value is closest vertically (a numeric axis uses true
    /// distance). Gaps take no part.
    func nearest(x: Float, y: Float) -> Mark? {
        let all = marks.flatMap { $0 }.filter { $0.y.isFinite }
        guard !all.isEmpty else { return nil }
        if isScatter {
            return all.min { hypotf($0.x - x, $0.y - y) < hypotf($1.x - x, $1.y - y) }
        }
        let column = all.min { abs($0.x - x) < abs($1.x - x) }!.x
        let candidates = all.filter { abs($0.x - column) < 0.5 }
        return candidates.min { abs($0.y - y) < abs($1.y - y) }
    }

    func mark(_ point: ChartPoint) -> Mark? {
        guard marks.indices.contains(point.series), marks[point.series].indices.contains(point.index) else { return nil }
        return marks[point.series][point.index]
    }
}

extension ChartGeometry {
    /// Reduces a run to what the plot can actually resolve.
    ///
    /// Each horizontal pixel keeps its lowest and highest point, in x order, so
    /// the drawn envelope is unchanged while the stroke costs one segment per
    /// column instead of one per sample. Picking extremes rather than every
    /// n-th sample is what keeps a spike from vanishing between two samples. A
    /// run that already fits the width comes back untouched.
    static func decimated(_ marks: [Mark], width: Float) -> [Mark] {
        guard marks.count > 2, width >= 1 else { return marks }
        let columns = max(1, Int(width.rounded(.up)))
        guard marks.count > columns * 2 else { return marks }
        // Rounding the bucket up keeps the bucket count at or below the column
        // count, so the result never exceeds two points per pixel.
        let bucket = max(2, Int((Float(marks.count) / Float(columns)).rounded(.up)))
        var result: [Mark] = []
        result.reserveCapacity(columns * 2 + 2)
        result.append(marks[0])
        var start = 1
        while start < marks.count - 1 {
            let end = min(start + bucket, marks.count - 1)
            var low = start, high = start
            for i in start..<end {
                if marks[i].y < marks[low].y { low = i }
                if marks[i].y > marks[high].y { high = i }
            }
            // Emitted in x order, which is the order the stroke needs.
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
