import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Chart scale and axis frame")
struct ChartAxisTests {
    private func scale(_ series: [ChartSeries], style: ChartStyle = ChartStyle(), tickCount: Int = 4,
                       thresholds: [ChartThreshold] = []) -> ChartScale {
        let stack = ChartStack(series: series, visible: Array(repeating: true, count: series.count), style: style)
        return ChartScale.fit(stack: stack, thresholds: thresholds, style: style, tickCount: tickCount)
    }

    private func frame(_ series: [ChartSeries], axis: ChartConfiguration.Axis,
                       x: ChartXDomain = .categorical([]),
                       style: ChartStyle = ChartStyle(contentInset: 0),
                       size: CGSize = CGSize(width: 200, height: 100)) -> ChartFrame {
        let stack = ChartStack(series: series, visible: Array(repeating: true, count: series.count), style: style)
        let yScale = ChartScale.fit(stack: stack, thresholds: [], style: style, tickCount: axis.y.tickCount)
        let xScale = x.isNumeric
            ? ChartScale.fit(extent: stack.shown.flatMap { series[$0].data.map(\.x) },
                             lower: nil, upper: nil, tickCount: axis.x.tickCount)
            : nil
        return ChartFrame.cartesian(stack: stack, xDomain: x, xScale: xScale, yScale: yScale,
                                    style: style, axis: axis, size: size, origin: .zero,
                                    text: ChartText(theme: .defaultLight, environment: nil))
    }

    private var unlabeled: ChartConfiguration.Axis {
        var axis = ChartConfiguration.Axis()
        axis.x.isLabeled = false
        axis.y.isLabeled = false
        return axis
    }

    @Test("A 0...47 range fits to 0...50 with a tick every 10")
    func niceTicks() {
        let scale = scale([.line([0, 47])])
        #expect(scale.domain.lowerBound == 0 && scale.domain.upperBound == 50)
        #expect(scale.ticks == [0, 10, 20, 30, 40, 50])
        #expect(scale.ticks.map(scale.label) == ["0", "10", "20", "30", "40", "50"])
    }

    @Test("Explicit bounds are kept; ticks stay round inside them")
    func explicitBounds() {
        let scale = scale([.line([0, 47])], style: ChartStyle(minValue: 0, maxValue: 47))
        #expect(scale.domain.lowerBound == 0 && scale.domain.upperBound == 47)
        #expect(scale.ticks == [0, 10, 20, 30, 40])
    }

    @Test("A fractional range ticks on quarters and trims trailing zeros")
    func fractionalTicks() {
        let scale = scale([.line([0, 1])])
        #expect(scale.ticks == [0, 0.25, 0.5, 0.75, 1])
        #expect(scale.ticks.map(scale.label) == ["0", "0.25", "0.5", "0.75", "1"])
    }

    @Test("Negative ranges keep zero on a tick")
    func negativeTicks() {
        let scale = scale([.bar([-30, 80])])
        #expect(scale.domain == -50...100)
        #expect(scale.ticks.first == -50 && scale.ticks.last == 100)
        #expect(scale.ticks.contains(0))
    }

    @Test("A constant series still gets a usable range")
    func constantSeries() {
        let scale = scale([.line([5, 5])])
        #expect(scale.domain.lowerBound < 5 && scale.domain.upperBound > 5)
        #expect(scale.ticks.count >= 2)
    }

    @Test("A zero tick count keeps the fitted range but draws no ticks")
    func noTicks() {
        let scale = scale([.line([0, 47])], tickCount: 0)
        #expect(scale.ticks.isEmpty)
        #expect(scale.domain.upperBound == 50)
    }

    @Test("Unlabelled axes leave the whole box to the plot; labelled ones take gutters")
    func gutters() {
        let series: [ChartSeries] = [.line([0, 47])]
        let bare = frame(series, axis: unlabeled)
        #expect(bare.plot == bare.bounds)
        #expect(bare.yTicks.count == 6 && bare.yTicks.allSatisfy { $0.label.isEmpty })

        var axis = unlabeled
        axis.y.isLabeled = true
        let labeled = frame(series, axis: axis)
        #expect(labeled.plot.minX > labeled.bounds.minX)
        #expect(labeled.plot.maxX == labeled.bounds.maxX)
        #expect(labeled.plot.width < bare.plot.width)
        #expect(labeled.yTicks.allSatisfy { !$0.label.isEmpty })
    }

    @Test("Tick labels sit on the value they name")
    func ticksFollowTheScale() {
        var axis = unlabeled
        axis.y.isLabeled = true
        let series: [ChartSeries] = [.line([0, 47])]
        let frame = frame(series, axis: axis)
        let stack = ChartStack(series: series, visible: [true], style: ChartStyle(contentInset: 0))
        let scale = ChartScale.fit(stack: stack, thresholds: [], style: ChartStyle(contentInset: 0), tickCount: 4)
        #expect(frame.yTicks.map(\.position) == scale.ticks.map { scale.y($0, in: frame.plot) })
        #expect(frame.yTicks.map(\.position) == frame.yTicks.map(\.position).sorted(by: >))
    }

    @Test("Crowded category labels thin out instead of overlapping")
    func labelThinning() {
        var axis = unlabeled
        axis.x.isLabeled = true
        let labels = (0..<12).map { "Month \($0 + 1)" }
        let series: [ChartSeries] = [.line((0..<12).map { Float($0) })]
        let wide = frame(series, axis: axis, x: .categorical(labels), size: CGSize(width: 900, height: 120))
        let narrow = frame(series, axis: axis, x: .categorical(labels), size: CGSize(width: 220, height: 120))
        #expect(wide.xTicks.count == 12)
        #expect(narrow.xTicks.count < 12 && !narrow.xTicks.isEmpty)
        #expect(narrow.xTicks.first?.label == "Month 1")
        // The reserved gutter follows the label row either way.
        #expect(narrow.plot.maxY < narrow.bounds.maxY)
    }

    @Test("A numeric x axis ticks on round values and labels them")
    func numericTicks() {
        var axis = unlabeled
        axis.x.isLabeled = true
        let series: [ChartSeries] = [.points(x: [0, 120, 240, 360, 480], y: [1, 2, 3, 4, 5])]
        let plain = frame(series, axis: axis, x: .numeric, size: CGSize(width: 240, height: 120))
        #expect(plain.xTicks.map(\.label) == ["0", "100", "200", "300", "400", "500"])
        #expect(plain.xTicks.map(\.position).first == plain.plot.minX)
        #expect(plain.plot.maxY < plain.bounds.maxY)
        // A custom formatter drives the labels.
        axis.x.format = { "\(Int($0))ms" }
        let formatted = frame(series, axis: axis, x: .numeric, size: CGSize(width: 240, height: 120))
        #expect(formatted.xTicks.first?.label == "0ms")
    }

    @Test("Axis titles reserve their own room")
    func titles() {
        var axis = unlabeled
        axis.y.title = "Revenue"
        axis.x.title = "Month"
        axis.x.isLabeled = true
        let withTitles = frame([.line([1, 2])], axis: axis, x: .categorical(["Jan", "Feb"]))
        let without = frame([.line([1, 2])], axis: unlabeled)
        #expect(withTitles.plot.minY > without.plot.minY)
        #expect(withTitles.plot.height < without.plot.height)
        #expect(withTitles.yTitle == "Revenue" && withTitles.xTitle == "Month")
    }
}
