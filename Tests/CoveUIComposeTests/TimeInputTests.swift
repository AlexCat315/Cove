import Foundation
import Testing
import CovePlatformCore
import CoveUIRuntime
@testable import CoveUICompose

@Suite("Time input", .serialized)
struct TimeInputTests: CoveUIComposeSerializedSuite {
    private final class Box<Value> {
        var value: Value
        init(_ value: Value) { self.value = value }
        var binding: Binding<Value> {
            Binding(get: { self.value }, set: { self.value = $0 })
        }
    }

    private func nodes(_ root: Node?) -> [Node] {
        guard let root else { return [] }
        return [root] + root.children.flatMap { nodes($0) }
    }
    private func key(_ code: UInt32, keycode: UInt32 = 0) -> KeyEvent {
        KeyEvent(scancode: code, keycode: keycode, modifiers: [], isRepeat: false)
    }
    private func withScene(_ test: (ViewGraph, InteractionRegistry, FocusChain, PointerCapture) -> Void) {
        GlobalTestLock.locked {
            let oldRegistry = InteractionRegistryHolder.current, oldFocus = FocusChainHolder.current
            let oldCapture = PointerCaptureHolder.current, oldText = TextEnvironmentHolder.current
            let registry = InteractionRegistry(), focus = FocusChain(), capture = PointerCapture()
            InteractionRegistryHolder.current = registry; FocusChainHolder.current = focus
            PointerCaptureHolder.current = capture
            TextEnvironmentHolder.current = TestTextEnvironmentFactory.make()
            defer {
                InteractionRegistryHolder.current = oldRegistry; FocusChainHolder.current = oldFocus
                PointerCaptureHolder.current = oldCapture; TextEnvironmentHolder.current = oldText
            }
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            test(graph, registry, focus, capture)
            graph.install(root: EmptyView())
            AnimatorScheduler.current.tick(deltaTime: 1)
        }
    }
    private func settle(_ graph: ViewGraph) {
        for _ in 0..<5 {
            graph.recomposer.commitAll()
            graph.computeLayout(width: 800, height: 400)
            AnimatorScheduler.current.tick(deltaTime: 1)
        }
    }
    private func segments(_ graph: ViewGraph) -> [Node] {
        nodes(graph.tree.root).filter { $0.attachments["time-segment"] != nil }
    }
    private func dispatch(_ graph: ViewGraph, registry: InteractionRegistry,
                          focus: FocusChain, capture: PointerCapture, _ event: KeyEvent) {
        let dispatcher = EventDispatcher(tree: graph.tree, interactions: registry,
                                         capture: capture, focusChain: focus)
        dispatcher.dispatch(.keyDown(event))
        graph.recomposer.commitAll()
    }

    @Test("Up/Down step the focused hour, wrapping around the clock")
    func stepsHour() {
        withScene { graph, registry, focus, capture in
            let time = Box(TimeOfDay?(TimeOfDay(hour: 9, minute: 30))).binding
            graph.install(root: TimeInput(selection: time, hourCycle: .twentyFour))
            settle(graph)
            let hour = segments(graph)[0]
            focus.focus(hour)
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.arrowUp))
            #expect(time.wrappedValue?.hour == 10)
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.arrowDown))
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.arrowDown))
            #expect(time.wrappedValue?.hour == 8)
        }
    }

    @Test("Digits set the hour and advance to the minutes")
    func typeHourAdvances() {
        withScene { graph, registry, focus, capture in
            let time = Box(TimeOfDay?(TimeOfDay(hour: 9, minute: 30))).binding
            graph.install(root: TimeInput(selection: time, hourCycle: .twentyFour))
            settle(graph)
            let segs = segments(graph)
            focus.focus(segs[0])
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.digit1))
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.digit4))
            #expect(time.wrappedValue?.hour == 14)
            // After two digits, focus moves to the minute segment.
            #expect(focus.focused === segs[1])
        }
    }

    @Test("Left/Right move focus between segments")
    func arrowMovesFocus() {
        withScene { graph, registry, focus, capture in
            let time = Box(TimeOfDay?(TimeOfDay(hour: 9, minute: 30))).binding
            graph.install(root: TimeInput(selection: time, hourCycle: .twentyFour))
            settle(graph)
            let segs = segments(graph)
            focus.focus(segs[0])
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.arrowRight))
            #expect(focus.focused === segs[1])
            dispatch(graph, registry: registry, focus: focus, capture: capture, key(Scancode.arrowLeft))
            #expect(focus.focused === segs[0])
        }
    }

    @Test("12-hour period letter and Up/Down toggle AM/PM")
    func togglesPeriod() {
        withScene { graph, registry, focus, capture in
            let time = Box(TimeOfDay?(TimeOfDay(hour: 9, minute: 30))).binding
            graph.install(root: TimeInput(selection: time, hourCycle: .twelve))
            settle(graph)
            let segs = segments(graph)
            // The third segment is the period.
            focus.focus(segs[2])
            dispatch(graph, registry: registry, focus: focus, capture: capture,
                     key(Scancode.arrowUp))
            #expect(time.wrappedValue?.hour == 21)
            // Typing "a" returns to the morning (A scancode = 4).
            dispatch(graph, registry: registry, focus: focus, capture: capture,
                     key(4, keycode: 0x61))
            #expect(time.wrappedValue?.hour == 9)
        }
    }
}
