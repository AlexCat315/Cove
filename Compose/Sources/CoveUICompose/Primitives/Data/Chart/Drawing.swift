internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Draws one resolved chart into a draw list. Pure: everything it needs is
/// passed in, so repaint identity stays explicit.
struct ChartPaint {
    let theme: Theme
    let style: ChartStyle
    let selection: ChartPoint?
    let frame: ChartFrame
    let text: ChartText

    func background(_ bounds: UIRect, list: DrawList) {
        if let background = style.background { list.addRect(bounds, color: background.resolve(theme)) }
    }

    func brush(_ range: ClosedRange<Float>, window: ChartWindow, list: DrawList) {
        guard range.lowerBound.isFinite, range.upperBound.isFinite else { return }
        let a = window.position(range.lowerBound, plot: frame.plot, orientation: frame.orientation)
        let b = window.position(range.upperBound, plot: frame.plot, orientation: frame.orientation)
        guard a.isFinite, b.isFinite else { return }
        let rect = frame.orientation == .vertical
            ? UIRect(x: min(a, b), y: frame.plot.minY, width: abs(b - a), height: frame.plot.height)
            : UIRect(x: frame.plot.minX, y: min(a, b), width: frame.plot.width, height: abs(b - a))
        list.addRect(rect, color: theme.colors.accent.multipliedAlpha(0.14))
        list.addRoundedRectStroke(rect, radius: 0, width: 1, color: theme.colors.accent)
    }

    /// One rule per value-axis tick, so the grid reads the same numbers as the
    /// labels instead of dividing the domain into arbitrary fractions.
    func grid(list: DrawList) {
        guard style.showsGrid, !frame.valueTicks.isEmpty else { return }
        let plot = frame.plot
        let color = style.gridColor.resolve(theme).multipliedAlpha(0.55)
        for tick in frame.valueTicks {
            switch frame.orientation {
            case .vertical:
                list.addLine(fromX: plot.minX, fromY: tick.position,
                             toX: plot.maxX, toY: tick.position, thickness: 1, color: color)
            case .horizontal:
                list.addLine(fromX: tick.position, fromY: plot.minY,
                             toX: tick.position, toY: plot.maxY, thickness: 1, color: color)
            }
        }
        // The baseline closes the grid into an axis. The bottom edge is the
        // independent axis on a vertical chart and the value axis on a
        // horizontal one, so the line lands on the axis either way.
        list.addLine(fromX: plot.minX, fromY: plot.maxY,
                     toX: plot.maxX, toY: plot.maxY, thickness: 1,
                     color: style.gridColor.resolve(theme).multipliedAlpha(0.9))
    }

