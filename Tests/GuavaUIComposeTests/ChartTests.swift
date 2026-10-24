import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Chart drawing and geometry")
struct ChartTests {
    /// Every chart here is categorical; the numeric domain has its own suite.
    private func cartesian(series: [ChartSeries], thresholds: [ChartThreshold] = [],
                           bands: [ChartBand] = [], annotations: [ChartAnnotation] = [],
                           markers: [ChartMarker] = []) -> ChartContent {
        .cartesian(series: series,
                   guides: ChartGuides(thresholds: thresholds, bands: bands,
                                       annotations: annotations, markers: markers),
                   x: .categorical([]))
    }

    /// Test the rendered stroke rather than a particular tessellation budget.
    private func covers(_ point: CGPoint, in list: DrawList, color: Color? = nil) -> Bool {
        for start in stride(from: 0, to: list.indices.count, by: 3) {
            let vertices = (0..<3).map { list.vertices[Int(list.indices[start + $0])] }
            if let color, vertices[0].color & 0x00FF_FFFF != color.rgba8 & 0x00FF_FFFF { continue }
            let a = vertices[0], b = vertices[1], c = vertices[2]
            let x = Float(point.x), y = Float(point.y)
            let denominator = (b.posY - c.posY) * (a.posX - c.posX) + (c.posX - b.posX) * (a.posY - c.posY)
            guard abs(denominator) > 0.0001 else { continue }
            let u = ((b.posY - c.posY) * (x - c.posX) + (c.posX - b.posX) * (y - c.posY)) / denominator
            let v = ((c.posY - a.posY) * (x - c.posX) + (a.posX - c.posX) * (y - c.posY)) / denominator
            let w = 1 - u - v
            let alpha = u * Float(a.color >> 24) + v * Float(b.color >> 24) + w * Float(c.color >> 24)
            if min(u, min(v, w)) >= -0.001 && alpha > 20 { return true }
        }
        return false
    }

    /// The pixel assertions below assume the whole node is plot, so the axes
    /// stay unlabelled here; `ChartAxisTests` covers the labelled gutters.
    private static var unlabeled: ChartConfiguration.Axis {
        var axis = ChartConfiguration.Axis()
        axis.x.isLabeled = false
        axis.y.isLabeled = false
        return axis
    }

    private func draw(_ content: ChartContent, style: ChartStyle, selection: ChartPoint? = nil,
                      axis: ChartConfiguration.Axis? = nil, pie: ChartConfiguration.Pie = .init(),
                      size: CGSize = CGSize(width: 120, height: 40)) -> (DrawList, Node) {
        GlobalTestLock.locked {
            let plot = ChartPlot(content: content, visible: Array(repeating: true, count: content.count), style: style,
                                 axis: axis ?? Self.unlabeled, pie: pie,
                                 format: { String(Int($0.rounded())) }, selection: selection,
                                 summary: ("Chart", ""),
                                 input: ChartPlotInput(isEnabled: false, select: { _ in }))
            let node = plot._makeNode()
            node.frame = CGRect(origin: .zero, size: size)
            plot._updateNode(node)
            let list = DrawList()
            node.draw?(list, .zero)
            return (list, node)
        }
    }

    private func geometry(_ series: [ChartSeries], visible: [Bool]? = nil,
                          thresholds: [ChartThreshold] = [],
                          style: ChartStyle = ChartStyle(minValue: 0, barSpacing: 0),
                          x: ChartXDomain = .categorical([]),
                          plot: UIRect) -> ChartGeometry {
        let visible = visible ?? Array(repeating: true, count: series.count)
        let axes = ChartValueAxes.resolve(series: series, visible: visible, style: style,
                                          guides: ChartGuides(thresholds: thresholds),
                                          axis: ChartConfiguration.Axis())
        let xScale = x.isNumeric
            ? ChartScale.fit(extent: axes.shown.flatMap { series[$0].data.map(\.x) },
                             lower: nil, upper: nil, tickCount: 5)
            : nil
        return ChartGeometry(series: series, axes: axes, xDomain: x, xScale: xScale,
                             barSpacing: style.barSpacing, plot: plot)
    }

