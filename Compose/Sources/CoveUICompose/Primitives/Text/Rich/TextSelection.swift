import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging
import CoveUIScene

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
    /// natively; on Android they map to the action-mode bar's items. Both menu
    /// entries stay hidden until a hook is set, so a stock selection never offers
    /// a row that does nothing.
    public var lookup: ((String) -> Void)?
    public var share: ((String) -> Void)?

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

    /// Whether anything is actually selected (an anchor with no focus is a caret,
    /// not a selection).
    public var hasSelection: Bool {
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

    public func clear() {
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

    public func selectAll() {
        let list = ordered
        guard let first = list.first, let last = list.last else { return }
        select(from: Position(member: first.0, offset: 0),
               to: Position(member: last.0, offset: last.1.text().utf8.count))
    }

    // MARK: - Keyboard extension

    /// Moves the focus by whole grapheme clusters, keeping the anchor fixed, and
    /// crosses paragraph boundaries in reading order. Returns false when there is
    /// no caret to extend from, so a key handler can pass the event on.
    @discardableResult
    func extendFocus(by clusters: Int) -> Bool {
        guard clusters != 0 else { return false }
        let list = ordered
        guard !list.isEmpty, let focus,
              let start = list.firstIndex(where: { $0.0 == focus.member }) else { return false }
        var index = start
        var offset = focus.offset
        var remaining = clusters
        while remaining != 0 {
            let step = remaining > 0 ? 1 : -1
            let text = list[index].1.text()
            if let next = Self.stepped(text, from: offset, by: step) {
                offset = next
            } else {
                // Step over the paragraph edge into the neighbour, when there is one.
                let neighbour = index + step
                guard list.indices.contains(neighbour) else { break }
                index = neighbour
                offset = step > 0 ? 0 : list[neighbour].1.text().utf8.count
            }
            remaining -= step
        }
        let target = list[index]
        extend(to: Position(member: target.0, offset: offset))
        return true
    }

    /// Extends the focus to the start or end of the paragraph holding it.
    @discardableResult
    func extendFocusToEdge(start: Bool) -> Bool {
        guard let focus, let member = members[focus.member] else { return false }
        extend(to: Position(member: focus.member, offset: start ? 0 : member.text().utf8.count))
        return true
    }

    /// The next caret offset one grapheme away, or nil at the paragraph edge.
    private static func stepped(_ text: String, from offset: Int, by delta: Int) -> Int? {
        guard let index = text.utf8.index(text.startIndex, offsetBy: offset, limitedBy: text.endIndex) else {
            return nil
        }
        if delta > 0 {
            guard index < text.endIndex else { return nil }
            return text.utf8.distance(from: text.startIndex, to: text.index(after: index))
        }
        guard index > text.startIndex else { return nil }
        return text.utf8.distance(from: text.startIndex, to: text.index(before: index))
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

    /// Whether a window-space point lands on a paragraph this scope owns. The
    /// region uses it to ignore secondary clicks and long presses anywhere else,
    /// so a selection region spanning a whole page (or the whole app) never
    /// shadows the context menus of the controls inside it.
    func textUnder(_ point: CGPoint) -> Bool {
        ordered.contains { $0.1.node?.absoluteFrame.contains(point) ?? false }
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
    public var selectedText: String {
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

    public func copy() {
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

    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        let scope = scope
        scope.requestMenu = { menuPoint = $0 }
        let entries = menuPoint == nil ? [] : Self.entries(scope)
        // `SurfaceReader` carries no layout node, so the host below stays the
        // layout root and the content keeps the height the window offers.
        return SurfaceReader { surface in
            _ContextMenuHost(
                content: content.compositionLocal(TextSelectionScope.key, scope),
                entries: entries,
                width: 200, point: menuPoint,
                onOpen: { point in
                    // Without a selection, a secondary click selects the word under it.
                    if !scope.hasSelection { scope.selectWord(at: point) }
                    menuPoint = point
                },
                onDismiss: { menuPoint = nil },
                // A region can span a whole page or the whole app, so it takes only
                // the presses that land on its own text and leaves every other click
                // to the control underneath.
                isInterested: { scope.textUnder($0) },
                // A touch screen has no secondary click, so the same entries arrive
                // as a sheet under the finger after the long press.
                sheet: TouchPresentation.usesSheet(surface)
                    ? AnyView(TouchOverlay(isPresented: Binding(get: { menuPoint != nil },
                                                                set: { if !$0 { menuPoint = nil } }),
                                           content: Menu(entries,
                                                         onItemActivated: { menuPoint = nil },
                                                         onDismiss: { menuPoint = nil })))
                    : nil)
        }
    }

    /// Look Up and Share appear only when the host wired the matching hook, so
    /// the stock menu never offers an item that cannot do anything.
    private static func entries(_ scope: TextSelectionScope) -> [MenuEntry] {
        var entries: [MenuEntry] = [
            .item(MenuItem(id: "copy", title: "Copy", shortcut: shortcut("C"),
                           isEnabled: scope.hasSelection) { scope.copy() }),
        ]
        if scope.lookup != nil {
            entries.append(.item(MenuItem(id: "lookup", title: "Look Up",
                                          isEnabled: scope.hasSelection) { scope.lookup?(scope.selectedText) }))
        }
        if scope.share != nil {
            entries.append(.item(MenuItem(id: "share", title: "Share",
                                          isEnabled: scope.hasSelection) { scope.share?(scope.selectedText) }))
        }
        entries.append(.item(MenuItem(id: "selectAll", title: "Select All",
                                      shortcut: shortcut("A")) { scope.selectAll() }))
        return entries
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
    /// The app root already installs one, so labels are selectable out of the
    /// box. Use `.textSelection(false)` on a subtree to make it inert again —
    /// for example a preview pane whose text belongs to an embedded editor.
    func textSelection(_ enabled: Bool = true) -> AnyView {
        enabled ? AnyView(TextSelectionRoot { self })
                : AnyView(self.compositionLocal(TextSelectionScope.key, nil))
    }

    /// Adds Look Up and Share to the selection menu of the enclosing
    /// `textSelection()` region. Each entry appears only when its action is
    /// supplied; without them the menu offers Copy and Select All.
    ///
    /// ```swift
    /// MarkdownView(notes)
    ///     .textSelection()
    ///     .textSelectionActions(lookup: { dictionary.lookUp($0) },
    ///                           share: { shareSheet.present($0) })
    /// ```
    func textSelectionActions(lookup: ((String) -> Void)? = nil,
                              share: ((String) -> Void)? = nil) -> some View {
        TextSelectionActions(lookup: lookup, share: share, content: self)
    }
}

/// Publishes the host's Look Up / Share actions onto the enclosing selection
/// scope. A primitive so the composition local is read from the mounted node,
/// like `TextScalerReader`.
struct TextSelectionActions<Content: View>: _PrimitiveView {
    let lookup: ((String) -> Void)?
    let share: ((String) -> Void)?
    let content: Content

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = false
        return node
    }

    func _updateNode(_ node: Node) {}
    func _makeLayoutNode() -> LayoutNode? { nil }

    func _children(for node: Node) -> [any View] {
        let scope = node.compositionValue(of: TextSelectionScope.key)
        scope?.lookup = lookup
        scope?.share = share
        return [content]
    }
}

/// Chrome shared by every selectable text primitive: the two draggable endpoint
/// handles and the magnifier that follows a finger while it refines a selection.
/// `Text` and `RichText` lay text out differently, so only the parts that read
/// the same for both live here — a caret rectangle in, pixels out.
enum TextSelectionChrome {
    /// Radius of the round endpoint handles.
    static let handleRadius: CGFloat = 6
    /// A press within this distance of a handle centre grabs it.
    static let hitSlop: CGFloat = 16
    static let loupeRadius: Float = 44
    static let loupeScale: Float = 1.8
    /// Gap between the lens and the finger holding it.
    private static let loupeLift: CGFloat = 28

    /// Which endpoint a press grabs. The start handle hangs below its line and the
    /// end handle above, so a one-line selection keeps them apart.
    enum Endpoint: Equatable { case start, end }

    static func handleCenter(caret: CGRect, isStart: Bool) -> CGPoint {
        CGPoint(x: caret.midX,
                y: isStart ? caret.maxY + handleRadius : caret.minY - handleRadius)
    }

    /// The endpoint under a node-local point, or nil when the press missed both.
    static func endpoint(at local: CGPoint, start: CGRect?, end: CGRect?) -> Endpoint? {
        if let start, distance(local, handleCenter(caret: start, isStart: true)) <= hitSlop {
            return .start
        }
        if let end, distance(local, handleCenter(caret: end, isStart: false)) <= hitSlop {
            return .end
        }
        return nil
    }

    /// Draws whichever handles are visible. `origin` is the node's draw origin and
    /// the carets are in text-block space, so the caller keeps one coordinate
    /// conversion instead of repeating it per handle.
    static func drawHandles(into list: DrawList, origin: CGPoint, start: CGRect?, end: CGRect?,
                            fill: Color, border: Color) {
        for (caret, isStart) in [(start, true), (end, false)] {
            guard let caret else { continue }
            let center = handleCenter(caret: caret, isStart: isStart)
            let rect = UIRect(x: Float(origin.x + center.x) - Float(handleRadius),
                              y: Float(origin.y + center.y) - Float(handleRadius),
                              width: Float(handleRadius) * 2,
                              height: Float(handleRadius) * 2)
            list.addRoundedRect(rect, radius: Float(handleRadius), color: fill)
            list.addRoundedRectStroke(rect, radius: Float(handleRadius), width: 1, color: border)
        }
    }

    /// Circular magnifier above the finger. `drawText` receives the draw list and
    /// the vertical shift to add to the text block origin, which is what puts the
    /// character under the finger at the centre of the lens once scaled.
    static func drawLoupe(into list: DrawList, finger: CGPoint, surface: Color, border: Color,
                          drawText: (DrawList, CGFloat) -> Void) {
        let radius = loupeRadius
        let center = CGPoint(x: finger.x, y: finger.y - CGFloat(radius) - loupeLift)
        let rect = UIRect(x: Float(center.x) - radius, y: Float(center.y) - radius,
                          width: radius * 2, height: radius * 2)
        list.addRoundedRect(rect, radius: radius, color: surface)
        let clip = UIClip(rect, mask: UIRoundedRect(rect, radius: radius))
        list.pushClip(clip)
        let shift = (center.y - finger.y) / CGFloat(loupeScale)
        paintScaled({ temp, _ in drawText(temp, shift) }, into: list, origin: finger, scale: loupeScale)
        list.popClip(clip)
        list.addRoundedRectStroke(rect, radius: radius, width: 1.5, color: border)
        // Stem from the lens down to the finger.
        let stem: Float = 3
        let stemTop = Float(center.y) + radius
        list.addRoundedRect(UIRect(x: Float(center.x) - stem / 2, y: stemTop, width: stem,
                                   height: max(0, Float(finger.y) - stemTop)),
                            radius: stem / 2, color: border)
    }

    private static func distance(_ lhs: CGPoint, _ rhs: CGPoint) -> CGFloat {
        hypot(lhs.x - rhs.x, lhs.y - rhs.y)
    }

    /// True when an ancestor already owns the pointer gesture — a button, list row
    /// or table cell. Those controls keep every click, so a copyable label inside
    /// one stays inert instead of swallowing the press. Only normal-priority
    /// routes count: chrome-priority ones are infrastructure (scrollbar chrome,
    /// the selection root's own menu anchor) and must not disable selection, and a
    /// scroll view's route exists to scroll, not to click. Gesture containers that
    /// page or pan content must register with the `.scroll` role for the same
    /// reason — with `.control` they would make every label inside them inert.
    static func isInsideControl(_ node: Node) -> Bool {
        guard let registry = InteractionRegistryHolder.current else { return false }
        var ancestor = node.parent
        while let current = ancestor {
            if let route = registry.handlers(for: current).pointerRoute,
               route.priority <= .normal, route.role != .scroll {
                return true
            }
            ancestor = current.parent
        }
        return false
    }
}
