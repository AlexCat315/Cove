import Foundation
import Testing
import CoveUIRuntime
@testable import CoveUICompose

/// What a reader who cannot see the plot is given: one stop per value on the
/// chart itself, and the whole set as a table beside it.
@Suite("Chart accessibility", .serialized)
@MainActor
struct ChartAccessibilityTests {
    private struct Harness: View {
        @State var point: ChartPoint?
        var binds = true
        var exposesValues = true
        var values: [Float] = [1, 2, 3, 4]
        var pie = false

        var body: some View {
            let configure: (inout ChartConfiguration) -> Void = {
                $0.label = "Sales"
                $0.accessibility.exposesValues = self.exposesValues
                if self.binds { $0.selection.point = self.$point }
            }
            return Box(direction: .column, alignItems: .stretch) {
                if pie {
                    Chart(slices: [ChartSlice("Desktop", value: 3), ChartSlice("Mobile", value: 1)],
                          configure: configure)
                } else {
                    Chart([.bar(values, title: "Revenue"),
                           .line(values.map { $0 * 2 }, color: .success, title: "Signups")],
                          x: .categorical(["Jan", "Feb", "Mar", "Apr"]), configure: configure)
                }
            }
            .frame(width: 400, height: 200)
        }
    }

    nonisolated private func nodes(_ node: Node?) -> [Node] {
        guard let node else { return [] }
        return [node] + node.children.flatMap(nodes)
    }

    private func scene(_ harness: Harness, _ work: (ViewGraph) throws -> Void) throws {
        try GlobalTestLock.locked {
            let old = TextEnvironmentHolder.current
            TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer { TextEnvironmentHolder.current = old }
            let context = PlatformInputContext()
            try context.withCurrent {
                let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
                graph.install(root: harness)
                settle(graph, harness)
                defer { graph.install(root: EmptyView()) }
                try work(graph)
            }
        }
    }

    private func settle(_ graph: ViewGraph, _ harness: Harness) {
        for _ in 0..<6 {
            graph.recomposer.commitAll()
            graph.computeLayout(width: 400, height: 200)
        }
    }

    private func chart(_ graph: ViewGraph) throws -> Node {
        try #require(nodes(graph.tree.root).first { $0.accessibility?.role == .image })
    }

    @Test("A chart without a binding still reads one value at a time")
    func readsWithoutBinding() throws {
        let harness = Harness(binds: false)
        try scene(harness) { graph in
            let chart = try chart(graph)
            // Nothing selected: the chart describes its range in one sentence.
            #expect(chart.accessibility?.value.contains("to") == true)
            let increment = try #require(chart.accessibilityActions.increment)
            increment()
            let first = try #require(chart.accessibility?.value)
            #expect(first.hasPrefix("Jan: Revenue 1, Signups 2"))
            #expect(first.contains("Point 1 of 4"))
            chart.accessibilityActions.increment?()
            let second = try #require(chart.accessibility?.value)
            #expect(second.hasPrefix("Feb:"))
            #expect(second.contains("Point 2 of 4"))
            chart.accessibilityActions.decrement?()
            #expect(chart.accessibility?.value.hasPrefix("Jan:") == true)
        }
    }

    @Test("Stepping skips a gap rather than reading a value that was never measured")
    func steppingSkipsGaps() throws {
        let harness = Harness(binds: false, values: [1, .nan, 3, 4])
        try scene(harness) { graph in
            let chart = try chart(graph)
            chart.accessibilityActions.increment?()
            #expect(chart.accessibility?.value.hasPrefix("Jan:") == true)
            // Feb is a gap, so the second step lands on March.
            chart.accessibilityActions.increment?()
            let next = try #require(chart.accessibility?.value)
            #expect(next.hasPrefix("Mar: Revenue 3"))
            #expect(next.contains("Point 3 of 4"))
        }
    }

    @Test("A bound chart steps its binding, and the readout follows it")
    func stepsTheBinding() throws {
        let harness = Harness()
        try scene(harness) { graph in
            let chart = try chart(graph)
            #expect(harness.point == nil)
            chart.accessibilityActions.increment?()
            #expect(harness.point == ChartPoint(series: 0, index: 0))
            settle(graph, harness)
            #expect(chart.accessibility?.value.hasPrefix("Jan:") == true)
        }
    }

    @Test("Turning value exposure off leaves the summary and offers no stepping")
    func summaryOnly() throws {
        let harness = Harness(binds: false, exposesValues: false)
        try scene(harness) { graph in
            let chart = try chart(graph)
            #expect(chart.accessibilityActions.increment == nil)
            #expect(chart.accessibilityActions.decrement == nil)
            #expect(chart.accessibility?.value.contains("to") == true)
        }
    }

    @Test("A pie reads one slice at a time, with its share")
    func readsSlices() throws {
        let harness = Harness(binds: false, pie: true)
        try scene(harness) { graph in
            let chart = try chart(graph)
            chart.accessibilityActions.increment?()
            let value = try #require(chart.accessibility?.value)
            #expect(value.contains("Desktop"))
            // The callout's middle dot is spoken as nothing; a comma is not.
            #expect(value.contains("3, 75%"))
            #expect(value.contains("Slice 1 of 2"))
        }
    }

