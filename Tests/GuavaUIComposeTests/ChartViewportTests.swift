import Foundation
import Testing
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Chart viewport and transitions", .serialized)
@MainActor
struct ChartViewportTests {
    private final class Harness: View {
        @State var range: ClosedRange<Float>?
        @State var brushed: ClosedRange<Float>?
        @State var point: ChartPoint?
        @State var values: [Float] = (0..<10).map(Float.init)
        let horizontal: Bool
        let brushing: Bool
        init(horizontal: Bool = false, brushing: Bool = true) {
            self.horizontal = horizontal; self.brushing = brushing
        }
        var body: some View {
            Chart([.line(values)], style: ChartStyle(minValue: 0, maxValue: 10, contentInset: 0,
                                                    orientation: horizontal ? .horizontal : .vertical)) {
                $0.label = "Viewport"
                $0.axis.x.isLabeled = false; $0.axis.y.isLabeled = false
                $0.viewport.range = self.$range
                $0.selection.point = self.$point
                if self.brushing { $0.selection.range = self.$brushed }
            }
            .frame(width: 400, height: 220)
        }
    }

    private func scene(_ harness: Harness, _ work: (ViewGraph, PlatformInputContext, EventDispatcher, Node) throws -> Void) throws {
        try GlobalTestLock.locked {
            let old = TextEnvironmentHolder.current; TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer { TextEnvironmentHolder.current = old }
            let context = PlatformInputContext()
            try context.withCurrent {
                let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
                graph.install(root: harness); settle(graph)
                defer { graph.install(root: EmptyView()) }
                func nodes(_ node: Node?) -> [Node] { guard let node else { return [] }; return [node] + node.children.flatMap(nodes) }
                let plot = try #require(nodes(graph.tree.root).first { $0.accessibility?.label == "Viewport" })
                try work(graph, context, EventDispatcher(tree: graph.tree, interactions: context.interactions,
                                                         capture: context.pointerCapture, focusChain: context.focusChain), plot)
            }
        }
    }

    private func settle(_ graph: ViewGraph) {
        for _ in 0..<6 { graph.recomposer.commitAll(); graph.computeLayout(width: 400, height: 220) }
    }

    private func drag(_ a: CGPoint, _ b: CGPoint, _ input: EventDispatcher, modifiers: KeyModifiers = []) {
        input.dispatch(.mouseButtonDown(.init(button: .left, x: Float(a.x), y: Float(a.y), clicks: 1, modifiers: modifiers)))
        input.dispatch(.mouseMotion(.init(x: Float(b.x), y: Float(b.y), deltaX: Float(b.x - a.x), deltaY: Float(b.y - a.y))))
        input.dispatch(.mouseButtonUp(.init(button: .left, x: Float(b.x), y: Float(b.y), clicks: 1, modifiers: modifiers)))
    }

    @Test("Wheel zoom anchors at the pointer and keyboard reset fits the full domain")
    func zoomAndReset() throws {
        let harness = Harness(brushing: false)
        try scene(harness) { graph, context, input, plot in
            let f = plot.absoluteFrame
            input.dispatch(.mouseWheel(.init(x: 0, y: 5, mouseX: Float(f.midX), mouseY: Float(f.midY))))
            settle(graph)
            let range = try #require(harness.range)
            #expect(range.lowerBound > 0 && range.upperBound < 9)
            #expect(abs((range.lowerBound + range.upperBound) / 2 - 4.5) < 0.001)
            context.focusChain.focus(plot)
            input.dispatch(.keyDown(.init(scancode: Scancode.digit0, keycode: 0, modifiers: [], isRepeat: false)))
            settle(graph)
            #expect(harness.range == nil)
        }
    }

    @Test("Reverse brushing returns data coordinates within the zoomed interval")
    func brushZoomed() throws {
        let harness = Harness(); harness.range = 2...6
        try scene(harness) { graph, context, input, plot in
            let f = plot.absoluteFrame
            drag(CGPoint(x: f.minX + f.width * 0.8, y: f.midY),
                 CGPoint(x: f.minX + f.width * 0.2, y: f.midY), input)
            settle(graph)
            let range = try #require(harness.brushed)
            #expect(abs(range.lowerBound - 2.8) < 0.01 && abs(range.upperBound - 5.2) < 0.01)
            #expect(harness.point == nil)
            #expect(context.pointerCapture.target == nil)
            context.focusChain.focus(plot)
            input.dispatch(.keyDown(.init(scancode: Scancode.escape, keycode: 0, modifiers: [], isRepeat: false)))
            settle(graph)
            #expect(harness.brushed == nil && harness.range == 2...6)
        }
    }