    private func flat(_ style: ChartStyle = ChartStyle(showsGrid: false, contentInset: 0)) -> ChartStyle { style }

    /// The labels the plot keeps at a fixed size, with their placement.
    private func overlay(_ content: ChartContent, style: ChartStyle,
                         size: CGSize = CGSize(width: 200, height: 100),
                         pie: ChartConfiguration.Pie = .init()) -> [ChartOverlaySlot] {
        GlobalTestLock.locked {
            let plot = ChartPlot(content: content, visible: Array(repeating: true, count: content.count), style: style,
                                 axis: Self.unlabeled, pie: pie,
                                 format: { String(Int($0.rounded())) }, selection: nil,
                                 summary: ("Chart", ""),
                                 input: ChartPlotInput(isEnabled: false, select: { _ in }))
            let node = plot._makeNode()
            node.frame = CGRect(origin: .zero, size: size)
            plot._updateNode(node)
            // Dropped labels keep their slot but lose their placement.
            return plot.overlaySlots(in: size, text: ChartText(theme: node.theme, environment: nil))
                .filter { $0.placement != nil }
        }
    }

    @Test("A chart without a selection binding is a passive image with a 64pt default height")
    func passive() {
        GlobalTestLock.locked {
            let plot = ChartPlot(content: cartesian(series: [.line([1, 2, 3])], thresholds: [], markers: []), visible: [true],
                                 style: ChartStyle(), axis: Self.unlabeled, pie: .init(),
                                 format: { String(Int($0.rounded())) }, selection: nil,
                                 summary: ("Sales", "1 to 3"),
                                 input: ChartPlotInput(isEnabled: false, select: { _ in }))
            let node = plot._makeNode(), layout = plot._makeLayoutNode()!
            plot._updateLayout(layout); plot._updateNode(node)
            #expect(node.isHitTestable == false && node.isFocusable == false)
            #expect(layout.height == 64)
            #expect(node.accessibility?.role == .image && node.accessibility?.label == "Sales")
        }
    }

    @Test("Lines stroke between values; areas fill down to the baseline")
    func lineAndArea() {
        let (line, _) = draw(cartesian(series: [.line([0, 1, 0])], thresholds: [], markers: []), style: flat())
        #expect(covers(CGPoint(x: 15, y: 30), in: line))
        #expect(covers(CGPoint(x: 60, y: 1), in: line))
        #expect(!covers(CGPoint(x: 60, y: 30), in: line))
        let (area, node) = draw(cartesian(series: [.area([0, 1, 0])], thresholds: [], markers: []), style: flat())
        #expect(covers(CGPoint(x: 60, y: 30), in: area, color: node.theme.colors.accent))
        #expect(area.vertices.allSatisfy { $0.posX.isFinite && $0.posY.isFinite })
    }

    @Test("Bar series group side by side, or accumulate when stacked")
    func groupedAndStackedBars() {
        let series: [ChartSeries] = [.bar([2, 4], title: "A"), .bar([2, 2], color: .success, title: "B")]
        let plot = UIRect(x: 0, y: 0, width: 100, height: 100)
        let grouped = geometry(series, style: ChartStyle(minValue: 0, barSpacing: 0), plot: plot)
        let a = grouped.marks[0][0].bar!, b = grouped.marks[1][0].bar!
        #expect(a.maxX <= b.minX + 0.5 && a.minY == b.minY)
        #expect(grouped.domain.upperBound == 4)
        let stacked = geometry(series, style: ChartStyle(minValue: 0, barSpacing: 0, stacking: .stacked), plot: plot)
        let low = stacked.marks[0][1].bar!, high = stacked.marks[1][1].bar!
        #expect(stacked.domain.upperBound == 6)
        #expect(abs(low.minX - high.minX) < 0.01 && abs(high.maxY - low.minY) < 0.01)
        #expect(!stacked.marks[0][1].isOuterEnd && stacked.marks[1][1].isOuterEnd && grouped.marks[0][0].isOuterEnd)
        // Hidden series take no space and leave the domain.
        let hidden = geometry(series, visible: [true, false], style: ChartStyle(minValue: 0, barSpacing: 0), plot: plot)
        #expect(hidden.marks[1].isEmpty && hidden.marks[0][0].bar!.width > a.width)
    }

