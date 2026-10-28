import Foundation
import CoveUIRuntime

/// One selection shared by every paragraph of a document, so a drag can run
/// from a heading through a list into a code block and copy as one string.
/// Transient view state: owned by the mounted root, never persisted.
public final class TextSelectionScope {
    public static let key = CompositionLocal<TextSelectionScope?>(defaultValue: nil)

    struct Position: Equatable {
        var member: ObjectIdentifier
        var offset: Int
    }

    /// A paragraph that can hold part of the selection.
    struct Member {
        weak var node: Node?
        let text: () -> String
        /// Caret offset nearest to a window-space point.
        let offsetAt: (CGPoint) -> Int
    }

    private var members: [ObjectIdentifier: Member] = [:]
    private(set) var anchor: Position?
    private(set) var focus: Position?
    /// Opens the root's context menu at a window-space point.
    var requestMenu: ((CGPoint) -> Void)?

    /// Optional hooks wired by the host so the selection toolbar can offer Look Up
    /// and Share. On Apple platforms the context menu already surfaces these
    /// natively; on Android they map to the action-mode bar's items.
    var lookup: ((String) -> Void)?
    var share: ((String) -> Void)?

    func register(_ node: Node, text: @escaping () -> String, offsetAt: @escaping (CGPoint) -> Int) {
        members[ObjectIdentifier(node)] = Member(node: node, text: text, offsetAt: offsetAt)
    }

    /// Mounted members in reading order: top to bottom, then leading to trailing.
    private var ordered: [(ObjectIdentifier, Member)] {
        members = members.filter { $0.value.node?.parent != nil }
        return members.map { ($0.key, $0.value) }.sorted { lhs, rhs in
            let a = lhs.1.node!.absoluteFrame, b = rhs.1.node!.absoluteFrame
            return abs(a.minY - b.minY) > 0.5 ? a.minY < b.minY : a.minX < b.minX
        }
    }

    var hasSelection: Bool {
        guard let anchor, let focus else { return false }
        return anchor != focus
    }

    // MARK: - Changing the selection

    func select(from anchor: Position, to focus: Position) {
        self.anchor = anchor
        self.focus = focus
        invalidate()
    }

    func extend(to focus: Position) {
        guard anchor != nil else { return select(from: focus, to: focus) }
        self.focus = focus
        invalidate()
    }

    func clear() {
        guard anchor != nil || focus != nil else { return }
        anchor = nil
        focus = nil
        invalidate()
    }

    /// Moves the endpoint of the selection that belongs to `node`, keeping the
    /// other endpoint fixed. Used by the draggable selection handles so a finger or
    /// pen can refine either edge without starting a fresh selection.
    func replaceRange(in node: Node, with range: Range<Int>) {
        guard let anchor, let focus else { return }
        let nodeID = ObjectIdentifier(node)
        if anchor.member == nodeID {
            select(from: Position(member: nodeID, offset: range.lowerBound),
                   to: Position(member: focus.member, offset: focus.offset))
        } else if focus.member == nodeID {
            select(from: Position(member: anchor.member, offset: anchor.offset),
                   to: Position(member: nodeID, offset: range.upperBound))
        }
    }

    func selectAll() {
        let list = ordered
        guard let first = list.first, let last = list.last else { return }
        select(from: Position(member: first.0, offset: 0),
               to: Position(member: last.0, offset: last.1.text().utf8.count))
    }

    func selectWord(at point: CGPoint) {
        guard let position = position(at: point), let member = members[position.member] else { return }
        let word = TextBoundaries.word(in: member.text(), at: position.offset)
        select(from: Position(member: position.member, offset: word.lowerBound),
               to: Position(member: position.member, offset: word.upperBound))
    }

    /// Selects one whole paragraph.
    func selectParagraph(_ node: Node) {
        guard let member = members[ObjectIdentifier(node)] else { return }
        select(from: Position(member: ObjectIdentifier(node), offset: 0),
               to: Position(member: ObjectIdentifier(node), offset: member.text().utf8.count))
    }

    /// The position under a window-space point; points between or beyond
    /// paragraphs snap to the nearest paragraph edge in reading order.
    func position(at point: CGPoint) -> Position? {
        let list = ordered
        guard !list.isEmpty else { return nil }
        if let hit = list.first(where: { $0.1.node!.absoluteFrame.contains(point) }) {
            return Position(member: hit.0, offset: hit.1.offsetAt(point))
        }
        // Same line band: the nearest paragraph horizontally (table rows, list markers).
        let band = list.filter { let f = $0.1.node!.absoluteFrame; return point.y >= f.minY && point.y < f.maxY }
        if let nearest = band.min(by: { distance(point, $0.1) < distance(point, $1.1) }) {
            return Position(member: nearest.0, offset: nearest.1.offsetAt(point))
        }
        if let next = list.first(where: { $0.1.node!.absoluteFrame.minY > point.y }) {
            return Position(member: next.0, offset: 0)
        }
        let last = list[list.count - 1]
        return Position(member: last.0, offset: last.1.text().utf8.count)
    }

    private func distance(_ point: CGPoint, _ member: Member) -> CGFloat {
        let frame = member.node!.absoluteFrame
        return point.x < frame.minX ? frame.minX - point.x : max(0, point.x - frame.maxX)
    }

    // MARK: - Reading the selection