    /// Tick labels in the gutters, plus the axis titles.
    ///
    /// A value tick sits in the left gutter on a vertical chart and under the
    /// plot on a horizontal one; the category ticks swap. Text itself is never
    /// rotated, which is the point of turning the chart instead.
    func axisLabels(list: DrawList) {
        guard let environment = text.environment else { return }
        let color = theme.colors.onSurfaceMuted
        let bounds = frame.bounds, plot = frame.plot
        let vertical = frame.orientation == .vertical
        func draw(_ layout: TextLayoutResult, _ x: Float, _ y: Float) {
            list.addText(layout, origin: (x, y), color: color,
                         textureID: environment.atlasTextureID, atlas: environment.atlas)
        }
        for tick in frame.valueTicks where !tick.label.isEmpty {
            guard let layout = text.layout(tick.label) else { continue }
            if vertical {
                // Right-aligned against the plot, nudged inside the box at the edges.
                draw(layout, max(bounds.minX, plot.minX - ChartFrame.labelGap - layout.totalWidth),
                     min(max(bounds.minY, tick.position - layout.totalHeight / 2),
                         bounds.maxY - layout.totalHeight))
            } else {
                draw(layout, min(max(bounds.minX, tick.position - layout.totalWidth / 2),
                                 bounds.maxX - layout.totalWidth),
                     plot.maxY + 4)
            }
        }
        for tick in frame.categoryTicks where !tick.label.isEmpty {
            guard let layout = text.layout(tick.label) else { continue }
            if vertical {
                draw(layout, min(max(bounds.minX, tick.position - layout.totalWidth / 2),
                                 bounds.maxX - layout.totalWidth),
                     plot.maxY + 4)
            } else {
                draw(layout, max(bounds.minX, plot.minX - ChartFrame.labelGap - layout.totalWidth),
                     min(max(bounds.minY, tick.position - layout.totalHeight / 2),
                         bounds.maxY - layout.totalHeight))
            }
        }
        // The second value axis grows away from the plot, into its own gutter:
        // right of the plot when vertical, above it when horizontal.
        for tick in frame.secondaryTicks where !tick.label.isEmpty {
            guard let layout = text.layout(tick.label) else { continue }
            if vertical {
                draw(layout, min(plot.maxX + ChartFrame.labelGap, bounds.maxX - layout.totalWidth),
                     min(max(bounds.minY, tick.position - layout.totalHeight / 2),
                         bounds.maxY - layout.totalHeight))
            } else {
                draw(layout, min(max(bounds.minX, tick.position - layout.totalWidth / 2),
                                 bounds.maxX - layout.totalWidth),
                     bounds.minY)
            }
        }
        if !frame.valueTitle.isEmpty, let layout = text.layout(frame.valueTitle) {
            if vertical {
                draw(layout, bounds.minX, bounds.minY)
            } else {
                draw(layout, plot.minX + (plot.width - layout.totalWidth) / 2, bounds.maxY - layout.totalHeight)
            }
        }
        if !frame.secondaryTitle.isEmpty, let layout = text.layout(frame.secondaryTitle) {
            draw(layout, bounds.maxX - layout.totalWidth, bounds.minY)
        }
        if !frame.categoryTitle.isEmpty, let layout = text.layout(frame.categoryTitle) {
            if vertical {
                draw(layout, plot.minX + (plot.width - layout.totalWidth) / 2, bounds.maxY - layout.totalHeight)
            } else {
                draw(layout, bounds.minX, bounds.minY)
            }
        }
    }

    func thresholds(_ thresholds: [ChartThreshold], geometry: ChartGeometry, list: DrawList) {
        let plot = geometry.plot
        for threshold in thresholds where threshold.value.isFinite {
            let at = geometry.along(threshold.value, axis: threshold.axis)
            let color = threshold.color.resolve(theme).multipliedAlpha(0.8)
            switch geometry.orientation {
            case .vertical:
                list.addLine(fromX: plot.minX, fromY: at, toX: plot.maxX, toY: at,
                             thickness: 1, color: color)
            case .horizontal:
                list.addLine(fromX: at, fromY: plot.minY, toX: at, toY: plot.maxY,
                             thickness: 1, color: color)
            }
        }
    }

    /// A shaded corridor across the plot, under the grid so the grid stays the
    /// top rule. A band that lies entirely outside the domain draws nothing.
    func bands(_ bands: [ChartBand], geometry: ChartGeometry, list: DrawList) {
        let plot = geometry.plot
        for band in bands where band.from.isFinite && band.to.isFinite {
            let a = geometry.along(band.to, axis: band.axis)
            let b = geometry.along(band.from, axis: band.axis)
            let low = min(a, b), high = max(a, b)
            let rect = geometry.orientation == .vertical
                ? UIRect(x: plot.minX, y: low, width: plot.width, height: high - low)
                : UIRect(x: low, y: plot.minY, width: high - low, height: plot.height)
            guard rect.width > 0.5, rect.height > 0.5 else { continue }
            list.addRect(rect, color: band.color.resolve(theme).multipliedAlpha(0.12))
        }
    }

    /// Leader lines from the ring to each slice's label.
    func pieLeaders(_ slices: [ChartSlice], geometry: PieGeometry, list: DrawList) {
        guard geometry.radius > 2 else { return }
        let color = theme.colors.onSurfaceMuted.multipliedAlpha(0.4)
        for wedge in geometry.wedges where !slices[wedge.index].title.isEmpty {
            let from = geometry.ringPoint(of: wedge), to = geometry.labelAnchor(of: wedge)
            list.addLine(fromX: from.x, fromY: from.y, toX: to.x, toY: to.y, thickness: 1, color: color)
        }
    }