    @Test("A negative value stacks under the baseline instead of cancelling the surplus")
    func negativeStacking() {
        let series: [ChartSeries] = [.bar([3, 3], title: "Gain"), .bar([-2, 1], color: .success, title: "Loss")]
        let plot = UIRect(x: 0, y: 0, width: 100, height: 100)
        let stacked = geometry(series, style: ChartStyle(barSpacing: 0, stacking: .stacked), plot: plot)
        let zero = stacked.along(0)
        // The loss hangs from zero rather than being subtracted from the gain.
        #expect(abs(stacked.marks[1][0].base - zero) < 0.01)
        #expect(stacked.marks[1][0].y > zero)
        // A gain still stacks on the gain.
        #expect(abs(stacked.marks[1][1].base - stacked.marks[0][1].y) < 0.01)
        #expect(stacked.domain.lowerBound <= -2 && stacked.domain.upperBound >= 4)
        // Each direction rounds only its own outermost segment.
        #expect(stacked.marks[0][0].isOuterEnd && stacked.marks[1][0].isOuterEnd)
        #expect(!stacked.marks[0][1].isOuterEnd && stacked.marks[1][1].isOuterEnd)
    }

    @Test("Percent stacking fills each index to 100 and reads as shares")
    func percentStacking() {
        let series: [ChartSeries] = [.bar([1, 3]), .bar([3, 1], color: .success)]
        let style = ChartStyle(barSpacing: 0, stacking: .percent)
        let plot = UIRect(x: 0, y: 0, width: 100, height: 100)
        let axes = ChartValueAxes.resolve(series: series, visible: [true, true], style: style,
                                          guides: ChartGuides(), axis: ChartConfiguration.Axis())
        #expect(axes.primaryScale.domain == 0...100)
        let stacked = geometry(series, style: style, plot: plot)
        // Both indexes reach the top of the plot, whichever series is larger.
        #expect(abs(stacked.marks[1][0].y - plot.minY) < 0.01)
        #expect(abs(stacked.marks[1][1].y - plot.minY) < 0.01)
        // The first segment covers its share: 1 of 4 is a quarter of the height.
        #expect(abs(stacked.marks[0][0].y - (plot.maxY - plot.height * 0.25)) < 0.5)
        #expect(abs(stacked.marks[0][1].y - (plot.maxY - plot.height * 0.75)) < 0.5)
    }

    @Test("Percent stacking keeps a negative side and spans -100 to 100")
    func percentWithNegatives() {
        let series: [ChartSeries] = [.bar([2]), .bar([-3], color: .error)]
        let style = ChartStyle(barSpacing: 0, stacking: .percent)
        let axes = ChartValueAxes.resolve(series: series, visible: [true, true], style: style,
                                          guides: ChartGuides(), axis: ChartConfiguration.Axis())
        #expect(axes.primaryScale.domain == -100...100)
        let plot = UIRect(x: 0, y: 0, width: 40, height: 100)
        let stacked = geometry(series, style: style, plot: plot)
        let zero = stacked.along(0)
        #expect(abs(zero - plot.maxY / 2) < 0.5)
        // The loss is the taller side, so it takes the full drop to -100 and the
        // gain keeps its 2/3 share of the half above zero.
        #expect(abs(stacked.marks[1][0].y - plot.maxY) < 0.5)
        #expect(abs(stacked.marks[0][0].y - (zero - (zero - plot.minY) * 2 / 3)) < 0.5)
    }

