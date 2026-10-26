import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import CoveUIRuntime
@testable import CoveUICompose

/// A chart turned on its side. The value axis runs along the bottom and the
/// categories down the left; everything else is the same renderer, projected
/// through one swap.
@Suite("Chart orientation")
struct ChartOrientationTests {
    private static var unlabeled: ChartConfiguration.Axis {
        var axis = ChartConfiguration.Axis()
        axis.x.isLabeled = false
        axis.y.isLabeled = false
        return axis
    }

    private func flat(_ orientation: ChartOrientation,
                      stacking: ChartStacking = .none,
                      min: Float? = 0, max: Float? = 4) -> ChartStyle {
        ChartStyle(minValue: min, maxValue: max, showsGrid: false, contentInset: 0,
                   stacking: stacking, orientation: orientation)
    }

    private func geometry(_ series: [ChartSeries], style: ChartStyle,
                          x: ChartXDomain = .categorical([]),
                          plot: UIRect = UIRect(x: 0, y: 0, width: 200, height: 100)) -> ChartGeometry {
        let axes = ChartValueAxes.resolve(series: series, visible: Array(repeating: true, count: series.count),
                                          style: style, guides: ChartGuides(), axis: Self.unlabeled)
        let xScale = x.isNumeric
            ? ChartScale.fit(extent: axes.shown.flatMap { series[$0].data.map(\.x) },
                             lower: nil, upper: nil, tickCount: 5)
            : nil
        return ChartGeometry(series: series, axes: axes, xDomain: x, xScale: xScale,
                             barSpacing: style.barSpacing, orientation: style.orientation, plot: plot)
    }

    private func frame(_ series: [ChartSeries], x: ChartXDomain, style: ChartStyle,
                       axis: ChartConfiguration.Axis = ChartConfiguration.Axis(),
                       size: CGSize = CGSize(width: 300, height: 200)) -> ChartFrame {
        let axes = ChartValueAxes.resolve(series: series, visible: Array(repeating: true, count: series.count),
                                          style: style, guides: ChartGuides(), axis: axis)
        let xScale = x.isNumeric
            ? ChartScale.fit(extent: axes.shown.flatMap { series[$0].data.map(\.x) },
                             lower: nil, upper: nil, tickCount: 5)
            : nil
        return ChartFrame.cartesian(axes: axes, xDomain: x, xScale: xScale, style: style, axis: axis,
                                    size: size, origin: .zero,
                                    text: ChartText(theme: .defaultLight, environment: nil))
    }

    private func draw(_ content: ChartContent, style: ChartStyle, selection: ChartPoint? = nil,
                      size: CGSize = CGSize(width: 200, height: 100))
        -> (list: DrawList, resolution: ChartResolution) {
        GlobalTestLock.locked {
            let plot = ChartPlot(content: content, visible: Array(repeating: true, count: content.count),
                                 style: style, axis: Self.unlabeled, pie: .init(),
                                 format: { String(Int($0.rounded())) }, selection: selection,
                                 summary: ("Chart", ""),
                                 input: ChartPlotInput(isEnabled: false, select: { _ in }))
            let node = plot._makeNode()
            node.frame = CGRect(origin: .zero, size: size)
            plot._updateNode(node)
            let list = DrawList()
            node.draw?(list, .zero)
            return (list, plot.resolve(in: size, text: ChartText(theme: node.theme, environment: nil)))
        }
    }

    /// Whether a rule spans from `left` to `right` at `y`. Both ends have to be
    /// there, so a line crossing the row at an angle does not count.
    private func hasHorizontalRule(at y: Float, from left: Float, to right: Float, in list: DrawList) -> Bool {
        var sawLeft = false, sawRight = false
        for vertex in list.vertices {
            guard abs(vertex.posY - y) < 1.5 else { continue }
            if abs(vertex.posX - left) < 1.5 { sawLeft = true }
            if abs(vertex.posX - right) < 1.5 { sawRight = true }
        }
        return sawLeft && sawRight
    }

    /// The same, for a rule running down the plot.
    private func hasVerticalRule(at x: Float, from top: Float, to bottom: Float, in list: DrawList) -> Bool {
        var sawTop = false, sawBottom = false
        for vertex in list.vertices {
            guard abs(vertex.posX - x) < 1.5 else { continue }
            if abs(vertex.posY - top) < 1.5 { sawTop = true }
            if abs(vertex.posY - bottom) < 1.5 { sawBottom = true }
        }
        return sawTop && sawBottom
    }

