import Foundation
import Testing
import CoveUIRuntime
@testable import CoveUICompose

@Suite("Route isolation and virtual preload", .serialized)
@MainActor
struct RouteAndPreloadTests: CoveUIComposeSerializedSuite {
    final class Counts {
        var shell = 0
        var pages: [Int] = []
        var rows: [Int: Int] = [:]
        var child = 0
    }

    @TrackStateReads
    struct Routed: View {
        @State var selection = 0
        let counts: Counts
        var body: some View {
            let _ = { counts.shell += 1 }()
            RouteOutlet(selection: $selection) { route in
                let _ = { counts.pages.append(route) }()
                Text("Page \(route)").debugName("route-\(route)")
            }
        }
    }

    @Test("Only the active route is created and selecting another route leaves the shell untouched")
    func routeIsolation() throws {
        try InteractiveScene.run { scene in
            let counts = Counts()
            let routed = Routed(counts: counts)
            scene.install(routed)
            let before = counts.shell
            #expect(counts.pages == [0])
            let first = try #require(scene.named("route-0"))
            routed.$selection.wrappedValue = 1; scene.settle()
            #expect(counts.shell == before)
            #expect(counts.pages == [0, 1])
            #expect(scene.named("route-0") == nil && scene.named("route-1") != nil)
            routed.$selection.wrappedValue = 0; scene.settle()
            #expect(scene.named("route-0") !== first)
        }
    }

    private struct MemoHarness: View {
        @State var parent = 0
        @State var input = 1
        @State var scale: Float = 1
        let counts: Counts
        var body: some View {
            MemoizedView(key: input) { MemoChild(input: input, counts: counts) }.textScaler(.init(scale))
        }
    }
    private struct MemoChild: View {
        let input: Int
        let counts: Counts
        @State private var local = 0
        var body: some View {
            let _ = { counts.child += 1 }()
            Button("\(input):\(local)") { local += 1 }.debugName("memo-child")
        }
    }

