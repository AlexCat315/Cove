import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Paints one paragraph — code chips, glyphs, link underlines and strike
/// lines — and resolves pointer input against the same flow.
struct RichTextSurface: _PrimitiveView {
    let source: RichTextStyleSource
    let onOpenLink: ((String) -> Void)?

    private struct PaintIdentity: Equatable {
        let paragraph: RichParagraph
        let palette: RichTextPalette
        let selectionScope: ObjectIdentifier?
    }

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = true
        return node
    }

    func _makeLayoutNode() -> LayoutNode? { LayoutNode() }
    /// The flow layout sizes the paragraph; the surface fills what it was given.
    func _updateLayout(_ layout: LayoutNode) {
        layout.setWidthPercent(100)
        layout.setHeightPercent(100)
    }

    func _updateNode(_ node: Node) {
        source.node = node
        let text = source.plainText, role = source.role
        node.accessibility = AccessibilitySemantics(role.accessibilityRole) {
            $0.label = text
            if case .heading(let level) = role { $0.value = "level \(level)" }
        }
        let source = source
        let scope = node.compositionValue(of: TextSelectionScope.key)
        if scope == nil { TextSelectionScope.unregister(node) }
        // Selectable paragraphs take focus on click for copy shortcuts, but are
        // not Tab stops: links are the keyboard targets inside text. Focus is
        // applied explicitly by the press handler, so automatic focus stays off
        // and a paragraph inside a button never pulls focus off its control.
        node.isFocusable = scope != nil && !TextSelectionChrome.isInsideControl(node)
        node.isTabStop = false
        node.automaticallyFocusOnPointerDown = false
        // Cached for the hover cursor, which resolves on every motion event and
        // must not walk the ancestor chain each time.
        node.attachments[Self.inertKey] = TextSelectionChrome.isInsideControl(node)
        scope?.register(node, text: { source.plainText }, offsetAt: { [weak node] point in
            guard let node, let env = TextEnvironmentHolder.current else { return 0 }
            let frame = node.absoluteFrame
            return RichTextFlow.layout(source.paragraph, maxWidth: Float(frame.width), env: env)
                .offset(at: Float(point.x - frame.minX), Float(point.y - frame.minY))
        })
        node.updateDraw(identity: PaintIdentity(paragraph: source.paragraph, palette: source.palette,
                                              selectionScope: scope.map(ObjectIdentifier.init))) { [weak node] list, origin in
            guard let node, let env = TextEnvironmentHolder.current else { return }
            let paragraph = source.paragraph, palette = source.palette
            let flow = RichTextFlow.layout(paragraph, maxWidth: Float(node.frame.width), env: env)
            let selection = scope?.range(for: node)
            RichTextPainter(flow: flow, paragraph: paragraph, palette: palette,
                            hoveredLink: node.attachments[Self.hoveredLinkKey] as? Int,
                            focusedLink: FocusChainHolder.current?.isFocusVisible == true
                                ? node.attachments[Self.focusedLinkKey] as? Int : nil,
                            selection: selection,
                            opacity: node.opacity)
                .paint(into: list, x: Float(origin.x), y: Float(origin.y), env: env)
            // Handles and the magnifier sit above the glyphs so a finger or pen can
            // keep refining the selection without losing sight of the endpoints.
            guard let selection else { return }
            let length = source.plainText.utf8.count
            let start = selection.lowerBound > 0 ? flow.caretRect(at: selection.lowerBound) : nil
            let end = selection.upperBound < length ? flow.caretRect(at: selection.upperBound) : nil
            TextSelectionChrome.drawHandles(into: list, origin: origin, start: start, end: end,
                                            fill: node.theme.colors.accent,
                                            border: node.theme.colors.surfaceFloating)
            if let finger = node.attachments[Self.loupeKey] as? CGPoint {
                TextSelectionChrome.drawLoupe(
                    into: list,
                    finger: CGPoint(x: origin.x + finger.x, y: origin.y + finger.y),
                    surface: node.theme.colors.surfaceFloating,
                    border: node.theme.colors.border
                ) { temp, shift in
                    temp.addText(flow.glyphs,
                                 origin: (Float(origin.x), Float(origin.y) + Float(shift)),
                                 color: palette.text.multipliedAlpha(node.opacity),
                                 textureID: env.atlasTextureID, atlas: env.atlas)
                }
            }
        }
        installPointer(on: node, scope: scope)
    }

    private func installPointer(on node: Node, scope: TextSelectionScope?) {
        guard let registry = InteractionRegistryHolder.current else { return }
        let destinations = source.destinations, open = onOpenLink, source = source
        let flowFor = { (node: Node) -> RichTextFlow? in
            guard let env = TextEnvironmentHolder.current else { return nil }
            return RichTextFlow.layout(source.paragraph, maxWidth: Float(node.absoluteFrame.width), env: env)
        }
        let linkAt = { [weak node] (x: Float, y: Float) -> Int? in
            guard let node, open != nil, let flow = flowFor(node) else { return nil }
            let frame = node.absoluteFrame
            guard let fragment = flow.fragment(at: x - Float(frame.minX), y - Float(frame.minY)) else { return nil }
            return source.paragraph.spans[fragment.span].link
        }
        let setHovered = { [weak node] (link: Int?) in
            guard let node else { return }
            let selectable = scope != nil && node.attachments[Self.inertKey] as? Bool != true
            node.cursor = link != nil ? .pointer : selectable && node.supportsControlHover ? .ibeam : nil
            guard node.attachments[Self.hoveredLinkKey] as? Int != link else { return }
            node.attachments[Self.hoveredLinkKey] = link
            node.markRenderDirty(reason: .styleSet(field: Self.hoveredLinkKey))
        }
        let point = { (x: Float, y: Float) in CGPoint(x: CGFloat(x), y: CGFloat(y)) }
        registry.setHover(node) { phase in if phase == .leave { setHovered(nil) } }
        registry.setMotion(node) { [weak node] event, phase in
            guard phase != .capture else { return .ignored }
            guard let node else { return .ignored }
            // Dragging an endpoint refines the selection, even with a finger, and
            // keeps the pointer cursor instead of a link's hand.
            if let drag = node.attachments[Self.handleDragKey] as? TextSelectionChrome.Endpoint,
               let scope, let selection = scope.range(for: node), let flow = flowFor(node) {
                let local = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX,
                                    y: CGFloat(event.y) - node.absoluteFrame.minY)
                node.attachments[Self.loupeKey] = local
                let offset = flow.offset(at: Float(local.x), Float(local.y))
                if drag == .start {
                    let lower = min(max(0, offset), selection.upperBound)
                    scope.replaceRange(in: node, with: lower..<selection.upperBound)
                } else {
                    let length = source.plainText.utf8.count
                    let upper = max(min(length, offset), selection.lowerBound)
                    scope.replaceRange(in: node, with: selection.lowerBound..<upper)
                }
                node.markRenderDirty(reason: .styleSet(field: Self.handleDragKey))
                return .handled
            }
            setHovered(linkAt(event.x, event.y))
            guard node.attachments[Self.selectingKey] as? Bool == true, let scope,
                  let position = scope.position(at: point(event.x, event.y)) else { return .ignored }
            scope.extend(to: position)
            return .handled
        }
        registry.setPointer(node) { [weak node] event, phase, eventPhase in
            guard let node, eventPhase != .capture, event.button == .left else { return .ignored }
            switch phase {
            case .down:
                // A handle wins over a link and over starting a fresh selection.
                if let scope, let drag = handleEndpoint(node, event: event, scope: scope) {
                    node.attachments[Self.handleDragKey] = drag
                    node.attachments[Self.loupeKey] = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX,
                                                              y: CGFloat(event.y) - node.absoluteFrame.minY)
                    PointerCaptureHolder.current?.acquire(node)
                    FocusChainHolder.current?.focus(node, visible: false)
                    return .handled
                }
                if let link = linkAt(event.x, event.y) {
                    node.attachments[Self.pressedLinkKey] = link
                    PointerCaptureHolder.current?.acquire(node, allowsGestureTransfer: true)
                    return .handled
                }
                // A paragraph inside a button or row leaves the press to that
                // control, so the control keeps every click.
                guard let scope, !TextSelectionChrome.isInsideControl(node) else { return .ignored }
                // A precise pointer (mouse/pen) selects; a finger keeps the drag for
                // scrolling and long-presses to select. On touch PCs both a mouse and a
                // touchscreen report `supportsHover`, so branch on the event's source,
                // not just the device capability.
                let precise = event.kind == .mouse || event.kind == .pen
                guard node.supportsControlHover, precise else {
                    // Touch keeps drags for scrolling; a long press selects instead.
                    node.attachments[Self.touchPressKey] = (Date(), point(event.x, event.y))
                    return .ignored
                }
                let here = point(event.x, event.y)
                if event.clicks >= 3 { scope.selectParagraph(node) }
                else if event.clicks == 2 { scope.selectWord(at: here) }
                else if let position = scope.position(at: here) {
                    if event.modifiers.isDisjoint(with: .shift) { scope.select(from: position, to: position) }
                    else { scope.extend(to: position) }
                }
                node.attachments[Self.selectingKey] = event.clicks < 2
                PointerCaptureHolder.current?.acquire(node)
                // Focus without a ring, so copy and select-all shortcuts reach the paragraph.
                FocusChainHolder.current?.focus(node, visible: false)
                return .handled
            case .up:
                if let drag = node.attachments.removeValue(forKey: Self.handleDragKey)
                    as? TextSelectionChrome.Endpoint {
                    node.attachments.removeValue(forKey: Self.loupeKey)
                    if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                    // Offer the action bar where the handle landed.
                    if let scope, let selection = scope.range(for: node), let flow = flowFor(node) {
                        let caret = flow.caretRect(at: drag == .start ? selection.lowerBound : selection.upperBound)
                        let frame = node.absoluteFrame
                        scope.requestMenu?(CGPoint(x: frame.minX + caret.origin.x,
                                                   y: frame.minY + caret.origin.y))
                    }
                    node.markRenderDirty(reason: .styleSet(field: Self.handleDragKey))
                    return .handled
                }
                if let (start, origin) = node.attachments.removeValue(forKey: Self.touchPressKey) as? (Date, CGPoint) {
                    let here = point(event.x, event.y)
                    guard let scope, Date().timeIntervalSince(start) >= 0.45,
                          hypot(here.x - origin.x, here.y - origin.y) < 10 else { return .ignored }
                    scope.selectWord(at: here)
                    // Focus without a ring so copy reaches this paragraph's selection.
                    FocusChainHolder.current?.focus(node, visible: false)
                    scope.requestMenu?(here)
                    return .handled
                }
                if node.attachments.removeValue(forKey: Self.selectingKey) != nil {
                    if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                    return .handled
                }
                guard let pressed = node.attachments.removeValue(forKey: Self.pressedLinkKey) as? Int else { return .ignored }
                if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                if linkAt(event.x, event.y) == pressed, pressed < destinations.count { open?(destinations[pressed]) }
                return .handled
            }
        }
        guard let scope else { registry.setKey(node) { _, _ in .ignored }; return }
        registry.setKey(node) { event, phase in
            guard phase != .capture else { return .ignored }
            let primary = !event.modifiers.isDisjoint(with: .gui) || !event.modifiers.isDisjoint(with: .ctrl)
            switch event.scancode {
            case Scancode.c where primary:
                guard scope.hasSelection else { return .ignored }
                scope.copy()
                return .handled
            case Scancode.a where primary:
                scope.selectAll()
                return .handled
            case Scancode.arrowLeft where event.modifiers.contains(.shift):
                return scope.extendFocus(by: -1) ? .handled : .ignored
            case Scancode.arrowRight where event.modifiers.contains(.shift):
                return scope.extendFocus(by: 1) ? .handled : .ignored
            case Scancode.home where event.modifiers.contains(.shift):
                return scope.extendFocusToEdge(start: true) ? .handled : .ignored
            case Scancode.end where event.modifiers.contains(.shift):
                return scope.extendFocusToEdge(start: false) ? .handled : .ignored
            case Scancode.escape:
                guard scope.hasSelection else { return .ignored }
                scope.clear()
                return .handled
            default:
                return .ignored
            }
        }
    }

    static let hoveredLinkKey = "__rich_text_hovered_link"
    static let pressedLinkKey = "__rich_text_pressed_link"
    static let focusedLinkKey = "__rich_text_focused_link"
    static let selectingKey = "__rich_text_selecting"
    static let touchPressKey = "__rich_text_touch_press"
    /// Set while a selection endpoint handle is being dragged.
    static let handleDragKey = "__rich_text_handle_drag"
    /// Node-local finger point (CGPoint) while a handle is dragged, drives the loupe.
    static let loupeKey = "__rich_text_loupe"
    /// Cached "this paragraph sits inside a control" flag for the hover cursor.
    static let inertKey = "__rich_text_inert"

    /// Caret rectangles for the two draggable endpoints, in node-local space. An
    /// endpoint on the paragraph's own edge has no handle to drag.
    private func handleCarets(_ node: Node, scope: TextSelectionScope) -> (start: CGRect?, end: CGRect?) {
        guard let selection = scope.range(for: node), let env = TextEnvironmentHolder.current else {
            return (nil, nil)
        }
        let frame = node.absoluteFrame
        let flow = RichTextFlow.layout(source.paragraph, maxWidth: Float(frame.width), env: env)
        let length = source.plainText.utf8.count
        return (selection.lowerBound > 0 ? flow.caretRect(at: selection.lowerBound) : nil,
                selection.upperBound < length ? flow.caretRect(at: selection.upperBound) : nil)
    }

    /// Which endpoint handle sits under a pressed point, if any.
    private func handleEndpoint(_ node: Node, event: MouseButtonEvent,
                                scope: TextSelectionScope) -> TextSelectionChrome.Endpoint? {
        let carets = handleCarets(node, scope: scope)
        guard carets.start != nil || carets.end != nil else { return nil }
        let local = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX,
                            y: CGFloat(event.y) - node.absoluteFrame.minY)
        return TextSelectionChrome.endpoint(at: local, start: carets.start, end: carets.end)
    }
}

