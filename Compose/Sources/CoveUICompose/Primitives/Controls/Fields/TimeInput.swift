import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// The hour cycle a `TimeInput` displays.
public enum TimeInputHourCycle: Sendable, Equatable { case twelve, twentyFour }

/// A segmented time field, like AppKit's date picker: hours, minutes and (in a
/// 12-hour cycle) AM/PM each take the focus. Up/Down step the focused segment,
/// digits set it and advance to the next, Left/Right move between segments,
/// and the first letter of AM/PM chooses the half of the day.
///
/// ```swift
/// TimeInput(selection: $time, hourCycle: .twelve)
/// ```
public struct TimeInput: View {
    public let selection: Binding<TimeOfDay?>
    public let hourCycle: TimeInputHourCycle
    public let isEnabled: Bool
    public let accessibilityLabel: String?

    @State private var hourBuffer = ""
    @State private var minuteBuffer = ""

    public init(selection: Binding<TimeOfDay?>,
                hourCycle: TimeInputHourCycle = .twentyFour,
                isEnabled: Bool = true,
                accessibilityLabel: String? = nil) {
        self.selection = selection
        self.hourCycle = hourCycle
        self.isEnabled = isEnabled
        self.accessibilityLabel = accessibilityLabel
    }

    private var time: TimeOfDay { selection.wrappedValue ?? TimeOfDay() }
    private var twelve: Bool { hourCycle == .twelve }

    private var hourText: String {
        twelve
            ? String(format: "%d", (time.hour + 11) % 12 + 1)
            : String(format: "%02d", time.hour)
    }
    private var minuteText: String { String(format: "%02d", time.minute) }
    private var periodText: String { time.hour < 12 ? "AM" : "PM" }

    private func set(hour: Int, minute: Int) {
        let next = TimeOfDay(hour: ((hour % 24) + 24) % 24, minute: ((minute % 60) + 60) % 60)
        if next != time { selection.wrappedValue = next }
    }

    private func stepHour(_ delta: Int) {
        if twelve {
            // Step within the same half of the day, keeping AM/PM.
            let half = time.hour / 12 * 12
            let within = (time.hour % 12 + delta + 12) % 12
            set(hour: half + (within == 0 ? 12 : within), minute: time.minute)
        } else {
            set(hour: time.hour + delta, minute: time.minute)
        }
    }

    private func stepMinute(_ delta: Int) {
        set(hour: time.hour, minute: time.minute + delta)
    }

    private func togglePeriod() {
        set(hour: time.hour + 12, minute: time.minute)
    }

    /// Types a digit into the hour segment; returns true when focus should
    /// advance to the minutes.
    private func typeHour(_ digit: Int) -> Bool {
        let top = twelve ? 13 : 24
        if hourBuffer.isEmpty {
            hourBuffer = String(digit)
            if twelve {
                if digit == 0 { return false }
                set(hour: time.hour / 12 * 12 + digit % 12, minute: time.minute)
            } else {
                set(hour: digit, minute: time.minute)
            }
            return digit * 10 >= top
        }
        if let first = Int(hourBuffer), let n = Int("\(first)\(digit)"), n < top {
            if twelve {
                set(hour: time.hour / 12 * 12 + (n == 0 ? 12 : n % 12), minute: time.minute)
            } else {
                set(hour: n, minute: time.minute)
            }
        }
        hourBuffer = ""
        return true
    }

    /// Types a digit into the minute segment; returns true when the segment is
    /// complete (focus may leave).
    private func typeMinute(_ digit: Int) -> Bool {
        if minuteBuffer.isEmpty {
            minuteBuffer = String(digit)
            set(hour: time.hour, minute: digit)
            return digit * 10 >= 60
        }
        if let first = Int(minuteBuffer), let n = Int("\(first)\(digit)"), n < 60 {
            set(hour: time.hour, minute: n)
        }
        minuteBuffer = ""
        return true
    }

    public var body: some View {
        Row(alignment: .center, spacing: 2) {
            TimeSegment(kind: .hour, text: hourText, isEnabled: isEnabled,
                        onFocusChange: { focused in if !focused { hourBuffer = "" } },
                        onStep: stepHour,
                        onType: typeHour,
                        onPeriodLetter: nil)
            Text(":").font(.body).foregroundColor(.onSurfaceMuted)
            TimeSegment(kind: .minute, text: minuteText, isEnabled: isEnabled,
                        onFocusChange: { focused in if !focused { minuteBuffer = "" } },
                        onStep: stepMinute,
                        onType: typeMinute,
                        onPeriodLetter: nil)
            if twelve {
                TimeSegment(kind: .period, text: periodText, isEnabled: isEnabled,
                            onFocusChange: { _ in },
                            onStep: { _ in togglePeriod() },
                            onType: { _ in false },
                            onPeriodLetter: { isPM in
                                let currentlyPM = time.hour >= 12
                                if isPM != currentlyPM { togglePeriod() }
                            })
            }
        }
        .padding(horizontal: 8, vertical: 5)
        .background(.surface)
        .cornerRadius(.inputRadius)
        .border(.border, width: 1)
        .accessibility {
            $0.role = .group
            if let accessibilityLabel { $0.label = accessibilityLabel }
        }
    }
}

private enum TimeSegmentKind { case hour, minute, period }

/// A single focusable segment of a `TimeInput`.
private struct TimeSegment: View {
    let kind: TimeSegmentKind
    let text: String
    let isEnabled: Bool
    let onFocusChange: (Bool) -> Void
    let onStep: (Int) -> Void
    let onType: (Int) -> Bool
    let onPeriodLetter: ((Bool) -> Void)?

