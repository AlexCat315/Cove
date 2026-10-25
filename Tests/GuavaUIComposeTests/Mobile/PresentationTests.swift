import Foundation
import Testing
import GuavaUIRuntime
@testable import GuavaUICompose

@Suite("Mobile presentation", .serialized)
@MainActor
struct MobilePresentationTests: GuavaUIComposeSerializedSuite {
    @Test("Touch button geometry includes typography and chrome, and pointer changes retain the action")
    func buttons() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.button); scene.install(fixture)
            let button = try #require(scene.labelled("Action", role: .button))
            #expect(button.frame.height >= 44)
            let label = try #require(scene.nodes.first { $0.layoutNode?.textInputs?.text == "Action" })
            let designedSize = try #require(label.layoutNode?.textMeasure?.key.font.size)
            #expect(InteractiveScene.descendants(button).contains { $0 !== button && $0.cornerRadius >= 12 })
            scene.click(button); #expect(fixture.calls == 1)
            fixture.surface.input = [.pointer, .keyboard]; scene.settle()
            #expect(scene.labelled("Action", role: .button) === button)
            #expect(button.frame.height == 32)
            #expect(label.layoutNode?.textMeasure?.key.font.size == designedSize)
            fixture.surface.input = [.touch]; fixture.controlSize = .large; scene.settle()
            #expect(button.frame.height >= 56)
            fixture.surface.textScale = .init(2); scene.settle()
            #expect(button.frame.height >= 56)
            #expect(label.layoutNode?.textMeasure?.key.font.size == designedSize * 2)
        }
    }

    @Test("Phone navigation uses equal bottom items with badges in portrait and landscape")
    func tabs() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.tabs); scene.install(fixture)
            let bar = try #require(scene.named("mobile-tab-bar"))
            let home = try #require(scene.labelled("Home", role: .tab))
            let search = try #require(scene.labelled("Search", role: .tab))
            #expect(abs(home.frame.width - search.frame.width) <= 1)
            #expect(home.frame.height >= 52 && home.accessibility?.value == "3")
            #expect(bar.absoluteFrame.maxY == 820)
            scene.click(search); #expect(fixture.selection == 1)
            scene.click(try #require(scene.labelled("Unavailable", role: .tab)))
            #expect(fixture.selection == 1)
            scene.focus.focus(home); scene.key(Scancode.arrowRight)
            #expect(fixture.selection == 1)
            scene.width = 844; scene.height = 390
            fixture.surface.viewport.width = 844; fixture.surface.viewport.height = 390
            scene.settle(); #expect(scene.named("mobile-tab-bar") != nil)
            #expect(try #require(scene.named("mobile-tab-bar")).absoluteFrame.maxY == 366)
            fixture.documentTabs = true; scene.settle()
            #expect(scene.named("mobile-tab-bar") == nil)
            #expect(scene.labelled("Close Home", role: .button) != nil)
        }
    }

    @Test("Overflow stays reachable through More and promotes the selected destination")
    func tabOverflow() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.tabs); fixture.extraTabs = true; scene.install(fixture)
            scene.click(try #require(scene.labelled("More tabs", role: .button)))
            scene.click(try #require(scene.labelled("Page 7", role: .button)))
            #expect(fixture.selection == 7)
            let active = try #require(scene.labelled("Page 7", role: .tab))
            #expect(active.accessibility?.state.isSelected == true)
            #expect(scene.nodes.filter { $0.accessibility?.role == .tab }.count == 4)
            #expect(active.absoluteFrame.minX >= 0 && active.absoluteFrame.maxX <= 390)
        }
    }

    @Test("Native nonlinear text scaling keeps a table action inside its authored row")
    func nonlinearTableControls() {
        InteractiveScene.run(width: 390, height: 500) { scene in
            var surface = UIEnvironment(); surface.input = [.touch]
            surface.textScale = .init(samples: [.init(size: 16, scaledSize: 24),
                                               .init(size: 22, scaledSize: 30), .init(size: 36, scaledSize: 42)])
            scene.install(SurfaceRoot(surface) {
                Table([1, 2], id: \.self, columns: [TableColumn("action", "Action") { _ in
                    Button("Open") {}.accessibilityLabel("Table action")
                }])
            })
            let buttons = scene.nodes.filter { $0.accessibility?.label == "Table action" && $0.isFocusable }
            #expect(buttons.count == 2)
            for button in buttons {
                var parent = button.parent
                while let current = parent {
                    if current.accessibility?.role == .cell {
                        #expect(current.absoluteFrame.contains(button.absoluteFrame)); break
                    }
                    parent = current.parent
                }
            }
        }
    }

    @Test("Touch lists truncate in their proposal and scale two-line rows with the system font")
    func lists() throws {
        try InteractiveScene.run(width: 240, height: 500) { scene in
            let fixture = MobileFixture(.list)
            fixture.surface.viewport.width = 240; fixture.surface.viewport.height = 500
            scene.install(fixture)
            let rows = scene.nodes.filter { $0.accessibility?.role == .listItem }
            #expect(!rows.isEmpty && rows.allSatisfy { $0.frame.height == 56 })
            let title = try #require(scene.nodes.first { $0.layoutNode?.textInputs?.text.hasPrefix("Long title 0") == true })
            #expect(title.absoluteFrame.maxX <= 240)
            #expect(title.layoutNode?.textMeasure?.key.text.hasSuffix("…") == true)
            fixture.surface.textScale = .init(2); scene.settle()
            let large = scene.nodes.filter { $0.accessibility?.role == .listItem }
            #expect(large.allSatisfy { $0.frame.height == 112 })
            #expect(scene.nodes.contains { $0.layoutNode?.textInputs?.text == "Secondary detail" })
            fixture.surface.input = [.pointer, .keyboard]; scene.settle()
            #expect(scene.nodes.filter { $0.accessibility?.role == .listItem }.allSatisfy { $0.frame.height == 60 })
        }
    }

    @Test("Dragging a touch list scrolls and cancels the pending row selection")
    func listDrag() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.list); scene.install(fixture)
            let rows = scene.nodes.filter { $0.accessibility?.role == .listItem }
            let row = try #require(rows.dropFirst(2).first)
            let frame = row.absoluteFrame
            let x = Float(frame.midX), y = Float(frame.midY)
            let dispatcher = scene.dispatcher
            dispatcher.dispatch(.mouseButtonDown(.init(button: .left, x: x, y: y, clicks: 1)))
            dispatcher.dispatch(.mouseMotion(.init(x: x, y: y - 80, deltaX: 0, deltaY: -80)))
            dispatcher.dispatch(.mouseButtonUp(.init(button: .left, x: x, y: y - 80, clicks: 1)))
            scene.settle()
            #expect(fixture.selectedRow == 0)
            #expect(scene.nodes.contains { $0.contentOffset.y > 0 })
        }
    }

    @Test("Touch menus drill into branches in one modal and dismiss after activation")
    func menus() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.menu); scene.install(fixture)
            let trigger = try #require(scene.labelled("Commands", role: .button))
            scene.click(trigger)
            #expect(scene.named("touch-overlay-sheet") != nil)
            #expect(scene.focus.hasModalScope)
            let more = try #require(scene.labelled("More", role: .button))
            for title in ["Open", "More", "Delete", "Unavailable"] {
                let label = try #require(scene.nodes.first { $0.layoutNode?.textInputs?.text == title })
                #expect(label.frame.width > 0 && label.frame.height > 0, "Visible menu label: \(title)")
            }
            scene.click(more)
            #expect(scene.named("touch-menu-back") != nil)
            scene.click(try #require(scene.labelled("Export", role: .button)))
            #expect(fixture.calls == 10 && scene.named("touch-overlay-sheet") == nil)
            #expect(scene.focus.focused === trigger)
        }
    }

    @Test("Popover sheets avoid the keyboard and restore anchored presentation for pointer input")
    func popovers() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.popover); fixture.surface.keyboard = .init(height: 300)
            scene.install(fixture)
            scene.click(try #require(scene.labelled("Details", role: .button)))
            let commit = try #require(scene.labelled("Commit", role: .button))
            #expect(commit.absoluteFrame.maxY <= 544)
            scene.click(commit); #expect(fixture.calls == 1 && !fixture.presented)
            fixture.surface.idiom = .desktop; fixture.surface.input = [.pointer, .keyboard]
            fixture.surface.keyboard = .init(); scene.settle()
            scene.click(try #require(scene.labelled("Details", role: .button)))
            #expect(fixture.presented && scene.named("touch-overlay-sheet") == nil)
            #expect(!scene.portal.entries.isEmpty)
        }
    }

    @Test("A hold cancels the child tap; short taps and motion do not open a context menu")
    func contextHold() throws {
        try InteractiveScene.run(width: 390, height: 844) { scene in
            let fixture = MobileFixture(.context); scene.install(fixture)
            let target = try #require(scene.labelled("Target", role: .button))
            let frame = target.absoluteFrame
            let event = MouseButtonEvent(button: .left, x: Float(frame.midX), y: Float(frame.midY), clicks: 1)
            let dispatcher = scene.dispatcher
            dispatcher.dispatch(.mouseButtonDown(event)); scene.settle(ticks: 1, delta: 0.2)
            #expect(scene.named("touch-overlay-sheet") == nil)
            dispatcher.dispatch(.mouseButtonUp(event)); scene.settle()
            #expect(fixture.calls == 1_000_000)
            dispatcher.dispatch(.mouseButtonDown(event))
            dispatcher.dispatch(.mouseMotion(.init(x: event.x + 30, y: event.y, deltaX: 30, deltaY: 0)))
            scene.settle(ticks: 1, delta: 1)
            #expect(scene.named("touch-overlay-sheet") == nil)
            dispatcher.dispatch(.mouseButtonUp(event)); scene.settle()
            let before = fixture.calls
            dispatcher.dispatch(.mouseButtonDown(event)); scene.settle(ticks: 1, delta: 0.6)
            #expect(scene.named("touch-overlay-sheet") != nil)
            dispatcher.dispatch(.mouseButtonUp(event)); scene.settle()
            #expect(fixture.calls == before + 10_000)
        }
    }
}