    func series(_ series: [ChartSeries], geometry: ChartGeometry, list: DrawList) {
        // Fills first, so lines and points of every series stay on top.
        for (index, item) in series.enumerated() where item.mark == .area {
            area(geometry.marks[index], color: item.color.resolve(theme), seriesStyle: item.style,
                 geometry: geometry, list: list)
        }
        for (index, item) in series.enumerated() where item.mark == .bar {
            bars(geometry.marks[index], color: item.color.resolve(theme), list: list)
        }
        for (index, item) in series.enumerated() where item.mark == .line || item.mark == .area {
            line(geometry.marks[index], color: item.color.resolve(theme), seriesStyle: item.style,
                 geometry: geometry, list: list)
        }
        for (index, item) in series.enumerated() where item.mark == .point {
            // Two points in one pixel are one dot; drawing both only costs fill.
            var drawn = Set<UInt64>()
            for mark in geometry.marks[index] where mark.y.isFinite {
                guard drawn.insert(Self.pixel(mark.x, mark.y)).inserted else { continue }
                dot(x: mark.x, y: mark.y, radius: item.style.pointRadius,
                    color: item.color.resolve(theme).multipliedAlpha(0.85), list: list)
            }
        }
    }

    func markers(_ markers: [ChartMarker], geometry: ChartGeometry, list: DrawList) {
        guard geometry.sampleCount > 0 else { return }
        let plot = geometry.plot
        for marker in markers {
            let index = max(0, min(marker.index, geometry.sampleCount - 1))
            guard let at = geometry.across(index: index) else { continue }
            switch geometry.orientation {
            case .vertical:
                list.addLine(fromX: at, fromY: plot.minY, toX: at, toY: plot.maxY,
                             thickness: marker.width, color: marker.color.resolve(theme))
            case .horizontal:
                list.addLine(fromX: plot.minX, fromY: at, toX: plot.maxX, toY: at,
                             thickness: marker.width, color: marker.color.resolve(theme))
            }
        }
    }

    /// A rule through the selected column and a ringed dot on the value.
    func selection(_ series: [ChartSeries], geometry: ChartGeometry, list: DrawList) {
        guard let selection, let mark = geometry.mark(selection) else { return }
        let plot = geometry.plot
        let rule = theme.colors.onSurfaceMuted.multipliedAlpha(0.35)
        let at = geometry.across(mark)
        switch geometry.orientation {
        case .vertical:
            list.addLine(fromX: at, fromY: plot.minY, toX: at, toY: plot.maxY, thickness: 1, color: rule)
        case .horizontal:
            list.addLine(fromX: plot.minX, fromY: at, toX: plot.maxX, toY: at, thickness: 1, color: rule)
        }
        guard mark.bar == nil, mark.y.isFinite else { return }
        dot(x: mark.x, y: mark.y, radius: 6, color: theme.colors.surface, list: list)
        dot(x: mark.x, y: mark.y, radius: 4, color: series[mark.series].color.resolve(theme), list: list)
    }

    func pie(_ slices: [ChartSlice], geometry: PieGeometry, list: DrawList) {
        guard geometry.radius > 2 else { return }
        // A surface-colored gap separates slices without an outline.
        let gap: Float = geometry.wedges.count > 1 ? 1.5 : 0
        for wedge in geometry.wedges {
            let selected = selection?.index == wedge.index
            let dimmed = selection != nil && !selected
            var color = slices[wedge.index].resolvedColor(at: wedge.index).resolve(theme)
            if dimmed { color = color.multipliedAlpha(0.55) }
            ring(geometry, wedge: wedge, outer: geometry.radius + (selected ? 4 : 0), gap: gap, color: color, list: list)
        }
    }

    // MARK: Primitives

