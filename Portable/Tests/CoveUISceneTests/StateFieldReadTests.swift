import Foundation
import Testing
import CoveUIComposeCore
@testable import CoveUICore

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

    private struct DeferredProbe: _PrimitiveView {
        let read: () -> Void
        var layout = false
        func _makeNode() -> Node { Node() }
        func _updateNode(_ node: Node) {
            node.frame = CGRect(x: 0, y: 0, width: 64, height: 64)
            if layout { node.layoutDidUpdate = { _ in read() } }
            else { node.draw = { _, _ in read() } }
        }
    }

    @TrackStateReads
    struct Deferred: View {
        @State var model = Model()
        let bodyCounter: Counter
        let readCounter: Counter
        var layout = false
        var body: some View {
            let _ = bodyCounter.bump()
            DeferredProbe(read: { let _ = model.name; readCounter.bump() }, layout: layout)
        }
    }

    @TrackStateReads
    struct Signal: View {
        @State var revision = 0
        let counter: Counter
        var body: some View { let _ = counter.bump(); EmptyView() }
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

    @Test("Deferred custom painting tracks fields without subscribing event reads")
    func deferredPaint() throws {
        let body = Counter(), paint = Counter(), harness = Deferred(bodyCounter: body, readCounter: paint)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        NodeRenderer().render(root: try #require(graph.tree.root), into: DrawList())
        #expect(body.count == 1 && paint.count == 1)
        harness.model.age += 1
        #expect(!recomposer.hasPending)
        harness.model.name = "b"
        #expect(recomposer.hasPending)
        recomposer.commitAll()
        #expect(body.count == 2)
        NodeRenderer().render(root: try #require(graph.tree.root), into: DrawList())
        #expect(paint.count == 2)
    }

    @Test("Deferred layout callbacks retain their state owner")
    func deferredLayout() {
        let body = Counter(), layout = Counter(), harness = Deferred(bodyCounter: body, readCounter: layout, layout: true)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        graph.computeLayout(width: 64, height: 64)
        #expect(layout.count == 1)
        harness.model.age += 1
        #expect(!recomposer.hasPending)
        harness.model.name = "b"
        #expect(recomposer.hasPending)
        recomposer.commitAll()
        #expect(body.count == 2)
    }

    @Test("Unread state can still serve as an explicit invalidation signal")
    func unreadSignal() {
        let counter = Counter(), harness = Signal(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        harness.revision += 1
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

    @TrackStateReads
    struct OptionalUser: View {
        struct User: Equatable { var name: String; var age: Int }
        struct Model: Equatable { var user: User?; var other: Int }
        @State var model = Model(user: nil, other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.user?.name
            EmptyView()
        }
    }

    @Test("Optional chaining reads track \\.user?.name precisely")
    func optionalChaining() {
        let counter = Counter(), harness = OptionalUser(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.user = OptionalUser.User(name: "a", age: 1)
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        harness.model.user?.age = 5
        #expect(!recomposer.hasPending)
        harness.model.user?.name = "b"
        recomposer.commitAll()
        #expect(counter.count == 3)
        harness.model.user = nil
        recomposer.commitAll()
        #expect(counter.count == 4)
    }

    @TrackStateReads
    struct BoundDetails: View {
        struct Details: Equatable { var title: String; var count: Int }
        struct Model: Equatable { var details: Details?; var other: Int }
        @State var model = Model(details: Details(title: "a", count: 0), other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            if let d = model.details {
                let _ = d.title
            }
            EmptyView()
        }
    }

    @Test("if-let binding subscribes to the whole bound field")
    func bindingAlias() {
        let counter = Counter(), harness = BoundDetails(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        harness.model.details?.count += 1
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.details?.title = "b"
        recomposer.commitAll()
        #expect(counter.count == 3)
    }

    @TrackStateReads
    struct SignalField: View {
        struct Model: Equatable { var name: String; var age: Int }
        @State var model = Model(name: "a", age: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.name
            let _ = self._model[signal: \.age, name: "model.age"]
            EmptyView()
        }
    }

    @Test("Unread field still forces invalidation via signal")
    func signalField() {
        let counter = Counter(), harness = SignalField(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.name = "b"
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.age += 1
        recomposer.commitAll()
        #expect(counter.count == 3)
        harness.model = SignalField.Model(name: "b", age: 9)
        recomposer.commitAll()
        #expect(counter.count == 4)
        harness.model.age += 5
        recomposer.commitAll()
        #expect(counter.count == 5)
    }

    @TrackStateReads
    struct NestedOptional: View {
        struct Inner: Equatable { var value: Int }
        struct Outer: Equatable { var inner: Inner? }
        struct Model: Equatable { var outer: Outer?; var other: Int }
        @State var model = Model(outer: Outer(inner: Inner(value: 1)), other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.outer?.inner?.value
            EmptyView()
        }
    }

    @Test("Nested optional chains track \\.outer?.inner?.value precisely")
    func nestedOptional() {
        let counter = Counter(), harness = NestedOptional(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        harness.model.outer?.inner?.value = 9
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.outer = nil
        recomposer.commitAll()
        #expect(counter.count == 3)
        harness.model.outer = NestedOptional.Outer(inner: NestedOptional.Inner(value: 4))
        recomposer.commitAll()
        #expect(counter.count == 4)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
    }

    @TrackStateReads
    struct SubscriptItems: View {
        struct Model: Equatable { var items: [Int]; var other: Int }
        @State var model = Model(items: [1, 2, 3], other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.items[0]
            EmptyView()
        }
    }

    @Test("Subscript element tracking invalidates only the read index")
    func subscriptElement() {
        let counter = Counter(), harness = SubscriptItems(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.items[1] = 99
        #expect(!recomposer.hasPending)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        harness.model.items[0] = 42
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.items = [9, 9, 9]
        recomposer.commitAll()
        #expect(counter.count == 3)
    }

    @TrackStateReads
    struct SubscriptMatrix: View {
        struct Model: Equatable { var matrix: [[Int]]; var other: Int }
        @State var model = Model(matrix: [[1, 2], [3, 4]], other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.matrix[0][1]
            EmptyView()
        }
    }

    @Test("Deeply nested subscripts track the first dimension")
    func subscriptNested() {
        let counter = Counter(), harness = SubscriptMatrix(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        // Changing a different row's element must NOT invalidate.
        harness.model.matrix[1][0] = 99
        #expect(!recomposer.hasPending)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        // Changing the read row's element (or the whole row) must invalidate.
        harness.model.matrix[0][1] = 42
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.matrix[0] = [7, 7]
        recomposer.commitAll()
        #expect(counter.count == 3)
    }

    @TrackStateReads
    struct SubscriptDict: View {
        struct Model: Equatable { var dict: [String: Int]; var other: Int }
        @State var model = Model(dict: ["a": 1, "b": 2], other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.dict["a"]
            EmptyView()
        }
    }

    @Test("Dictionary element tracking invalidates only the read key")
    func subscriptDictionary() {
        let counter = Counter(), harness = SubscriptDict(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        harness.model.dict["b"] = 99
        #expect(!recomposer.hasPending)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
        harness.model.dict["a"] = 42
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.dict = ["a": 5, "b": 6]
        recomposer.commitAll()
        #expect(counter.count == 3)
    }

    /// A custom subscriptable field degrades to whole-field tracking (no element
    /// precision) but still compiles and behaves correctly.
    @TrackStateReads
    struct SubscriptCustom: View {
        struct Cell: Equatable { var v: Int }
        struct Grid: Equatable {
            var cells: [Int]
            subscript(_ i: Int) -> Int { cells[i] }
        }
        struct Model: Equatable { var grid: Grid; var other: Int }
        @State var model = Model(grid: Grid(cells: [1, 2, 3]), other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = model.grid[0]
            EmptyView()
        }
    }

    @Test("Custom subscriptable falls back to whole-field tracking")
    func subscriptCustom() {
        let counter = Counter(), harness = SubscriptCustom(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        // Whole-field granularity: any change to `grid` invalidates.
        harness.model.grid.cells[1] = 99
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.model.other += 1
        #expect(!recomposer.hasPending)
    }

    /// Each `@State` property is its own independent root: it owns a separate
    /// `StateStorage` with its own reader registry, so a write to one state
    /// cannot invalidate a dependency recorded against another.
    @TrackStateReads
    struct TwoStates: View {
        struct Model: Equatable { var items: [Int]; var other: Int }
        @State var first = Model(items: [1, 2], other: 0)
        @State var second = Model(items: [3, 4], other: 0)
        let counter: Counter
        var body: some View {
            let _ = counter.bump()
            let _ = first.items[0]
            let _ = second.items[1]
            EmptyView()
        }
    }

    @Test("Separate @State properties are independent roots")
    func twoStatesAreIndependent() {
        let counter = Counter(), harness = TwoStates(counter: counter)
        let recomposer = Recomposer(), graph = ViewGraph(tree: NodeTree(), recomposer: recomposer)
        graph.install(root: harness)
        defer { graph.install(root: EmptyView()) }
        #expect(counter.count == 1)
        // The read elements are first.items[0] and second.items[1]; writes to
        // any other index of either state must not invalidate.
        harness.first.items[1] = 99
        harness.second.items[0] = 99
        harness.first.other += 1
        harness.second.other += 1
        #expect(!recomposer.hasPending)
        #expect(counter.count == 1)
        // A write to the read element of either state invalidates.
        harness.second.items[1] = 42
        recomposer.commitAll()
        #expect(counter.count == 2)
        harness.first.items[0] = 7
        recomposer.commitAll()
        #expect(counter.count == 3)
    }

    /// Sendable box so the assertion result can cross a real thread boundary.
    private final class BoolBox: @unchecked Sendable {
        private let lock = NSLock()
        private var value = false
        func set(_ v: Bool) { lock.withLock { value = v } }
        func get() -> Bool { lock.withLock { value } }
    }

    /// The scope stack lives in native thread-local storage, implemented per
    /// platform by `CThreadLocal`. A scope pushed on one thread must be invisible
    /// on every other thread — the read path would otherwise record a view's
    /// dependencies against an unrelated scope.
    @Test("The scope stack is thread-local on every platform")
    func scopeStackIsThreadLocal() {
        let mainOwner = NSObject()
        #expect(StateReadTracking.current == nil)

        StateReadTracking.withScope(.init(id: ObjectIdentifier(mainOwner), invalidate: { _ in })) {
            #expect(StateReadTracking.current?.id == ObjectIdentifier(mainOwner))

            // A genuine OS thread: `DispatchQueue.global().sync` is allowed to
            // run its block on the calling thread, which would prove nothing.
            let done = DispatchSemaphore(value: 0)
            let result = BoolBox()
            let thread = Thread {
                result.set(StateReadTracking.current == nil)
                let bgOwner = NSObject()
                StateReadTracking.withScope(.init(id: ObjectIdentifier(bgOwner), invalidate: { _ in })) {
                    if StateReadTracking.current?.id != ObjectIdentifier(bgOwner) { result.set(false) }
                }
                if StateReadTracking.current != nil { result.set(false) }
                done.signal()
            }
            thread.start()
            done.wait()
            #expect(result.get())

            // The background work must not have disturbed this thread's scope.
            #expect(StateReadTracking.current?.id == ObjectIdentifier(mainOwner))
        }
        #expect(StateReadTracking.current == nil)
    }
}