    @Test("A horizontal bar grows right from the zero line, and is thick across the rows")
    func barsGrowRight() throws {
        let chart = geometry([.bar([2, 4], title: "Revenue")], style: flat(.horizontal))
        let first = try #require(chart.marks[0][0].bar)
        // 2 of 0…4 is half the plot, measured from the left edge.
        #expect(abs(first.minX) < 0.01)
        #expect(abs(first.maxX - 100) < 0.01)
        // The slot runs down the plot, so the bar is wide and short.
        #expect(first.width > first.height)
        #expect(abs(first.height - 37.5) < 0.01)
    }

    @Test("Categories stack down the left and values run along the bottom")
    func rowsAndValues() {
        let chart = geometry([.bar([2, 4], title: "Revenue")], style: flat(.horizontal))
        // Later categories sit lower; later values sit further right.
        #expect(chart.marks[0][0].y < chart.marks[0][1].y)
        #expect(chart.marks[0][0].x < chart.marks[0][1].x)
        // A column chart would have them the other way round.
        let vertical = geometry([.bar([2, 4], title: "Revenue")], style: flat(.vertical))
        #expect(vertical.marks[0][0].x < vertical.marks[0][1].x)
        // A bigger value sits higher, so its y is smaller.
        #expect(vertical.marks[0][0].y > vertical.marks[0][1].y)
    }

    @Test("A negative bar grows left of the zero line")
    func negativeBars() throws {
        let chart = geometry([.bar([-2, 2], title: "Change")],
                             style: flat(.horizontal, min: -2, max: 2))
        let loss = try #require(chart.marks[0][0].bar)
        let gain = try #require(chart.marks[0][1].bar)
        #expect(abs(loss.minX) < 0.01 && abs(loss.maxX - 100) < 0.01)
        #expect(abs(gain.minX - 100) < 0.01 && abs(gain.maxX - 200) < 0.01)
    }

    @Test("Stacked segments butt together along the value axis")
    func stackedSegments() throws {
        let chart = geometry([.bar([1, 1], title: "Online"), .bar([1, 1], color: .success, title: "Retail")],
                             style: flat(.horizontal, stacking: .stacked, min: 0, max: 2))
        let lower = try #require(chart.marks[0][0].bar)
        let upper = try #require(chart.marks[1][0].bar)
        #expect(abs(lower.maxX - upper.minX) < 0.01)
        #expect(abs(lower.minX) < 0.01 && abs(upper.maxX - 200) < 0.01)
        // Both segments share the row, so their thickness matches.
        #expect(abs(lower.minY - upper.minY) < 0.01)
    }

    @Test("A percent stack fills the plot across the value axis")
    func percentStack() {
        let chart = geometry([.bar([3, 1], title: "Online"), .bar([1, 3], color: .success, title: "Retail")],
                             style: flat(.horizontal, stacking: .percent, min: nil, max: nil))
        for index in 0..<2 {
            let segments = (0..<2).compactMap { chart.marks[$0][index].bar }
            #expect(abs((segments.map(\.minX).min() ?? 0)) < 0.01)
            #expect(abs((segments.map(\.maxX).max() ?? 0) - 200) < 0.01)
        }
    }

    @Test("The value axis keeps the bottom gutter and the categories take the left one")
    func axisEdges() {
        var axis = ChartConfiguration.Axis()
        axis.y.title = "Revenue"
        axis.x.title = "Country"
        let chart = frame([.bar([2, 4], title: "Revenue")], x: .categorical(["A", "B"]),
                          style: ChartStyle(orientation: .horizontal), axis: axis)
        #expect(chart.orientation == .horizontal)
        // The plot gives up room on the left for category labels and at the
        // bottom for value labels and the value title.
        #expect(chart.plot.minX > chart.bounds.minX)
        #expect(chart.plot.maxY < chart.bounds.maxY)
        // Values advance along x; categories advance down y.
        #expect(zip(chart.valueTicks, chart.valueTicks.dropFirst()).allSatisfy { $0.position < $1.position })
        #expect(chart.categoryTicks.count == 2)
        #expect(zip(chart.categoryTicks, chart.categoryTicks.dropFirst()).allSatisfy { $0.position < $1.position })
        #expect(abs(chart.categoryTicks[0].position - (chart.plot.minY + chart.plot.height / 4)) < 0.5)
        #expect(chart.valueTitle == "Revenue" && chart.categoryTitle == "Country")
    }

