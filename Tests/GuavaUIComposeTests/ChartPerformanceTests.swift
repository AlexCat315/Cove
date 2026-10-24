import Testing
#if canImport(CoreGraphics)
import CoreGraphics
#else
import Foundation
#endif
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Chart downsampling and geometry cache")
struct ChartPerformanceTests {
    private static var unlabeled: ChartConfiguration.Axis {
        var axis = ChartConfiguration.Axis()
        axis.x.isLabeled = false
        axis.y.isLabeled = false
        return axis
    }

    private func plot(_ series: [ChartSeries], selection: ChartPoint? = nil,
                      style: ChartStyle = ChartStyle()) -> ChartPlot {
        ChartPlot(content: .cartesian(series: series, guides: ChartGuides(), x: .categorical([])),
                  visible: Array(repeating: true, count: series.count), style: style,
                  axis: Self.unlabeled, pie: .init(),
                  format: { String(Int($0.rounded())) }, selection: selection,
                  summary: ("Chart", ""),
                  input: ChartPlotInput(isEnabled: false, select: { _ in }))
    }

    /// The marks one series resolves to at a given plot width.
    private func marks(_ values: [Float], width: Float) -> [ChartGeometry.Mark] {
        let series = [ChartSeries(values)]
        let style = ChartStyle(showsGrid: false, contentInset: 0)
        let axes = ChartValueAxes.resolve(series: series, visible: [true], style: style,
                                          guides: ChartGuides(), axis: Self.unlabeled)
        return ChartGeometry(series: series, axes: axes, xDomain: .categorical([]), xScale: nil,
                             barSpacing: style.barSpacing,
                             plot: UIRect(x: 0, y: 0, width: width, height: 40)).marks[0]
    }

    private func triangles(_ series: [ChartSeries], width: CGFloat) -> Int {
        GlobalTestLock.locked {
            // A hairline keeps the round joins out of the count, so every
            // triangle here belongs to the stroke itself.
            let plot = plot(series, style: ChartStyle(showsGrid: false, lineWidth: 1, contentInset: 0))
            let node = plot._makeNode()
            node.frame = CGRect(origin: .zero, size: CGSize(width: width, height: 40))
            plot._updateNode(node)
            let list = DrawList()
            node.draw?(list, .zero)
            return list.indices.count / 3
        }
    }

    @Test("A dense run keeps its envelope while shedding most of its points")
    func decimationKeepsEnvelope() {
        let values = (0..<2_000).map { sinf(Float($0) / 40) * 10 + 20 }
        let full = marks(values, width: 120)
        let reduced = ChartGeometry.decimated(full, length: 120)
        #expect(reduced.count <= 242)
        #expect(reduced.count < full.count / 4)
        // Extremes survive exactly: that is what picking per-column min/max buys.
        #expect(abs((reduced.map(\.y).min() ?? 0) - (full.map(\.y).min() ?? 0)) < 0.001)
        #expect(abs((reduced.map(\.y).max() ?? 0) - (full.map(\.y).max() ?? 0)) < 0.001)
        // Endpoints stay, and x never goes backwards.
        #expect(reduced.first?.x == full.first?.x && reduced.last?.x == full.last?.x)
        #expect(zip(reduced, reduced.dropFirst()).allSatisfy { $0.x <= $1.x })
    }

    @Test("A lone spike survives decimation")
    func decimationKeepsSpikes() {
        var values = [Float](repeating: 5, count: 1_000)
        values[500] = 100
        let full = marks(values, width: 120)
        let reduced = ChartGeometry.decimated(full, length: 120)
        #expect(reduced.contains { abs($0.y - full[500].y) < 0.001 })
    }

    @Test("A run that already fits the plot is left alone")
    func decimationLeavesSmallRuns() {
        let full = marks((0..<40).map { Float($0) }, width: 120)
        #expect(full.count == 40)
        #expect(ChartGeometry.decimated(full, length: 120).count == full.count)
    }

    @Test("A dense line draws segments per pixel, not per sample")
    func denseLineDrawsCompactly() {
        let dense = (0..<4_000).map { sinf(Float($0) / 60) * 10 + 20 }
        let compact = triangles([.line(dense)], width: 120)
        // With the room to resolve them, every sample still gets drawn.
        let roomy = triangles([.line(dense)], width: 4_000)
        #expect(compact > 0)
        #expect(compact * 8 < roomy)
    }

    @Test("Drawing and hit-testing share one resolution")
    func resolutionIsShared() {
        let plot = plot([.line([1, 2, 3, 4])])
        let cache = ChartResolutionCache()
        let text = ChartText(theme: .defaultLight, environment: nil)
        let size = CGSize(width: 120, height: 40)
        _ = plot.resolve(in: size, text: text, cache: cache)
        _ = plot.resolve(in: size, text: text, cache: cache)
        #expect(cache.builds == 1)
        _ = plot.resolve(in: CGSize(width: 200, height: 40), text: text, cache: cache)
        #expect(cache.builds == 2)
    }

    @Test("The two spaces a chart lives in keep separate entries")
    func twoSpacesCoexist() {
        let plot = plot([.line([1, 2, 3])])
        let cache = ChartResolutionCache()
        let text = ChartText(theme: .defaultLight, environment: nil)
        let size = CGSize(width: 120, height: 40), absolute = CGPoint(x: 20, y: 8)
        _ = plot.resolve(in: size, origin: .zero, text: text, cache: cache)
        _ = plot.resolve(in: size, origin: absolute, text: text, cache: cache)
        #expect(cache.builds == 2)
        // Alternating between them is free.
        _ = plot.resolve(in: size, origin: .zero, text: text, cache: cache)
        _ = plot.resolve(in: size, origin: absolute, text: text, cache: cache)
        #expect(cache.builds == 2)
    }

    @Test("New data drops the cached geometry; a new selection keeps it")
    func signatureInvalidates() throws {
        try GlobalTestLock.locked {
            let size = CGSize(width: 160, height: 60)
            let node = plot([.line([1, 2, 3])])._makeNode()
            node.frame = CGRect(origin: .zero, size: size)
            let text = ChartText(theme: node.theme, environment: nil)
            plot([.line([1, 2, 3])])._updateNode(node)
            let session = try #require(node.attachments[ChartSession.key] as? ChartSession)

            _ = plot([.line([1, 2, 3])]).resolve(in: size, text: text, cache: session.cache)
            #expect(session.cache.builds == 1)

            // A changed value moves the marks, so the cache goes.
            plot([.line([1, 2, 4])])._updateNode(node)
            _ = plot([.line([1, 2, 4])]).resolve(in: size, text: text, cache: session.cache)
            #expect(session.cache.builds == 2)

            // A selection repaints the chart but moves no mark.
            let selected = plot([.line([1, 2, 4])], selection: ChartPoint(series: 0, index: 1))
            selected._updateNode(node)
            _ = selected.resolve(in: size, text: text, cache: session.cache)
            #expect(session.cache.builds == 2)
        }
    }
}