    @Test("A right-axis series fits its own scale and never stretches the left one")
    func dualAxis() {
        let series: [ChartSeries] = [.bar([2, 4], title: "Orders"),
                                     .line([1000, 2000], color: .warning, title: "Latency", axis: .right)]
        let style = ChartStyle(barSpacing: 0)
        let axes = ChartValueAxes.resolve(series: series, visible: [true, true], style: style,
                                          guides: ChartGuides(), axis: ChartConfiguration.Axis())
        #expect(axes.usesSecondary)
        #expect(axes.primaryScale.domain.upperBound == 4)
        #expect(axes.secondaryScale?.domain.upperBound == 2000)
        let plot = UIRect(x: 0, y: 0, width: 100, height: 100)
        let geometry = geometry(series, style: style, plot: plot)
        #expect(abs(geometry.marks[1][1].y - plot.minY) < 0.01)
        // A rule on the right axis widens only that side.
        let withRule = ChartValueAxes.resolve(
            series: series, visible: [true, true], style: style,
            guides: ChartGuides(thresholds: [ChartThreshold(value: 4000, axis: .right)]),
            axis: ChartConfiguration.Axis())
        #expect(withRule.primaryScale.domain.upperBound == 4)
        #expect((withRule.secondaryScale?.domain.upperBound ?? 0) >= 4000)
        // Without a right-axis series there is no second axis at all.
        let single = ChartValueAxes.resolve(series: [.bar([2, 4])], visible: [true], style: style,
                                            guides: ChartGuides(), axis: ChartConfiguration.Axis())
        #expect(!single.usesSecondary && single.secondaryScale == nil)
    }

    @Test("Lines and bars in one chart share slot centers; markers follow the same x")
    func mixedMarksAlign() {
        let geometry = geometry([.bar([1, 2, 3]), .line([1, 2, 3])],
                                style: ChartStyle(minValue: 0, barSpacing: 0),
                                plot: UIRect(x: 0, y: 0, width: 90, height: 30))
        #expect(geometry.marks[0].map(\.x) == geometry.marks[1].map(\.x))
        #expect(geometry.marks[1].map(\.x) == [15, 45, 75])
    }

    @Test("Scatter points use their own x values and the nearest point by distance")
    func scatter() {
        let geometry = geometry([.points(x: [0, 10, 20], y: [0, 10, 0])], x: .numeric,
                                plot: UIRect(x: 0, y: 0, width: 200, height: 100))
        // A numeric axis fits x to round bounds, so the ends sit on the plot edges.
        #expect(geometry.marks[0].map { $0.x.rounded() } == [0, 100, 200])
        #expect(geometry.nearest(x: 95, y: 5)?.index == 1)
        #expect(geometry.nearest(x: 190, y: 92)?.index == 2)
    }

    @Test("Donut wedges start at 12 o'clock, run clockwise and hit-test by angle and ring")
    func donut() {
        let slices = [ChartSlice("A", value: 1), ChartSlice("B", value: 1), ChartSlice("C", value: 2)]
        let pie = PieGeometry(slices: slices, visible: [true, true, true], innerRatio: 0.5, plot: UIRect(x: 0, y: 0, width: 108, height: 108))
        #expect(pie.total == 4 && pie.wedges.count == 3)
        #expect(pie.wedge(atX: 80, y: 30)?.index == 0)       // upper right
        #expect(pie.wedge(atX: 80, y: 80)?.index == 1)       // lower right
        #expect(pie.wedge(atX: 20, y: 54)?.index == 2)       // left half
        #expect(pie.wedge(atX: 54, y: 54) == nil)            // the hole
        let (list, node) = draw(.pie(slices), style: ChartStyle(showsGrid: false, contentInset: 0, innerRadiusRatio: 0.5),
                                size: CGSize(width: 108, height: 108))
        #expect(covers(CGPoint(x: 20, y: 54), in: list, color: node.theme.colors.warning))
        #expect(!covers(CGPoint(x: 54, y: 54), in: list))
        let hidden = PieGeometry(slices: slices, visible: [true, true, false], innerRatio: 0.5, plot: UIRect(x: 0, y: 0, width: 108, height: 108))
        #expect(hidden.total == 2 && hidden.wedge(atX: 20, y: 54)?.index == 1)
    }

