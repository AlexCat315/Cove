import Testing
import CoveUIComposeCore

@Suite("Inferred value state field reads", .serialized)
@MainActor
struct StateFieldReadTests {
    final class Counter { var count = 0; func bump() { count += 1 } }
    struct Model: Equatable {
        struct Details: Equatable { var title = "a"; var other = 0 }
        var name = "a"
        var age = 0
        var details = Details()
    }

    final class Box { var text = "a" }
    struct OpaqueField: Equatable {
        let box = Box()
        static func == (lhs: Self, rhs: Self) -> Bool { lhs.box.text == rhs.box.text }
    }
    struct OpaqueModel { var field = OpaqueField() }
    @TrackStateReads
    struct Opaque: View {
        @State var model = OpaqueModel()
        let counter: Counter
        var body: some View {
            let _ = model.field
            let _ = counter.bump()
            EmptyView()
        }
    }

    @TrackStateReads
    struct Selective: View {
        @State var model = Model()
        @State var useName = true
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            if useName {
                let _ = model.name.uppercased()
                EmptyView()
            } else {
                let _ = model.details.title
                EmptyView()
            }
        }
    }

    @TrackStateReads
    struct Whole: View {
        @State var model = Model()
        let counter: Counter
        var body: some View {
            let snapshot = model
            let _ = counter.bump()
            let _ = snapshot.name
            EmptyView()
        }
    }

    @TrackStateReads
    struct Shadowed: View {
        @State var model = Model()
        let counter: Counter
        var body: some View {
            let model = Model()
            let closure: (Model) -> String = { model in model.name }
            let _ = closure(model)
            let _ = self.model.name
            let _ = counter.bump()
            EmptyView()
        }
    }

    @TrackStateReads
    struct Actions: View {
        @State var model = Model()
        let counter: Counter
        let expose: (@escaping () -> Void) -> Void
        var body: some View {
            let _ = counter.bump()
            let _ = model.name
            let _ = expose { model.age += 1; model.name.append("x") }
            EmptyView()
        }
    }

    @Test("Only a changed read field invalidates; conditional reads replace old dependencies")
    func selectiveAndConditional() {
        let counter = Counter(), harness = Selective(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.age += 1
        #expect(!recomposer.hasPending)
        harness.model.name = "b"
        #expect(recomposer.hasPending)
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.name = "b"
        #expect(!recomposer.hasPending)
        harness.useName = false
        recomposer.commitAll()
        #expect(counter.count == 3)
        harness.model.name = "c"
        harness.model.details.other += 1
        #expect(!recomposer.hasPending)
        harness.model.details.title = "b"
        recomposer.commitAll()
        #expect(counter.count == 4)
        graph.install(root: EmptyView())
        harness.model.details.title = "c"
        #expect(!recomposer.hasPending)
    }

    @Test("An Equatable value containing references cannot hide a mutation")
    func opaqueFallback() {
        let counter = Counter(), harness = Opaque(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        let replacement = harness.model
        replacement.field.box.text = "b"
        harness.model = replacement
        #expect(recomposer.hasPending)
        recomposer.commitAll()
        #expect(counter.count == 2)
    }

    @Test("Whole value reads retain conservative invalidation")
    func wholeValue() {
        let counter = Counter(), harness = Whole(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        harness.model.age += 1
        #expect(recomposer.hasPending)
        recomposer.commitAll()
        #expect(counter.count == 2)
    }

    @Test("Local and closure shadowing preserve Swift name lookup")
    func shadowing() {
        let counter = Counter(), harness = Shadowed(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        harness.model.age += 1
        #expect(!recomposer.hasPending)
        harness.model.name = "b"
        recomposer.commitAll()
        #expect(counter.count == 2)
    }

    @Test("Generated writable field paths retain event closure mutations")
    func actionWrites() {
        var action: (() -> Void)?
        let counter = Counter(), harness = Actions(counter: counter, expose: { action = $0 })
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        action?()
        recomposer.commitAll()
        #expect(harness.model.age == 1 && harness.model.name == "ax")
        #expect(counter.count == 2)
    }
}