    @Test("Horizontal brushing follows categories down the screen; Alt-drag pans instead")
    func horizontalBrushAndPan() throws {
        let harness = Harness(horizontal: true); harness.range = 2...6
        try scene(harness) { graph, _, input, plot in
            let f = plot.absoluteFrame
            drag(CGPoint(x: f.midX, y: f.minY + f.height * 0.2),
                 CGPoint(x: f.midX, y: f.minY + f.height * 0.8), input)
            settle(graph)
            let brushed = try #require(harness.brushed)
            #expect(abs(brushed.lowerBound - 2.8) < 0.01 && abs(brushed.upperBound - 5.2) < 0.01)
            drag(CGPoint(x: f.midX, y: f.minY + f.height * 0.8),
                 CGPoint(x: f.midX, y: f.minY + f.height * 0.4), input, modifiers: .lalt)
            settle(graph)
            let range = try #require(harness.range)
            #expect(abs(range.lowerBound - 3.6) < 0.01 && abs(range.upperBound - 7.6) < 0.01)
            #expect(harness.brushed == brushed)
        }
    }

    @Test("Logarithmic navigation preserves the anchored value and clamps to the full domain")
    func logarithmicNavigation() {
        let full = ChartWindow(full: 1...1000, kind: .log)
        let zoomed = full.zoom(factor: 0.5, anchor: 0.5)
        #expect(abs(zoomed.value(at: 0.5) - sqrt(1000)) < 0.01)
        #expect(abs(zoomed.fraction(zoomed.value(at: 0.27)) - 0.27) < 0.0001)
        #expect(zoomed.pan(fraction: 100).range.upperBound <= 1000.001)
        #expect(zoomed.zoom(factor: 100, anchor: 0).range == full.range)
        #expect(full.replacing(-1...10).range == full.range)
        #expect(full.zoom(factor: .nan, anchor: 0.5) == full)
    }

    @Test("Zoomed ticks, marks and picking retain original indices in either orientation")
    func zoomedGeometry() throws {
        for orientation in [ChartOrientation.vertical, .horizontal] {
            let plot = ChartPlot(content: .cartesian(series: [.line((0..<10).map(Float.init))],
                                                     guides: ChartGuides(),
                                                     x: .categorical((0..<10).map { "Category \($0)" })),
                                 visible: [true], style: ChartStyle(orientation: orientation),
                                 axis: ChartConfiguration.Axis(), pie: ChartConfiguration.Pie(),
                                 format: { String($0) }, selection: nil, summary: ("Test", ""),
                                 visibleRange: 2...6, input: ChartPlotInput(isEnabled: false) { _ in })
            let resolution = plot.resolve(in: CGSize(width: 600, height: 400), text: ChartText(theme: .defaultLight, environment: nil))
            let geometry = try #require(resolution.cartesian)
            let mark = geometry.marks[0][4]
            #expect(geometry.nearest(x: mark.x, y: mark.y)?.index == 4)
            #expect(resolution.frame.categoryTicks.allSatisfy { tick in
                tick.position >= orientation.acrossOrigin(geometry.plot)
                    && tick.position <= orientation.acrossOrigin(geometry.plot) + orientation.breadth(geometry.plot)
            })
            let cropped = geometry.visibleRun(geometry.marks[0])
            #expect(cropped.map(\.index) == Array(1...7))
            #expect(geometry.nearest(x: geometry.marks[0][0].x, y: geometry.marks[0][0].y)?.index == 2)
        }
    }

    @Test("Data transitions use the presentation value when interrupted and snap for reduced motion")
    func transitions() {
        let scheduler = AnimatorScheduler()
        AnimatorScheduler.$current.withValue(scheduler) {
            let node = Node(), cache = ChartResolutionCache(), transition = ChartTransition()
            func content(_ value: Float) -> ChartContent { .cartesian(series: [.bar([value])], guides: ChartGuides(), x: .categorical([])) }
            transition.update(content: content(2), animation: nil, node: node, cache: cache)
            transition.update(content: content(8), animation: Animation(duration: 1, curve: .linear), node: node, cache: cache)
            scheduler.tick(deltaTime: 0.5)
            #expect(abs((cache.samples?.series.first?.first?.y ?? 0) - 5) < 0.001)
            transition.update(content: content(4), animation: Animation(duration: 1, curve: .linear), node: node, cache: cache)
            scheduler.tick(deltaTime: 0.5)
            #expect(abs((cache.samples?.series.first?.first?.y ?? 0) - 4.5) < 0.001)
            scheduler.tick(deltaTime: 1)
            #expect(cache.samples?.series.first?.first?.y == 4)
            transition.update(content: content(9), animation: Animation(duration: 1, curve: .linear), node: node, cache: cache)
            scheduler.tick(deltaTime: 0.2)
            node.setCompositionValue(ReducedMotionEnvironment.key, true)
            transition.update(content: content(9), animation: Animation(duration: 1, curve: .linear), node: node, cache: cache)
            #expect(cache.samples?.series.first?.first?.y == 9)
            scheduler.tick(deltaTime: 1)
            #expect(scheduler.activeCount == 0)
            node.cancelAnimation(for: "chart.samples")
        }
    }
}
