import Testing
import CoveUIRuntime
@testable import CoveUICompose
#if canImport(Observation)
import Observation

@Suite("Swift Observation", .serialized)
struct SwiftObservationTests: CoveUIComposeSerializedSuite {
    @Observable final class Model {
        var first = 0
        var second = 0
        var readsFirst = true
    }
    private final class Counter { var value = 0 }
    private struct Reader: View {
        let counter: Counter
        let read: () -> Int
        var body: some View {
            let _ = { counter.value += 1 }()
            Text(String(read()))
        }
    }
    private struct Root: View {
        let model: Model
        let root: Counter
        let first: Counter
        let second: Counter
        var body: some View {
            let _ = { root.value += 1 }()
            Column {
                Reader(counter: first) { model.readsFirst ? model.first : 0 }
                Reader(counter: second) { model.second }
            }
        }
    }

    @Test func onlyReadersRecomposeAndDependenciesAreReplaced() { GlobalTestLock.locked {
        let previous = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
        defer { TextEnvironmentHolder.current = previous }
        let model = Model(), root = Counter(), first = Counter(), second = Counter()
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        graph.install(root: Root(model: model, root: root, first: first, second: second))
        model.first = 1
        #expect(graph.recomposer.hasPending)
        graph.recomposer.commitAll()
        #expect(root.value == 1)
        #expect(first.value == 2)
        #expect(second.value == 1)
        model.first = 2
        graph.recomposer.commitAll()
        #expect(first.value == 3)
        model.readsFirst = false
        graph.recomposer.commitAll()
        model.first = 3
        #expect(!graph.recomposer.hasPending)
        graph.install(root: EmptyView())
        model.second = 1
        #expect(!graph.recomposer.hasPending)
    } }

    @Test func readerClosuresObserveSwiftProperties() { GlobalTestLock.locked {
        let model = Model(), counter = Counter()
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        graph.install(root: ThemeReader { _ in
            let _ = { counter.value += 1 }()
            Spacer().frame(width: Float(model.first + 1), height: 1)
        })
        model.first = 10
        graph.recomposer.commitAll()
        #expect(counter.value == 2)
        graph.install(root: EmptyView())
    } }

    @Test func graphDeinitializationCancelsDependencies() throws { try GlobalTestLock.locked {
        let previous = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
        defer { TextEnvironmentHolder.current = previous }
        let model = Model(), counter = Counter(), cleanup = Counter()
        let recomposer = Recomposer()
        var graph: ViewGraph? = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph?.install(root: Reader(counter: counter) { model.first })
        let anchor = try #require(graph?.tree.root?.children.first)
        ObservableStateTracking.registerDependencyCleaner(for: ObjectIdentifier(anchor),
            ownerID: ObjectIdentifier(cleanup)) { _ in cleanup.value += 1 }
        graph = nil
        #expect(cleanup.value == 1)
        model.first = 1
        #expect(!recomposer.hasPending)
    } }
}
#endif