    @Test("Explicit memoization skips equal parent updates but preserves local state invalidation")
    func memoization() throws {
        try InteractiveScene.run { scene in
            let counts = Counts()
            let harness = MemoHarness(counts: counts)
            scene.install(harness)
            let before = counts.child
            harness.$parent.wrappedValue += 1; scene.settle()
            #expect(counts.child == before)
            scene.click(try #require(scene.named("memo-child")))
            #expect(counts.child > before)
            harness.$input.wrappedValue = 2; scene.settle()
            #expect(scene.labelled("2:1") != nil)
            let beforeEnvironment = counts.child
            harness.$scale.wrappedValue = 2; scene.settle()
            #expect(counts.child > beforeEnvironment)
        }
    }

    @Test("Programmatic sidebar focus survives a local hover update without changing selection")
    func sidebarFocusWithoutSelection() throws {
        try InteractiveScene.run(width: 260, height: 400) { scene in
            scene.install(Sidebar(selection: .constant(0), sections: [
                SidebarSection("main", items: (0..<6).map { SidebarItem($0, "Item \($0)") })
            ]).frame(width: 260, height: 400))
            let row = try #require(scene.named("sidebar-item-3"))
            scene.focus.focus(row); scene.settle()
            scene.dispatcher.dispatch(.mouseMotion(MouseMotionEvent(x: Float(row.absoluteFrame.midX),
                y: Float(row.absoluteFrame.midY), deltaX: 0, deltaY: 0)))
            scene.settle()
            #expect(row.isTabStop)
            #expect(row.accessibility?.state.isSelected == false)
            #expect(scene.all { $0.attachments["__roving.item"] != nil && $0.isTabStop }.count == 1)
        }
    }

    @Test("Refill thresholds leave a buffer at both ends and always respond to viewport size changes")
    func windowMath() {
        let window = VirtualWindow(), size = CGSize(width: 200, height: 1_000)
        window.record(range: 180..<320, visible: 200..<300, count: 10_000, size: size)
        #expect(!window.needsRefill(visible: 200..<300, size: size))
        #expect(!window.needsRefill(visible: 205..<305, size: size))
        #expect(window.needsRefill(visible: 211..<311, size: size))
        #expect(window.needsRefill(visible: 189..<289, size: size))
        #expect(window.needsRefill(visible: 200..<300, size: CGSize(width: 201, height: 1_000)))
        let prepared = VirtualWindow.preparation(offset: 2_000, viewport: 1_000, preload: 0.3)
        #expect(abs(prepared.offset - 1_850) < 0.01 && abs(prepared.height - 1_300) < 0.01)
        #expect(VirtualWindow.preparation(offset: 2_000, viewport: 1_000, preload: 0).height == 1_000)
        window.record(range: 0..<0, visible: 0..<0, count: 0, size: size)
        #expect(!window.needsRefill(visible: 0..<0, size: size))
        let hidden = CGSize(width: 200, height: 0)
        window.record(range: 0..<0, visible: 0..<0, count: 10, size: hidden)
        #expect(!window.needsRefill(visible: 0..<0, size: hidden))
    }

    @Test("Buffered motion does not rebuild rows and a refill only constructs entering rows", arguments: [0, 1, 2])
    func bufferedScroll(kind: Int) throws {
        try InteractiveScene.run(width: 200, height: 1_000) { scene in
            let counts = Counts()
            func row(_ index: Int) -> some View {
                counts.rows[index, default: 0] += 1
                return Box {}.frame(height: 10).debugName("preload-row-\(index)")
            }
            let stack: AnyView
            switch kind {
            case 0: stack = AnyView(VirtualStack(0..<10_000, id: \.self, rowHeight: 10, overscan: 0, content: row))
            case 1: stack = AnyView(VirtualStack(0..<10_000, id: \.self,
                rowHeight: { _, _, _ in 10 }, overscan: 0, content: row))
            default: stack = AnyView(VirtualStack(0..<10_000, id: \.self, estimatedRowHeight: 10, overscan: 0, content: row))
            }
            scene.install(stack.frame(width: 200, height: 1_000))
            let scroll = try #require(scene.first { $0.isScrollViewport })
            func wheel(_ distance: Float) {
                scene.dispatcher.dispatch(.mouseWheel(MouseWheelEvent(x: 0, y: -distance,
                    mouseX: 50, mouseY: 50, input: .init(unit: .points, device: .continuous))))
                scene.settle()
            }
            wheel(2_000)
            let before = counts.rows, previousOffset = scroll.contentOffset.y
            let mounted = scene.all { ($0.attachments[LayoutDebugAttachmentKey.debugName] as? String)?.hasPrefix("preload-row-") == true }
            #expect(mounted.count >= 130 && mounted.count <= 136)
            wheel(1)
            #expect(scroll.contentOffset.y == previousOffset + 1)
            #expect(counts.rows == before)
            wheel(100)
            let kept = Set(before.keys).intersection(counts.rows.keys).filter { $0 >= 200 && $0 < 300 }
            #expect(!kept.isEmpty)
            #expect(kept.allSatisfy { counts.rows[$0] == before[$0] })
            #expect(counts.rows.keys.contains { $0 > (before.keys.max() ?? 0) })
            // Several wheel packets may arrive before composition. Returning
            // to the old window must not consume a later refill at the same offset.
            func dispatch(_ distance: Float) {
                scene.dispatcher.dispatch(.mouseWheel(MouseWheelEvent(x: 0, y: -distance,
                    mouseX: 50, mouseY: 50, input: .init(unit: .points, device: .continuous))))
            }
            dispatch(2_000); dispatch(-2_000); scene.settle()
            dispatch(2_000); scene.settle()
            let firstVisible = Int(scroll.contentOffset.y / 10)
            #expect(scene.named("preload-row-\(firstVisible + 1)") != nil)
        }
    }
}