    private func line(_ marks: [ChartGeometry.Mark], color: Color, seriesStyle: ChartSeriesStyle,
                      geometry: ChartGeometry, list: DrawList) {
        let width = seriesStyle.lineWidth ?? style.lineWidth
        let length = geometry.orientation.breadth(geometry.plot)
        for run in runs(marks) {
            let points = sampled(ChartGeometry.decimated(geometry.visibleRun(run), orientation: geometry.orientation, length: length),
                                 curved: seriesStyle.curve, geometry: geometry)
            stroke(points, color: color, width: width, dash: seriesStyle.dash, list: list)
        }
    }

    /// One point of a filled run: where the value is, and where the fill meets
    /// the baseline. Keeping both as points makes the fill independent of which
    /// way the chart is turned.
    private struct FillPoint {
        let value: (x: Float, y: Float)
        let base: (x: Float, y: Float)
    }

    private func area(_ marks: [ChartGeometry.Mark], color: Color, seriesStyle: ChartSeriesStyle,
                      geometry: ChartGeometry, list: DrawList) {
        let fill = color.multipliedAlpha(seriesStyle.fillOpacity)
        let length = geometry.orientation.breadth(geometry.plot)
        for run in runs(marks) {
            let points = sampledArea(ChartGeometry.decimated(geometry.visibleRun(run), orientation: geometry.orientation, length: length),
                                     curved: seriesStyle.curve, geometry: geometry)
            for (a, b) in zip(points, points.dropFirst()) {
                list.addTriangle(a.value.x, a.value.y, b.value.x, b.value.y, b.base.x, b.base.y, color: fill)
                list.addTriangle(a.value.x, a.value.y, b.base.x, b.base.y, a.base.x, a.base.y, color: fill)
            }
        }
    }

    /// Contiguous stretches of measured values. A gap ends a run, so lines and
    /// fills break there instead of bridging a value that was never measured.
    private func runs(_ marks: [ChartGeometry.Mark]) -> [[ChartGeometry.Mark]] {
        var runs: [[ChartGeometry.Mark]] = []
        var current: [ChartGeometry.Mark] = []
        for mark in marks {
            if mark.y.isFinite {
                current.append(mark)
            } else if !current.isEmpty {
                runs.append(current)
                current = []
            }
        }
        if !current.isEmpty { runs.append(current) }
        return runs
    }

    /// Emits a polyline, split into on/off runs when a dash pattern is set.
    ///
    /// `addLine` rounds both ends of every segment, so consecutive segments
    /// already meet in a round join — no extra join geometry is needed, and on a
    /// decimated run that would be thousands of redundant discs.
    private func stroke(_ points: [(x: Float, y: Float)], color: Color, width: Float,
                        dash: [Float], list: DrawList) {
        guard points.count > 1 else {
            if let point = points.first { dot(x: point.x, y: point.y, radius: width, color: color, list: list) }
            return
        }
        let pattern = dash.filter { $0 > 0 }
        guard pattern.count >= 2, pattern.reduce(0, +) > 0.5 else {
            for (a, b) in zip(points, points.dropFirst()) {
                list.addLine(fromX: a.x, fromY: a.y, toX: b.x, toY: b.y, thickness: width, color: color)
            }
            return
        }
        var index = 0
        var remaining = pattern[0]
        var isOn = true
        for (a, b) in zip(points, points.dropFirst()) {
            let dx = b.x - a.x, dy = b.y - a.y
            let length = hypotf(dx, dy)
            guard length > 0.0001 else { continue }
            var travelled: Float = 0
            while travelled < length - 0.0001 {
                let step = min(remaining, length - travelled)
                if isOn, step > 0 {
                    let t0 = travelled / length, t1 = (travelled + step) / length
                    list.addLine(fromX: a.x + dx * t0, fromY: a.y + dy * t0,
                                 toX: a.x + dx * t1, toY: a.y + dy * t1,
                                 thickness: width, color: color)
                }
                travelled += step
                remaining -= step
                if remaining <= 0.0001 {
                    index = (index + 1) % pattern.count
                    remaining = pattern[index]
                    isOn.toggle()
                }
            }
        }
    }

