import Testing
import CoveUIRuntime
@testable import CoveUICompose

@Suite("Value-state subtree reuse", .serialized)
struct ValueSubtreeTests: CoveUIComposeSerializedSuite {
    nonisolated(unsafe) private static var runs = 0
    nonisolated(unsafe) private static var binding: Binding<Int>?
    private struct Leaf: View {
        let input: Int
        @State var count = 0
        var body: some View {
            let _ = { SelfRecord.record($count) }()
            Text("\(input):\(count)")
        }
    }
    private enum SelfRecord {
        static func record(_ value: Binding<Int>) { ValueSubtreeTests.runs += 1; ValueSubtreeTests.binding = value }
    }
    private struct Root: View {
        @State var tick = 0
        @State var input = 7
        var body: some View {
            let _ = tick
            Column { Leaf(input: input) }
        }
    }
    @Test func equalInputsKeepTheOriginalStateAndCallbacks() { GlobalTestLock.locked {
        Self.runs = 0
        let old = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
        defer { TextEnvironmentHolder.current = old; Self.binding = nil }
        let root = Root(), graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        graph.install(root: root)
        root.tick += 1
        graph.recomposer.commitAll()
        #expect(Self.runs == 1)
        #expect(graph.skippedParentUpdates > 0)
        Self.binding?.wrappedValue = 1
        graph.recomposer.commitAll()
        #expect(Self.runs == 2)
        root.input = 8
        graph.recomposer.commitAll()
        #expect(Self.runs == 3)
        #expect(Self.binding?.wrappedValue == 1)
    } }
}