    /// The selected UTF-8 range within one paragraph.
    func range(for node: Node) -> Range<Int>? {
        guard let anchor, let focus, anchor != focus else { return nil }
        let ids = ordered.map(\.0)
        guard let a = ids.firstIndex(of: anchor.member), let f = ids.firstIndex(of: focus.member),
              let index = ids.firstIndex(of: ObjectIdentifier(node)) else { return nil }
        let (start, end) = (a, anchor.offset) <= (f, focus.offset) ? (anchor, focus) : (focus, anchor)
        let (low, high) = (min(a, f), max(a, f))
        guard (low...high).contains(index) else { return nil }
        let length = members[ObjectIdentifier(node)]?.text().utf8.count ?? 0
        let lower = index == low ? start.offset : 0
        let upper = index == high ? end.offset : length
        return lower < upper ? lower..<upper : nil
    }

    /// Paragraphs on one row (table cells) join with tabs, rows with newlines.
    var selectedText: String {
        var result = ""
        var previousFrame: CGRect?
        for (_, member) in ordered {
            guard let node = member.node, let range = range(for: node) else { continue }
            let bytes = Array(member.text().utf8)
            let part = String(decoding: bytes[max(0, range.lowerBound)..<min(bytes.count, range.upperBound)], as: UTF8.self)
            if let previousFrame {
                let frame = node.absoluteFrame
                result += abs(frame.minY - previousFrame.minY) < 1 ? "\t" : "\n"
            }
            result += part
            previousFrame = node.absoluteFrame
        }
        return result
    }

    func copy() {
        guard hasSelection else { return }
        ClipboardHolder.write?(selectedText)
    }

    private func invalidate() {
        for member in members.values { member.node?.markRenderDirty(reason: .styleSet(field: "textSelection")) }
    }
}

/// Owns the selection for its content and offers Copy / Select All on
/// secondary click or a touch long press.
public struct TextSelectionRoot<Content: View>: View {
    let content: Content
    @State private var scope = TextSelectionScope()
    @State private var menuPoint: CGPoint?

    init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        let scope = scope
        scope.requestMenu = { menuPoint = $0 }
        return _ContextMenuHost(
            content: content.compositionLocal(TextSelectionScope.key, scope),
            entries: menuPoint == nil ? [] : [
                .item(MenuItem(id: "copy", title: "Copy", shortcut: Self.shortcut("C"),
                               isEnabled: scope.hasSelection) { scope.copy() }),
                .item(MenuItem(id: "lookup", title: "Look Up",
                               isEnabled: scope.hasSelection) { scope.lookup?(scope.selectedText) }),
                .item(MenuItem(id: "share", title: "Share",
                               isEnabled: scope.hasSelection) { scope.share?(scope.selectedText) }),
                .item(MenuItem(id: "selectAll", title: "Select All", shortcut: Self.shortcut("A")) { scope.selectAll() }),
            ],
            width: 200, point: menuPoint,
            onOpen: { point in
                // Without a selection, a secondary click selects the word under it.
                if !scope.hasSelection { scope.selectWord(at: point) }
                menuPoint = point
            },
            onDismiss: { menuPoint = nil })
    }

    private static func shortcut(_ key: String) -> String {
        #if os(macOS) || os(iOS)
        "⌘\(key)"
        #else
        "Ctrl+\(key)"
        #endif
    }
}

enum TextBoundaries {
    #if !canImport(Darwin)
    private static let words = try! NSRegularExpression(
        pattern: #"[\p{L}\p{M}\p{N}_]+(?:['’][\p{L}\p{M}\p{N}_]+)*"#)
    #endif

    /// The word (or, outside words, the single character) around a UTF-8 offset.
    static func word(in text: String, at offset: Int) -> Range<Int> {
        var result: Range<Int>?
        #if canImport(Darwin)
        text.enumerateSubstrings(in: text.startIndex..<text.endIndex, options: .byWords) { _, range, _, stop in
            let lower = text.utf8.distance(from: text.startIndex, to: range.lowerBound)
            let upper = text.utf8.distance(from: text.startIndex, to: range.upperBound)
            if offset >= lower && offset < upper || offset == upper && upper == text.utf8.count {
                result = lower..<upper
                stop = true
            }
        }
        #else
        for match in words.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
            guard let range = Range(match.range, in: text) else { continue }
            let lower = text.utf8.distance(from: text.startIndex, to: range.lowerBound)
            let upper = text.utf8.distance(from: text.startIndex, to: range.upperBound)
            if offset >= lower && offset < upper || offset == upper && upper == text.utf8.count {
                result = lower..<upper
                break
            }
        }
        #endif
        if let result { return result }
        var lower = 0
        for character in text {
            let upper = lower + character.utf8.count
            if offset >= lower && offset < upper { return lower..<upper }
            lower = upper
        }
        let end = max(0, min(offset, text.utf8.count))
        return end..<end
    }
}

public extension View {
    /// Opt a region of the view tree into text selection. Every `Text` and
    /// `RichText` inside shares one selection, so a drag can run from one
    /// through another and copy as one string; Copy / Select All are offered on
    /// secondary click or a touch long press (like SwiftUI's `textSelection`).
    ///
    /// Without this, each `RichText` owns its own isolated selection, and
    /// `Text` never selects at all.
    func textSelection(_ enabled: Bool = true) -> AnyView {
        enabled ? AnyView(TextSelectionRoot { self }) : AnyView(self)
    }
}