    @Test("A vertical chart keeps its own gutters when the same options are used")
    func axisEdgesStayVertical() {
        var axis = ChartConfiguration.Axis()
        axis.y.title = "Revenue"
        axis.x.title = "Country"
        let chart = frame([.bar([2, 4], title: "Revenue")], x: .categorical(["A", "B"]),
                          style: ChartStyle(orientation: .vertical), axis: axis)
        #expect(chart.plot.minX > chart.bounds.minX)
        #expect(chart.plot.maxY < chart.bounds.maxY)
        #expect(zip(chart.valueTicks, chart.valueTicks.dropFirst()).allSatisfy { $0.position > $1.position })
        #expect(zip(chart.categoryTicks, chart.categoryTicks.dropFirst()).allSatisfy { $0.position < $1.position })
    }

    @Test("Grid rules cross the value axis, so a horizontal chart draws them upright")
    func gridRunsUpright() {
        let content = ChartContent.cartesian(series: [.bar([2, 4], title: "Revenue")],
                                             guides: ChartGuides(), x: .categorical(["A", "B"]))
        let style = ChartStyle(minValue: 0, maxValue: 4, showsGrid: true, contentInset: 0,
                               orientation: .horizontal)
        let (list, resolution) = draw(content, style: style)
        let plot = resolution.frame.plot
        // Ticks land on 0, 1, 2, 3, 4 across 200pt: one of them is at 100.
        #expect(hasVerticalRule(at: 100, from: plot.minY, to: plot.maxY, in: list))
        // The axis line still closes the bottom edge.
        #expect(hasHorizontalRule(at: plot.maxY, from: plot.minX, to: plot.maxX, in: list))
    }

    @Test("Selection rules across the selected row")
    func selectionRule() throws {
        let content = ChartContent.cartesian(series: [.bar([2, 4], title: "Revenue")],
                                             guides: ChartGuides(), x: .categorical(["A", "B"]))
        let style = ChartStyle(minValue: 0, maxValue: 4, showsGrid: false, contentInset: 0,
                               orientation: .horizontal)
        let (list, resolution) = draw(content, style: style, selection: ChartPoint(series: 0, index: 1))
        let chart = try #require(resolution.cartesian)
        #expect(hasHorizontalRule(at: chart.marks[0][1].y, from: chart.plot.minX, to: chart.plot.maxX,
                                  in: list))
    }

    @Test("Hit-testing picks the row the pointer is on, not the nearest column")
    func hitTestPicksTheRow() {
        let chart = geometry([.bar([2, 4], title: "Revenue")], style: flat(.horizontal))
        #expect(chart.nearest(x: 120, y: 20)?.index == 0)
        #expect(chart.nearest(x: 120, y: 80)?.index == 1)
    }

    @Test("A plot resolves its geometry in the style's orientation")
    func plotFollowsStyle() {
        let plot = ChartPlot(content: .cartesian(series: [.bar([1, 2, 3], title: "Revenue")],
                                                 guides: ChartGuides(), x: .categorical(["A", "B", "C"])),
                             visible: [true], style: ChartStyle(orientation: .horizontal),
                             axis: Self.unlabeled, pie: .init(),
                             format: { String(Int($0.rounded())) }, selection: nil,
                             summary: ("Chart", ""),
                             input: ChartPlotInput(isEnabled: false, select: { _ in }))
        let resolution = plot.resolve(in: CGSize(width: 200, height: 100),
                                      text: ChartText(theme: .defaultLight, environment: nil))
        #expect(resolution.frame.orientation == .horizontal)
        #expect(resolution.cartesian?.orientation == .horizontal)
    }

    @Test("An area fills to the baseline along the value axis, whichever way it runs")
    func areaFillsAcross() throws {
        let content = ChartContent.cartesian(series: [.area([2, 4], title: "Revenue")],
                                             guides: ChartGuides(), x: .categorical(["A", "B"]))
        let (list, resolution) = draw(content, style: ChartStyle(minValue: 0, maxValue: 4, showsGrid: false,
                                                                contentInset: 0, orientation: .horizontal))
        let chart = try #require(resolution.cartesian)
        // Between the two values, at the lower row, the fill reaches back to
        // the zero line on the left rather than down to a bottom edge.
        let mid = (chart.marks[0][0].y + chart.marks[0][1].y) / 2
        var covered = false
        for start in stride(from: 0, to: list.indices.count, by: 3) {
            let a = list.vertices[Int(list.indices[start])]
            let b = list.vertices[Int(list.indices[start + 1])]
            let c = list.vertices[Int(list.indices[start + 2])]
            let ys = [a.posY, b.posY, c.posY], xs = [a.posX, b.posX, c.posX]
            if ys.min()! <= mid, ys.max()! >= mid, xs.min()! <= 20, xs.max()! >= 100 { covered = true; break }
        }
        #expect(covered)
    }
}