private extension RichTextRole {
    var accessibilityRole: AccessibilityRole {
        switch self {
        case .heading: .heading
        case .paragraph, .codeBlock: .staticText
        }
    }
}

struct RichTextPainter {
    let flow: RichTextFlow
    let paragraph: RichParagraph
    let palette: RichTextPalette
    let hoveredLink: Int?
    let focusedLink: Int?
    var selection: Range<Int>? = nil
    let opacity: Float

    func paint(into list: DrawList, x: Float, y: Float, env: TextEnvironment) {
        for fragment in flow.fragments where paragraph.spans[fragment.span].traits.contains(.code) {
            let line = flow.lines[fragment.line]
            let inset: Float = 2
            let rect = UIRect(x: x + fragment.minX, y: y + line.baseline - fragment.ascent - inset,
                              width: fragment.maxX - fragment.minX,
                              height: fragment.ascent + fragment.descent + inset * 2)
            list.addRoundedRect(rect, radius: 4, color: palette.codeBackground.multipliedAlpha(opacity))
        }
        if let selection {
            for (sx, sy, sw, sh) in flow.selectionRects(selection) {
                list.addRect(UIRect(x: x + sx, y: y + sy, width: sw, height: sh), color: palette.selection.multipliedAlpha(opacity))
            }
        }
        list.addText(flow.glyphs, origin: (x, y), color: palette.text.multipliedAlpha(opacity),
                     textureID: env.atlasTextureID, atlas: env.atlas) { glyph in
            flow.span(atOffset: Int(glyph.cluster)).map { color(for: paragraph.spans[$0]) }
        }
        for fragment in flow.fragments {
            let span = paragraph.spans[fragment.span]
            let line = flow.lines[fragment.line]
            let width = fragment.maxX - fragment.minX
            if span.link != nil {
                let offset = max(1, (fragment.descent * 0.4).rounded())
                list.addRect(UIRect(x: x + fragment.minX, y: y + line.baseline + offset, width: width, height: 1),
                             color: color(for: span))
            }
            if span.traits.contains(.strikethrough) {
                let middle = (fragment.ascent * 0.32).rounded()
                list.addRect(UIRect(x: x + fragment.minX, y: y + line.baseline - middle, width: width, height: 1),
                             color: color(for: span))
            }
            // Keyboard focus rings every wrapped fragment of the link.
            if let focusedLink, span.link == focusedLink {
                let rect = UIRect(x: x + fragment.minX - 4, y: y + line.top - 1, width: width + 8, height: line.height + 2)
                list.addRoundedRectStroke(rect, radius: 4, width: 2, color: palette.focusRing.multipliedAlpha(opacity))
            }
        }
    }

    private func color(for span: RichParagraph.Span) -> Color {
        let base: Color = if let link = span.link {
            link == hoveredLink ? palette.linkHover : palette.link
        } else {
            palette.text
        }
        return base.multipliedAlpha(opacity)
    }
}