    @State private var isFocused = false

    var body: some View {
        let label = Text(text).font(.body)
            .foregroundColor(isFocused ? .onAccent : .onSurface)
            .padding(horizontal: 6, vertical: 3)
            .background(isFocused ? .accent : .clear)
            .cornerRadius(4)
        TimeSegmentHost(kind: kind, isEnabled: isEnabled,
                        onFocusChange: { focused in
                            if isFocused != focused { isFocused = focused }
                            onFocusChange(focused)
                        },
                        onStep: onStep, onType: onType, onPeriodLetter: onPeriodLetter,
                        label: AnyView(label))
    }
}

/// The primitive node behind a time segment: focusable, hit-testable, with
/// keyboard handling and the styled label as its child.
private struct TimeSegmentHost: _PrimitiveView {
    let kind: TimeSegmentKind
    let isEnabled: Bool
    let onFocusChange: (Bool) -> Void
    let onStep: (Int) -> Void
    let onType: (Int) -> Bool
    let onPeriodLetter: ((Bool) -> Void)?
    let label: AnyView

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = true
        node.isFocusable = true
        node.attachments["time-segment"] = true
        return node
    }

    func _makeLayoutNode() -> LayoutNode? {
        let l = LayoutNode()
        l.flexDirection = .row
        l.alignItems = .center
        l.justifyContent = .center
        return l
    }

    func _updateNode(_ node: Node) {
        node.isFocusable = isEnabled
        node.attachments[TextInputAttachmentKey.focusChangeHandler] = { [onFocusChange] (focused: Bool) in
            onFocusChange(focused)
        } as TextInputFocusChangeHandler
        node.cursor = isEnabled ? .pointer : .notAllowed
        node.accessibility = AccessibilitySemantics(.button) { semantics in
            switch kind {
            case .hour: semantics.label = "Hours"
            case .minute: semantics.label = "Minutes"
            case .period: semantics.label = "AM/PM"
            }
            semantics.state.isEnabled = isEnabled
            semantics.combinesChildren = true
        }
        InteractionRegistryHolder.current?.setPointer(node) { event, phase, eventPhase in
            guard eventPhase != .capture, event.button == .left, phase == .down else { return .ignored }
            FocusChainHolder.current?.focus(node)
            return .handled
        }
        InteractionRegistryHolder.current?.setKey(node) { event, phase in
            guard phase != .capture, isEnabled else { return .ignored }
            switch event.scancode {
            case Scancode.arrowUp:
                onStep(1); return .handled
            case Scancode.arrowDown:
                onStep(-1); return .handled
            case Scancode.arrowLeft:
                return focusNeighbor(node, offset: -1) ? .handled : .ignored
            case Scancode.arrowRight:
                return focusNeighbor(node, offset: 1) ? .handled : .ignored
            default:
                if let digit = Self.digit(event.scancode) {
                    let advanced = onType(digit)
                    if advanced { _ = focusNeighbor(node, offset: 1) }
                    return .handled
                }
                if kind == .period, let letter = Self.letter(event.keycode) {
                    let lower = letter.lowercased()
                    if lower == "a" { onPeriodLetter?(false) }
                    else if lower == "p" { onPeriodLetter?(true) }
                    return .handled
                }
                return .ignored
            }
        }
    }

    var _children: [any View] { [label] }

    private static func digit(_ scancode: UInt32) -> Int? {
        switch scancode {
        case Scancode.digit0: return 0
        case Scancode.digit1: return 1
        case Scancode.digit2: return 2
        case Scancode.digit3: return 3
        case Scancode.digit4: return 4
        case Scancode.digit5: return 5
        case Scancode.digit6: return 6
        case Scancode.digit7: return 7
        case Scancode.digit8: return 8
        case Scancode.digit9: return 9
        default: return nil
        }
    }

    private static func letter(_ keycode: UInt32) -> String? {
        // keycode is the SDL keycode; map the A and P letters directly.
        let a: UInt32 = 0x61, p: UInt32 = 0x70
        switch keycode {
        case a: return "a"
        case p: return "p"
        default: return nil
        }
    }

    /// Focuses the next/previous time segment in the nearest group.
    private func focusNeighbor(_ node: Node, offset: Int) -> Bool {
        guard let host = groupRoot(for: node), let chain = FocusChainHolder.current else { return false }
        let segments = collectSegments(in: host)
        guard let index = segments.firstIndex(where: { $0 === node }) else { return false }
        let next = index + offset
        guard segments.indices.contains(next) else { return false }
        chain.focus(segments[next])
        return true
    }

    /// The lowest ancestor that contains at least two segments (the segment
    /// nodes may sit inside per-row layout wrappers).
    private func groupRoot(for node: Node) -> Node? {
        var current: Node? = node.parent
        while let n = current {
            if countSegments(in: n) >= 2 { return n }
            current = n.parent
        }
        return nil
    }

    private func countSegments(in host: Node) -> Int {
        var count = 0
        func walk(_ n: Node) {
            if n.attachments["time-segment"] != nil { count += 1 }
            for child in n.children { walk(child) }
        }
        walk(host)
        return count
    }

    private func collectSegments(in host: Node) -> [Node] {
        var result: [Node] = []
        func walk(_ n: Node) {
            if n.attachments["time-segment"] != nil { result.append(n) }
            for child in n.children { walk(child) }
        }
        walk(host)
        return result
    }
}