    @Test("Thresholds and markers add guide geometry")
    func thresholdAndMarker() {
        let (list, node) = draw(cartesian(series: [.line([0, 1, 2])], thresholds: [ChartThreshold(value: 1, color: .error)],
                                           markers: [ChartMarker(index: 2, color: .success)]), style: flat())
        #expect(covers(CGPoint(x: 15, y: 20), in: list, color: node.theme.colors.error))
        #expect(covers(CGPoint(x: 120, y: 30), in: list, color: node.theme.colors.success))
    }

    @Test("Selection draws a rule and a ringed point, and dims unselected bars")
    func selectionPaint() {
        let (list, node) = draw(cartesian(series: [.line([0, 2, 0])], thresholds: [], markers: []), style: flat(),
                                selection: ChartPoint(series: 0, index: 1))
        #expect(covers(CGPoint(x: 60, y: 30), in: list))   // rule
        #expect(covers(CGPoint(x: 64.5, y: 1), in: list, color: node.theme.colors.surface))
    }

    @Test("A non-finite value breaks the line instead of drawing a zero")
    func gapBreaksLine() {
        let values: [Float] = [0, 1, .nan, 1, 0]
        let (list, _) = draw(cartesian(series: [.line(values)]), style: flat())
        // A coerced zero would draw a V through (60, 40) and cross this point.
        #expect(!covers(CGPoint(x: 45, y: 20), in: list))
        // The runs on either side stay intact.
        #expect(covers(CGPoint(x: 15, y: 20), in: list))
        #expect(covers(CGPoint(x: 105, y: 20), in: list))
    }

    @Test("A dash pattern leaves gaps in the stroke")
    func dashedStroke() {
        let dashed = ChartSeriesStyle(dash: [6, 6])
        let (list, _) = draw(cartesian(series: [.line([0.5, 0.5], style: dashed)]), style: flat())
        var drawn = 0, skipped = 0
        for x in stride(from: 2, through: 118, by: 2) {
            if covers(CGPoint(x: x, y: 20), in: list) { drawn += 1 } else { skipped += 1 }
        }
        #expect(drawn > 0 && skipped > 0)
    }

    @Test("A curved series bulges past the straight chord")
    func curvedLine() {
        let values: [Float] = [0, 10, 10]
        let straight = draw(cartesian(series: [.line(values)]), style: flat()).0
        let curved = draw(cartesian(series: [.line(values, style: ChartSeriesStyle(curve: true))]), style: flat()).0
        // Halfway along the first segment the chord reads 5 (y 20); the monotone
        // cubic reads about 6.25 (y 15).
        #expect(!covers(CGPoint(x: 30, y: 15), in: straight))
        #expect(covers(CGPoint(x: 30, y: 15), in: curved))
    }

    @Test("A series can widen its own stroke and size its own points")
    func perSeriesStyle() {
        let thick = draw(cartesian(series: [.line([0.5, 0.5], style: ChartSeriesStyle(lineWidth: 10))]), style: flat()).0
        let thin = draw(cartesian(series: [.line([0.5, 0.5])]), style: flat()).0
        #expect(covers(CGPoint(x: 60, y: 16), in: thick))
        #expect(!covers(CGPoint(x: 60, y: 16), in: thin))

        let big = draw(cartesian(series: [.points(x: [0], y: [0.5], style: ChartSeriesStyle(pointRadius: 8))]), style: flat()).0
        let small = draw(cartesian(series: [.points(x: [0], y: [0.5])]), style: flat()).0
        #expect(covers(CGPoint(x: 60, y: 14), in: big))
        #expect(!covers(CGPoint(x: 60, y: 14), in: small))
    }