    /// Straight segments, or a monotone cubic through them.
    ///
    /// The curve is built where the independent axis always increases — across
    /// the categories, with the value on the other axis — and then projected
    /// back, so a horizontal chart curves exactly like a vertical one. A run
    /// whose categories do not increase stays straight rather than looping back
    /// on itself.
    private func sampled(_ marks: [ChartGeometry.Mark], curved: Bool,
                         geometry: ChartGeometry) -> [(x: Float, y: Float)] {
        let orientation = geometry.orientation
        let logical = marks.map { mark in orientation.axes(of: (mark.x, mark.y)) }
            .map { (x: $0.across, y: $0.along) }
        guard curved else { return logical.map { orientation.point(across: $0.x, along: $0.y) } }
        return curve(logical).map { orientation.point(across: $0.x, along: $0.y) }
    }

    private func curve(_ points: [(x: Float, y: Float)]) -> [(x: Float, y: Float)] {
        guard points.count > 2, isIncreasing(points) else { return points }
        let tangents = tangents(for: points)
        var result: [(x: Float, y: Float)] = [points[0]]
        for i in 0..<points.count - 1 {
            let x0 = points[i].x, x1 = points[i + 1].x, h = x1 - x0
            let y0 = points[i].y, y1 = points[i + 1].y
            for step in 1...Self.curveSteps {
                let t = Float(step) / Float(Self.curveSteps), t2 = t * t, t3 = t2 * t
                let y = (2 * t3 - 3 * t2 + 1) * y0 + (t3 - 2 * t2 + t) * h * tangents[i]
                    + (-2 * t3 + 3 * t2) * y1 + (t3 - t2) * h * tangents[i + 1]
                result.append((x: x0 + h * t, y: y))
            }
        }
        return result
    }

    /// The same curve, carrying an interpolated baseline for the fill.
    private func sampledArea(_ marks: [ChartGeometry.Mark], curved: Bool,
                             geometry: ChartGeometry) -> [FillPoint] {
        let points = sampled(marks, curved: curved, geometry: geometry)
        guard points.count > marks.count else {
            return zip(marks, points).map { FillPoint(value: $1, base: geometry.basePoint($0)) }
        }
        // A curve added samples between marks; the baseline only moves linearly.
        let orientation = geometry.orientation
        var result: [FillPoint] = []
        var segment = 0
        for point in points {
            let across = orientation.axes(of: point).across
            while segment < marks.count - 2, across > geometry.across(marks[segment + 1]) { segment += 1 }
            let a = marks[segment], b = marks[min(segment + 1, marks.count - 1)]
            let span = geometry.across(b) - geometry.across(a)
            let t = span > 0.0001 ? max(0, min((across - geometry.across(a)) / span, 1)) : 0
            result.append(FillPoint(value: point,
                                    base: orientation.point(across: across,
                                                            along: a.base + (b.base - a.base) * t)))
        }
        return result
    }

    private static let curveSteps = 8

    private func isIncreasing(_ points: [(x: Float, y: Float)]) -> Bool {
        for i in 1..<points.count where points[i].x <= points[i - 1].x { return false }
        return true
    }

    /// Fritsch–Carlson tangents: monotone, so the curve never overshoots the
    /// values it connects.
    private func tangents(for points: [(x: Float, y: Float)]) -> [Float] {
        let count = points.count
        var slopes = [Float](repeating: 0, count: count - 1)
        for i in 0..<count - 1 {
            slopes[i] = (points[i + 1].y - points[i].y) / (points[i + 1].x - points[i].x)
        }
        var tangents = [Float](repeating: 0, count: count)
        tangents[0] = slopes[0]
        tangents[count - 1] = slopes[count - 2]
        for i in 1..<count - 1 {
            tangents[i] = slopes[i - 1] * slopes[i] <= 0 ? 0 : (slopes[i - 1] + slopes[i]) / 2
        }
        for i in 0..<count - 1 {
            guard slopes[i] != 0 else { tangents[i] = 0; tangents[i + 1] = 0; continue }
            let a = tangents[i] / slopes[i], b = tangents[i + 1] / slopes[i]
            let magnitude = a * a + b * b
            if magnitude > 9 {
                let scale = 3 / sqrtf(magnitude)
                tangents[i] = scale * a * slopes[i]
                tangents[i + 1] = scale * b * slopes[i]
            }
        }
        return tangents
    }