    // MARK: The table alternative

    private func elements<Content: View>(_ root: Content) throws -> [AccessibilityElement] {
        try GlobalTestLock.locked {
            let old = TextEnvironmentHolder.current
            TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer { TextEnvironmentHolder.current = old }
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            graph.install(root: root)
            defer { graph.install(root: EmptyView()) }
            for _ in 0..<4 {
                graph.recomposer.commitAll()
                graph.computeLayout(width: 320, height: 400)
            }
            let root = try #require(graph.tree.root)
            return flatten(AccessibilityTree.snapshot(root: root))
        }
    }

    private func flatten(_ list: [AccessibilityElement]) -> [AccessibilityElement] {
        list.flatMap { [$0] + flatten($0.children) }
    }

    @Test("A cartesian table is a table of rows and cells, each naming its column")
    func tableColumns() throws {
        let series = [ChartSeries.bar([1, 2, 3, 4], title: "Revenue"),
                      ChartSeries.line([2, 4, 6, 8], color: .success, title: "Signups")]
        let all = try elements(
            ChartDataTable(series, x: .categorical(["Jan", "Feb", "Mar", "Apr"]), label: "Revenue by month")
                .frame(width: 320))

        let table = try #require(all.first { $0.semantics.role == .table })
        #expect(table.semantics.label == "Revenue by month")
        // The table owns its rows, so a reader can move by row and column.
        #expect(table.children.contains { $0.semantics.role == .row })

        let rows = all.filter { $0.semantics.role == .row }
        #expect(rows.count == 5)
        #expect(rows.first?.semantics.label == "Columns")
        #expect(rows.dropFirst().map(\.semantics.label) == ["Jan", "Feb", "Mar", "Apr"])

        let april = try #require(all.first { $0.semantics.role == .cell && $0.semantics.label == "Signups" && $0.semantics.value == "8" })
        #expect(april.semantics.value == "8")
        // A gap is a cell too, so the grid keeps its shape.
        #expect(all.contains { $0.semantics.role == .cell && $0.semantics.label == "Revenue" })
    }

    @Test("A series that ends early leaves a gap rather than shifting its values")
    func tableShortSeries() throws {
        let series = [ChartSeries.bar([1, 2, 3], title: "Revenue"),
                      ChartSeries.line([2], color: .success, title: "Signups")]
        let all = try elements(ChartDataTable(series, x: .categorical(["Jan", "Feb", "Mar"])).frame(width: 320))
        let signups = all.filter { $0.semantics.role == .cell && $0.semantics.label == "Signups" }
        #expect(signups.count == 3)
        #expect(signups.map(\.semantics.value) == ["2", "—", "—"])
    }

    @Test("A long table says how many rows it left out")
    func tableCap() throws {
        let values = (0..<20).map { Float($0) }
        let all = try elements(
            ChartDataTable([ChartSeries.line(values, title: "Revenue")], label: "Trend", maximumRows: 5)
                .frame(width: 320))
        #expect(all.filter { $0.semantics.role == .row }.count == 6)
        let note = try #require(all.first { $0.semantics.role == .status })
        #expect(note.semantics.label == "15 more rows not shown")
    }

    @Test("Series that do not share x list one datum per row")
    func tableLongForm() throws {
        let a = ChartSeries(points: [ChartDatum(x: 1, y: 10), ChartDatum(x: 2, y: 20)], mark: .point, title: "A")
        let b = ChartSeries(points: [ChartDatum(x: 5, y: 50)], mark: .point, title: "B")
        let all = try elements(ChartDataTable([a, b], x: .numeric, label: "Scatter").frame(width: 320))
        let rows = all.filter { $0.semantics.role == .row }
        #expect(rows.count == 4)
        #expect(rows.dropFirst().map(\.semantics.label) == ["A", "A", "B"])
        // Both values are present: the x it was measured at, and the value.
        #expect(all.contains { $0.semantics.role == .cell && $0.semantics.label == "X" && $0.semantics.value == "5" })
        #expect(all.contains { $0.semantics.role == .cell && $0.semantics.label == "Value" && $0.semantics.value == "50" })
    }

    @Test("A pie table lists each slice with its share of the total")
    func tablePie() throws {
        let all = try elements(
            ChartDataTable(slices: [ChartSlice("Desktop", value: 3), ChartSlice("Mobile", value: 1)], label: "Traffic")
                .frame(width: 320))
        let table = try #require(all.first { $0.semantics.role == .table })
        #expect(table.semantics.label == "Traffic")
        #expect(all.filter { $0.semantics.role == .row }.count == 3)
        #expect(all.contains { $0.semantics.role == .cell && $0.semantics.label == "Share" && $0.semantics.value == "75%" })
        #expect(all.contains { $0.semantics.role == .cell && $0.semantics.label == "Share" && $0.semantics.value == "25%" })
    }

    @Test("The table and the chart format a value the same way")
    func sharedFormatter() {
        #expect(ChartConfiguration.Selection.defaultText(3) == "3")
        #expect(ChartConfiguration.Selection.defaultText(1.5) == "1.50")
    }
}