    @Test("A numeric x domain puts bars and points on the same axis")
    func numericMixesMarks() {
        let bars = ChartSeries(points: [ChartDatum(x: 0, y: 5), ChartDatum(x: 10, y: 6)], mark: .bar)
        let dot = ChartSeries(points: [ChartDatum(x: 10, y: 3)], mark: .point)
        let geometry = geometry([bars, dot], x: .numeric, plot: UIRect(x: 0, y: 0, width: 200, height: 100))
        #expect(abs(geometry.marks[0][1].x - geometry.marks[1][0].x) < 0.01)
        #expect(geometry.isScatter)
    }

    @Test("A band shades its corridor and widens the domain to stay visible")
    func bandShading() {
        let (list, node) = draw(cartesian(series: [.line([0.5, 0.5])],
                                          bands: [ChartBand(from: 0.25, to: 0.75, color: .success)]),
                                style: flat())
        #expect(covers(CGPoint(x: 60, y: 20), in: list, color: node.theme.colors.success))
        // The corridor, not the data, decides the top of the domain here.
        let stack = ChartStack(series: [.line([0.5])], visible: [true], style: ChartStyle())
        let scale = ChartScale.fit(stack: stack, bands: [ChartBand(from: 0, to: 10)],
                                   style: ChartStyle(), tickCount: 4)
        #expect(scale.domain.upperBound >= 10)
    }

    @Test("A threshold label ends its rule; a band label opens its corridor")
    func guideLabels() throws {
        let items = overlay(cartesian(series: [.line([0, 10])],
                                      thresholds: [ChartThreshold(value: 5, label: "Target")],
                                      bands: [ChartBand(from: 2, to: 4, label: "Safe")]),
                            style: flat())
        let target = try #require(items.first { $0.item.kind == .threshold })
        let safe = try #require(items.first { $0.item.kind == .band })
        #expect(target.item.title == "Target" && safe.item.title == "Safe")
        let rule = try #require(target.placement), corner = try #require(safe.placement)
        // Right-aligned at the plot edge, above the rule; the band opens at the
        // left edge, below its top.
        #expect(rule.horizontal == 1 && rule.vertical == 1)
        #expect(rule.anchor.x > corner.anchor.x)
        #expect(corner.horizontal == 0 && corner.vertical == 0)
        #expect(rule.anchor.y < corner.anchor.y)
    }

    @Test("Annotations pin a note above the mark they name")
    func annotationAnchor() throws {
        let items = overlay(cartesian(series: [.bar([1, 4, 2])],
                                      annotations: [ChartAnnotation("Launch", index: 1)]),
                            style: flat())
        let note = try #require(items.first { $0.item.kind == .annotation })
        let placement = try #require(note.placement)
        #expect(note.item.title == "Launch")
        #expect(placement.vertical == 1 && placement.flips)
        #expect(abs(placement.anchor.x - 100) < 0.01)      // the middle slot
        #expect(placement.anchor.y < 50)                   // the tallest bar
    }

    @Test("Value labels follow showsValues and drop the ones that would collide")
    func valueLabels() {
        #expect(!overlay(cartesian(series: [.line([1, 2, 3])]), style: flat()).contains { $0.item.kind == .value })

        let shown = overlay(cartesian(series: [.line([1, 2, 3], style: ChartSeriesStyle(showsValues: true))]),
                            style: flat())
        let labels = shown.filter { $0.item.kind == .value }
        #expect(labels.count == 3 && labels.map(\.item.title) == ["1", "2", "3"])
        #expect(labels.allSatisfy { $0.placement?.vertical == 1 && $0.placement?.horizontal == 0.5 })

        // Twenty-four points in 120pt cannot all be labelled.
        let dense = (0..<24).map { Float($0 % 5) }
        let crowded = overlay(cartesian(series: [.line(dense, style: ChartSeriesStyle(showsValues: true))]),
                              style: flat(), size: CGSize(width: 120, height: 40))
        #expect(crowded.filter { $0.item.kind == .value }.count < 24)
    }