    private func bars(_ marks: [ChartGeometry.Mark], color: Color, list: DrawList) {
        for mark in marks {
            guard let rect = mark.bar else { continue }
            let dimmed = selection != nil && selection != ChartPoint(series: mark.series, index: mark.index)
            let fill = dimmed ? color.multipliedAlpha(0.45) : color
            // Round the end away from the baseline only, and only where no
            // stacked segment sits on top of it.
            guard mark.isOuterEnd else { list.addRect(rect, color: fill); continue }
            let radius = min(3, rect.width / 2, rect.height / 2)
            list.addRoundedRect(rect, radius: radius, color: fill)
            if let squared = squaredEnd(rect, base: mark.base) { list.addRect(squared, color: fill) }
        }
    }

    /// The strip of a bar that stays square, so a stacked segment butts against
    /// the one below it instead of leaving a rounded notch.
    ///
    /// Which end that is follows the baseline: the bar grows away from it, so
    /// the square half is the one nearer it — on whichever axis the values run.
    private func squaredEnd(_ rect: UIRect, base: Float) -> UIRect? {
        guard base.isFinite else { return nil }
        let radius = min(3, rect.width / 2, rect.height / 2)
        guard radius > 0 else { return nil }
        switch frame.orientation {
        case .vertical:
            let towardBase = base >= rect.minY + rect.height / 2
            return UIRect(x: rect.minX, y: towardBase ? rect.maxY - radius : rect.minY,
                          width: rect.width, height: radius)
        case .horizontal:
            let towardBase = base <= rect.minX + rect.width / 2
            return UIRect(x: towardBase ? rect.minX : rect.maxX - radius, y: rect.minY,
                          width: radius, height: rect.height)
        }
    }

    private func dot(x: Float, y: Float, radius: Float, color: Color, list: DrawList) {
        list.addRoundedRect(UIRect(x: x - radius, y: y - radius, width: radius * 2, height: radius * 2), radius: radius, color: color)
    }

    /// The pixel a plot point lands in, packed so two dots that would cover the
    /// same pixel compare equal.
    private static func pixel(_ x: Float, _ y: Float) -> UInt64 {
        func packed(_ value: Float) -> UInt64 {
            guard value.isFinite else { return 0 }
            let clamped = max(-1_000_000, min(value, 1_000_000)).rounded()
            return UInt64(UInt32(bitPattern: Int32(clamped)))
        }
        return packed(x) << 32 | packed(y)
    }

    private func ring(_ geometry: PieGeometry, wedge: PieGeometry.Wedge, outer: Float, gap: Float, color: Color, list: DrawList) {
        let inner = geometry.innerRadius
        let steps = max(2, Int((wedge.end - wedge.start) / (2 * .pi) * 96))
        // Trim the gap as an arc length at each radius so it stays parallel.
        func point(_ angle: Float, _ radius: Float) -> (Float, Float) {
            (geometry.center.x + cosf(angle) * radius, geometry.center.y + sinf(angle) * radius)
        }
        for step in 0..<steps {
            let t0 = Float(step) / Float(steps), t1 = Float(step + 1) / Float(steps)
            let a0 = wedge.start + (wedge.end - wedge.start) * t0, a1 = wedge.start + (wedge.end - wedge.start) * t1
            let outerTrim = gap / 2 / max(outer, 1), innerTrim = inner > 0 ? gap / 2 / inner : 0
            let o0 = point(max(a0, wedge.start + outerTrim), outer), o1 = point(min(a1, wedge.end - outerTrim), outer)
            if inner <= 0.5 {
                list.addTriangle(geometry.center.x, geometry.center.y, o0.0, o0.1, o1.0, o1.1, color: color)
            } else {
                let i0 = point(max(a0, wedge.start + innerTrim), inner), i1 = point(min(a1, wedge.end - innerTrim), inner)
                list.addTriangle(o0.0, o0.1, o1.0, o1.1, i1.0, i1.1, color: color)
                list.addTriangle(o0.0, o0.1, i1.0, i1.1, i0.0, i0.1, color: color)
            }
        }
    }
}
