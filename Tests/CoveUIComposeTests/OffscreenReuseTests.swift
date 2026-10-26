import Testing
import CoveUIRuntime
@testable import CoveUICompose

@Suite("Offscreen geometry reuse", .serialized)
struct OffscreenReuseTests: CoveUIComposeSerializedSuite {
    @Test func unchangedSnapshotsReuseGeometryAndTrimRebuildsIt() throws { try GlobalTestLock.locked {
        let state = State(wrappedValue: "First")
        struct Content: View {
            @State var text: String
            init(state: State<String>) { _text = state }
            var body: some View { Text(text).frame(width: 100, height: 30) }
        }
        let view = OffscreenView()
        let content = Content(state: state)
        view.setRootView(content)
        let first = try #require(view.render(width: 120, height: 50))
        #expect(!first.vertices.isEmpty)
        #expect(!first.atlasUpdates.isEmpty)
        let second = try #require(view.render(width: 120, height: 50))
        #expect(second.atlasUpdates.isEmpty)
        #expect(second.vertices.map(\.posX) == first.vertices.map(\.posX))
        state.wrappedValue = "Second value"
        let changed = try #require(view.render(width: 120, height: 50))
        #expect(changed.vertices.count != first.vertices.count)
        view.trimMemory()
        let restored = try #require(view.render(width: 120, height: 50))
        #expect(restored.vertices.map(\.posX) == changed.vertices.map(\.posX))
    } }
}