    @Test("A pie starts at another angle and sorts by value without losing slice identity")
    func pieOrdering() {
        let slices = [ChartSlice("A", value: 1), ChartSlice("B", value: 3)]
        let plot = UIRect(x: 0, y: 0, width: 100, height: 100)
        let straight = PieGeometry(slices: slices, visible: [true, true], innerRatio: 0, plot: plot)
        #expect(straight.wedges.map(\.index) == [0, 1])
        #expect(abs(straight.wedges[0].start + .pi / 2) < 0.001)

        let turned = PieGeometry(slices: slices, visible: [true, true], innerRatio: 0,
                                 startAngle: .pi / 2, plot: plot)
        #expect(abs(turned.wedges[0].start) < 0.001)

        let sorted = PieGeometry(slices: slices, visible: [true, true], innerRatio: 0,
                                 sortsByValue: true, plot: plot)
        #expect(sorted.wedges.map(\.index) == [1, 0] && sorted.wedges[0].value == 3)
        #expect(sorted.wedges.map(\.value).reduce(0, +) == sorted.total)
    }

    @Test("A pie writes a slice's name and share outside the ring when asked")
    func pieLabels() throws {
        let slices = [ChartSlice("A", value: 1), ChartSlice("B", value: 3)]
        var pie = ChartConfiguration.Pie()
        pie.showsLabels = true
        let items = overlay(.pie(slices), style: ChartStyle(showsGrid: false, innerRadiusRatio: 0),
                            size: CGSize(width: 120, height: 120), pie: pie)
        let labels = items.filter { $0.item.kind == .slice }
        #expect(labels.count == 2)
        #expect(labels.map(\.item.title) == ["A", "B"])
        #expect(labels.map(\.item.subtitle) == ["25%", "75%"])
        // A label grows away from the ring, on the side its slice sits on.
        let first = try #require(labels.first?.placement)
        #expect(first.horizontal == 0 || first.horizontal == 1)
        #expect(items.allSatisfy { $0.item.kind != .center })
    }

    @Test("A donut can carry content in its hole")
    func pieCenter() throws {
        var pie = ChartConfiguration.Pie()
        pie.center = .init("4", label: "Total")
        let items = overlay(.pie([ChartSlice("A", value: 1), ChartSlice("B", value: 3)]),
                            style: ChartStyle(showsGrid: false, innerRadiusRatio: 0.6),
                            size: CGSize(width: 120, height: 120), pie: pie)
        let center = try #require(items.first { $0.item.kind == .center })
        #expect(center.item.title == "4" && center.item.subtitle == "Total")
        #expect(center.placement?.anchor == CGPoint(x: 60, y: 60))
    }

    @Test("A histogram bins samples into even widths and labels the edges")
    func histogramBins() {
        let histogram = ChartHistogram(binCount: 4).build([0, 1, 1, 2, 3, 3, 3, 4])
        #expect(histogram.counts == [1, 2, 1, 4])
        #expect(histogram.labels == ["0–1", "1–2", "2–3", "3–4"])
        #expect(histogram.series.mark == .bar)
        #expect(histogram.series.data.map(\.y) == [1, 2, 1, 4])
    }

    @Test("A histogram drops samples outside an explicit range and skips non-finite ones")
    func histogramRange() {
        let histogram = ChartHistogram(binCount: 2, range: 0...10).build([-5, 0, 4, 6, 10, 99, .nan])
        #expect(histogram.counts == [2, 2])
    }

    @Test("A histogram of identical samples still divides a width")
    func histogramConstant() {
        let histogram = ChartHistogram(binCount: 3).build([5, 5, 5])
        #expect(histogram.counts.reduce(0, +) == 3)
        #expect(histogram.labels.count == 3)
    }
}
