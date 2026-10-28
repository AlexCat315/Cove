import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif
import CoveUIRuntime
import CoveUIScene

/// Process-wide text rendering context. The Compose layer asks the host to
/// install a `TextEnvironment` once at startup; primitives (`Text`) read from
/// it during materialisation. Decoupled so the runtime tests do not need a
/// real FreeType font loaded.
public struct TextEnvironment {
    public let atlas: FontAtlas
    public let shaper: TextShaper
    public let atlasTextureID: TextureID
    public var defaultLineHeight: Float
    public var defaultColor: Color
    public var defaultFont: Font
    public var fontResolver: TextFontResolver?
    let shapeCache: SharedTextShapeCache
    let layoutCache: SharedTextLayoutCache

    public init(atlas: FontAtlas,
                shaper: TextShaper,
                atlasTextureID: TextureID,
                defaultLineHeight: Float,
                defaultColor: Color = .white,
                defaultFont: Font? = nil,
                fontResolver: TextFontResolver? = nil,
                cacheLimits: TextCacheLimits = TextCacheLimits()) {
        self.atlas = atlas
        self.shaper = shaper
        self.atlasTextureID = atlasTextureID
        self.defaultLineHeight = defaultLineHeight
        self.defaultColor = defaultColor
        let fallbackSize = atlas.fontSize > 0 ? atlas.fontSize : max(1, defaultLineHeight)
        self.defaultFont = defaultFont ?? Font.system(size: fallbackSize)
        self.fontResolver = fontResolver
        self.shapeCache = SharedTextShapeCache(limits: cacheLimits)
        self.layoutCache = SharedTextLayoutCache(limits: cacheLimits)
    }

    public var shapingCacheSnapshot: TextCacheSnapshot { shapeCache.cache.snapshot }
    public var layoutCacheSnapshot: TextCacheSnapshot { layoutCache.cache.snapshot }

    /// Drops reusable results. Visible nodes keep their own layout snapshots.
    public func trimTextCaches() {
        shapeCache.cache.clear()
        layoutCache.cache.clear()
    }

    public static func bootstrapped(atlasTextureID: TextureID,
                                    primaryFontName: String,
                                    defaultFont: Font = .system(size: 18),
                                    defaultLineHeight: Float = 22,
                                    defaultColor: Color = .white,
                                    rasterScale: Float = 1,
                                    atlasEdge: Int? = nil,
                                    cacheLimits: TextCacheLimits = TextCacheLimits()) -> TextEnvironment {
        let scale = max(1, rasterScale)
        let resolvedAtlasEdge = atlasEdge ?? max(1024, Int((1024 * scale).rounded(.up)))
        let atlas = FontAtlas(width: resolvedAtlasEdge, height: resolvedAtlasEdge)
        let shaper = TextShaper()
        let resolver = TextFontResolver(primaryFontName: primaryFontName,
                                        atlas: atlas,
                                        rasterScale: scale)

        if let primaryFont = resolver.preparePrimaryFont(defaultFont) {
            shaper.setFont(ftFace: primaryFont.rawFace,
                           size: defaultFont.size,
                           rasterScale: scale)
        }

        return TextEnvironment(
            atlas: atlas,
            shaper: shaper,
            atlasTextureID: atlasTextureID,
            defaultLineHeight: defaultLineHeight,
            defaultColor: defaultColor,
            defaultFont: defaultFont,
            fontResolver: resolver,
            cacheLimits: cacheLimits
        )
    }

    public func resolvedFont(_ override: Font?) -> Font {
        override ?? defaultFont
    }

    public func resolvedLineHeight(font: Font, override: Float?) -> Float {
        if let override {
            return override
        }
        guard defaultFont.size > 0 else { return defaultLineHeight }
        return defaultLineHeight * (font.size / defaultFont.size)
    }

    public func shape(text: String, font: Font?) -> [ShapedGlyph] {
        let resolved = resolvedFont(font)
        return shapeCache.value(text: text, font: resolved) {
            if let fontResolver {
                let glyphs = fontResolver.shape(text: text, font: resolved)
                if !glyphs.isEmpty || text.isEmpty {
                    return glyphs
                }
            }
            return shaper.shape(text: text)
        }
    }

    public func cachedLayout(text: String,
                             font: Font? = nil,
                             lineHeight: Float? = nil,
                             maxWidth: Float = .infinity,
                             alignment: TextAlignment = .leading,
                             letterSpacing: Float = 0) -> TextLayoutResult {
        let resolvedFont = resolvedFont(font)
        let resolvedLineHeight = resolvedLineHeight(font: resolvedFont, override: lineHeight)
        let spacing = letterSpacing.isFinite ? letterSpacing : 0
        let normalizedMaxWidth: Float = (maxWidth.isFinite && maxWidth > 0) ? maxWidth : .infinity
        let key = SharedTextLayoutCache.Key(
            text: text,
            font: resolvedFont,
            lineHeight: resolvedLineHeight,
            alignment: alignment,
            maxWidth: normalizedMaxWidth,
            letterSpacing: spacing
        )
        return layoutCache.value(for: key) {
            let glyphs = shape(text: text, font: resolvedFont)
            return TextLayout.layout(
                shapedGlyphs: glyphs,
                text: text,
                atlas: atlas,
                maxWidth: normalizedMaxWidth,
                lineHeight: resolvedLineHeight,
                alignment: alignment,
                letterSpacing: spacing
            )
        }
    }

    public func prewarmGlyphs(text: String, fonts: [Font]) {
        for font in Set(fonts) {
            prewarmGlyphs(text: text, font: font)
        }
    }

    public func prewarmGlyphs(text: String, font: Font? = nil) {
        let glyphs = shape(text: text, font: font)
        for glyph in glyphs {
            _ = atlas.rasterizeGlyph(glyphIndex: glyph.glyphID, fontID: glyph.fontID)
        }
    }
}

final class SharedTextShapeCache {
    struct Key: Hashable {
        let text: String
        let font: Font
    }

    let cache: TextCache<Key, [ShapedGlyph]>

    init(limits: TextCacheLimits = TextCacheLimits()) { cache = TextCache(limits: limits) }

    func value(text: String, font: Font, build: () -> [ShapedGlyph]) -> [ShapedGlyph] {
        let key = Key(text: text, font: font)
        return cache.value(for: key, cost: {
            text.utf8.count + MemoryLayout<Key>.stride + $0.count * MemoryLayout<ShapedGlyph>.stride
        }, build: build)
    }
}

final class SharedTextLayoutCache {
    struct Key: Hashable {
        let text: String
        let font: Font
        let lineHeight: Float
        let alignment: TextAlignment
        let maxWidth: Float
        let letterSpacing: Float
    }

    let cache: TextCache<Key, TextLayoutResult>

    init(limits: TextCacheLimits = TextCacheLimits()) { cache = TextCache(limits: limits) }

    func value(for key: Key, build: () -> TextLayoutResult) -> TextLayoutResult {
        cache.value(for: key, cost: { result in
            key.text.utf8.count + MemoryLayout<Key>.stride
                + result.lines.count * MemoryLayout<TextLine>.stride
                + result.lines.reduce(0) { $0 + $1.glyphs.count * MemoryLayout<PositionedGlyph>.stride }
        }, build: build)
    }
}

/// Holder so views can reach the environment without threading it through every
/// builder. Set by `SDL3PlatformHost` (or any other shell) before the first
/// `materialise` call.
public enum TextEnvironmentHolder {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var value: TextEnvironment?
    /// Reads retain under the lock swaps release under, so replacing the
    /// environment never frees one another thread is still loading.
    public static var current: TextEnvironment? {
        get { lock.withLock { value } }
        set { lock.withLock { value = newValue } }
    }
}

/// Static text primitive. Participates in flexbox layout via Yoga's measure
/// callback so a label sized by its parent will wrap; an unconstrained label
/// reports its natural single-line width.
public struct Text: _PrimitiveView {
    public let string: String
    public let alignment: TextAlignment
    public let color: Color?
    public let lineLimit: Int?
    private var truncates = false
    /// Opts the label into text selection when it sits inside a `textSelection()`
    /// region. Plain `Text` outside one stays inert, so labels in buttons and
    /// toolbars keep every click.
    private var isSelectable = true
    private static let touchPressKey = "__text_touch_press"
    private static let selectingKey = "__text_selecting"
    /// Set to `DragEnd` while a selection endpoint handle is being dragged.
    private static let handleDragKey = "__text_handle_drag"
    /// Node-local finger point (CGRect) while a handle is dragged, drives the loupe.
    private static let loupeKey = "__text_loupe"

    /// Which endpoint a drag started from.
    private enum DragEnd { case start, end }

    private struct PaintIdentity: Equatable {
        let string: String
        let alignment: TextAlignment
        let color: Color?
        let lineLimit: Int?
        let truncates: Bool
    }

    public init(_ string: String,
                alignment: TextAlignment = .leading,
                color: Color? = nil,
                lineLimit: Int? = nil) {
        self.string = string
        self.alignment = alignment
        self.color = color
        self.lineLimit = lineLimit
    }

    public init(_ key: LocalizedStringKey,
                alignment: TextAlignment = .leading,
                color: Color? = nil,
                lineLimit: Int? = nil) {
        self.init(key.resolved, alignment: alignment, color: color, lineLimit: lineLimit)
    }

    public func lineLimit(_ limit: Int?) -> Text {
        Text(string, alignment: alignment, color: color, lineLimit: limit)
    }

    /// A single line with a trailing ellipsis, a full-text tooltip and the
    /// original accessible name. Truncation never splits a grapheme cluster.
    /// The tooltip appears only when the line actually had to clip.
    public func truncated() -> some View { truncatingTail().tooltip(string, onlyWhenTruncated: true) }

    /// Controls with their own tooltip reuse the same painter without nesting descriptions.
    func truncatingTail() -> Text { var copy = self; copy.truncates = true; return copy }

    /// Enables text selection for this label when it is inside a `textSelection()`
    /// region, so its content can be copied like a `RichText`.
    public func textSelectionEnabled(_ enabled: Bool = true) -> Text {
        var copy = self; copy.isSelectable = enabled; return copy
    }

    public func _makeNode() -> Node {
        let n = Node()
        n.isHitTestable = false
        return n
    }

    public func _updateNode(_ node: Node) {
        node.syncTextStyleEnvironment()
        node.clipsToBounds = truncates
        // Rebuilding the full semantics struct for an unchanged label is a hot
        // path during list/table scrolling. Compare the fields this primitive
        // owns before allocating a new value.
        if node.accessibility?.role != .staticText || node.accessibility?.label != string {
            node.accessibility = AccessibilitySemantics(.staticText) { $0.label = string }
        }
        // Join an enclosing selection region. A `Text` outside one stays inert, so
        // labels inside buttons and toolbars keep every click for their control.
        let scope = node.compositionValue(of: TextSelectionScope.key)
        let selectable = isSelectable && scope != nil
        node.isHitTestable = selectable
        node.isFocusable = selectable
        // Bind the draw callback. Captures `string` etc by value.
        let snapshot = self
        node.updateDraw(identity: PaintIdentity(string: string,
                                                alignment: alignment,
                                                color: color,
                                                lineLimit: lineLimit, truncates: truncates)) { list, origin in
            guard let env = TextEnvironmentHolder.current else { return }
            let fontOverride = node.textStyleValue(StyleAttachmentKey.font) as Font?
            let lineHeightOverride = node.textStyleValue(StyleAttachmentKey.lineHeight) as Float?
            let resolvedFont = fontOverride ?? node.textScaler.scale(env.defaultFont)
            let resolvedLineHeight = env.resolvedLineHeight(font: resolvedFont,
                                                            override: lineHeightOverride)
            let padding = node.layoutNode?.resolvedPadding
            let result = snapshot.displayLayout(
                env: env,
                layout: node.layoutNode,
                font: resolvedFont,
                lineHeight: resolvedLineHeight,
                maxWidth: max(0, Float(node.frame.width) - (padding?.left ?? 0) - (padding?.right ?? 0)),
                letterSpacing: node.textStyleValue(StyleAttachmentKey.letterSpacing) ?? 0
            )
            if snapshot.truncates {
                let marker = TextTruncation.attachmentKey
                if node.attachments[marker] as? Bool != result.truncated {
                    node.attachments[marker] = result.truncated
                }
            }
            // Paint the selection behind the glyphs so the highlight reads under text.
            if let selection = scope?.range(for: node), !selection.isEmpty {
                let selColor = node.theme.colors.selection
                for rect in result.layout.selectionRects(selection) {
                    list.addRect(UIRect(x: Float(origin.x) + (padding?.left ?? 0) + Float(rect.origin.x),
                                        y: Float(origin.y) + (padding?.top ?? 0) + Float(rect.origin.y),
                                        width: Float(rect.width), height: Float(rect.height)),
                                 color: selColor.multipliedAlpha(node.opacity))
                }
            }
            // Composite button labels and other containers supply inherited
            // foregrounds; standalone text falls back to the active theme.
            let baseColor = node.debugForegroundColor ?? snapshot.color ?? node.inheritedForegroundColor ?? node.theme.colors.onSurface
            let drawColor = baseColor.multipliedAlpha(node.opacity)
            list.addText(result.layout,
                         origin: (Float(origin.x) + (padding?.left ?? 0), Float(origin.y) + (padding?.top ?? 0)),
                         color: drawColor,
                         textureID: env.atlasTextureID,
                         atlas: env.atlas)
            // Selection handles and the magnifier are drawn above the glyphs so they
            // stay visible while a finger or pen refines either endpoint.
            if let selection = scope?.range(for: node), !selection.isEmpty {
                let length = snapshot.string.utf8.count
                if selection.lowerBound != 0 || selection.upperBound != length {
                    let padL = CGFloat(padding?.left ?? 0), padT = CGFloat(padding?.top ?? 0)
                    let accent = node.theme.colors.accent
                    let border = node.theme.colors.surfaceFloating
                    if selection.lowerBound != 0 {
                        let c = Self.handleCenter(result.layout, offset: selection.lowerBound, bottom: true, padL: padL, padT: padT)
                        Self.drawHandle(into: list, at: CGPoint(x: CGFloat(origin.x) + c.x, y: CGFloat(origin.y) + c.y), color: accent, border: border)
                    }
                    if selection.upperBound != length {
                        let c = Self.handleCenter(result.layout, offset: selection.upperBound, bottom: false, padL: padL, padT: padT)
                        Self.drawHandle(into: list, at: CGPoint(x: CGFloat(origin.x) + c.x, y: CGFloat(origin.y) + c.y), color: accent, border: border)
                    }
                }
                if let finger = node.attachments[Self.loupeKey] as? CGPoint {
                    Self.drawLoupe(into: list, layout: result.layout, env: env, color: drawColor,
                                   finger: CGPoint(x: finger.x, y: finger.y),
                                   padL: CGFloat(padding?.left ?? 0), padT: CGFloat(padding?.top ?? 0),
                                   surface: node.theme.colors.surfaceFloating, border: node.theme.colors.border,
                                   origin: origin)
                }
            }
        }
        if selectable, let scope {
            installTextSelection(on: node, scope: scope)
        }
    }

    public func _makeLayoutNode() -> LayoutNode? {
        let layout = LayoutNode()
        Text.installMeasureFunc(on: layout, snapshot: self)
        layout.textInputs = TextMeasureInputs(
            text: string,
            alignment: alignment,
            lineLimit: truncates ? 1 : lineLimit
        )
        return layout
    }

    public func _updateLayout(_ layout: LayoutNode) {
        Text.installMeasureFunc(on: layout, snapshot: self)
        let next = TextMeasureInputs(text: string,
                                     alignment: alignment,
                                     lineLimit: truncates ? 1 : lineLimit)
        let previous = layout.textInputs
        layout.textInputs = next
        if previous != nil, previous != next {
            layout.markDirty()
        }
    }

    // MARK: - Text selection

    /// Binds the pointer, motion and key handlers that make this label selectable
    /// within `scope`. Mirrors `RichTextSurface`: a precise pointer selects, a
    /// touch long-press selects a word, endpoint handles refine the selection by
    /// drag, and Cmd/Ctrl+C / +A / Esc drive copy.
    private func installTextSelection(on node: Node, scope: TextSelectionScope) {
        guard let registry = InteractionRegistryHolder.current else { return }
        let point = { (x: Float, y: Float) in CGPoint(x: CGFloat(x), y: CGFloat(y)) }
        let snapshot = self
        scope.register(node, text: { snapshot.string }, offsetAt: { [weak node] windowPoint in
            guard let node, let layout = layoutResult(node) else { return 0 }
            let local = CGPoint(x: windowPoint.x - node.absoluteFrame.minX,
                                y: windowPoint.y - node.absoluteFrame.minY)
            return layout.offset(at: local)
        })
        registry.setHover(node) { [weak node] phase in
            guard let node else { return }
            let onHandle = node.attachments[Self.handleDragKey] != nil
            node.cursor = (phase == .enter && (node.supportsControlHover || onHandle)) ? .ibeam : nil
        }
        registry.setPointer(node) { [weak node] event, phase, eventPhase in
            guard let node, eventPhase != .capture, event.button == .left else { return .ignored }
            // Grabbing an endpoint handle refines the selection, even with a finger.
            if let drag = handleHitTest(node: node, event: event, scope: scope) {
                node.attachments[Self.handleDragKey] = drag
                node.attachments[Self.loupeKey] = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX,
                                                          y: CGFloat(event.y) - node.absoluteFrame.minY)
                PointerCaptureHolder.current?.acquire(node)
                FocusChainHolder.current?.focus(node, visible: false)
                return .handled
            }
            let precise = event.kind == .mouse || event.kind == .pen
            switch phase {
            case .down:
                guard precise else {
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
                FocusChainHolder.current?.focus(node, visible: false)
                return .handled
            case .up:
                if let drag = node.attachments.removeValue(forKey: Self.handleDragKey) as? DragEnd {
                    node.attachments.removeValue(forKey: Self.loupeKey)
                    if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                    // Offer the action bar where the handle landed.
                    if let selection = scope.range(for: node), let layout = layoutResult(node) {
                        let offset = drag == .start ? selection.lowerBound : selection.upperBound
                        let r = layout.caretRect(at: offset)
                        scope.requestMenu?(point(Float(node.absoluteFrame.minX + CGFloat(r.origin.x)),
                                                Float(node.absoluteFrame.minY + CGFloat(r.origin.y))))
                    }
                    return .handled
                }
                if let (start, origin) = node.attachments.removeValue(forKey: Self.touchPressKey) as? (Date, CGPoint) {
                    let here = point(event.x, event.y)
                    guard Date().timeIntervalSince(start) >= 0.45,
                          hypot(here.x - origin.x, here.y - origin.y) < 10 else { return .ignored }
                    scope.selectWord(at: here)
                    scope.requestMenu?(here)
                    return .handled
                }
                if node.attachments.removeValue(forKey: Self.selectingKey) != nil {
                    if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                    return .handled
                }
                return .ignored
            }
        }
        registry.setMotion(node) { [weak node] event, phase in
            guard phase != .capture else { return .ignored }
            guard let node else { return .ignored }
            if let drag = node.attachments[Self.handleDragKey] as? DragEnd,
               let selection = scope.range(for: node) {
                let local = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX,
                                    y: CGFloat(event.y) - node.absoluteFrame.minY)
                node.attachments[Self.loupeKey] = local
                if let layout = layoutResult(node) {
                    var newOffset = layout.offset(at: local)
                    if drag == .start {
                        newOffset = min(max(0, newOffset), selection.upperBound)
                        scope.replaceRange(in: node, with: newOffset..<selection.upperBound)
                    } else {
                        let length = snapshot.string.utf8.count
                        newOffset = max(min(length, newOffset), selection.lowerBound)
                        scope.replaceRange(in: node, with: selection.lowerBound..<newOffset)
                    }
                }
                node.markRenderDirty(reason: .styleSet(field: "textSelectionHandle"))
                return .handled
            }
            guard node.attachments[Self.selectingKey] as? Bool == true,
                  let position = scope.position(at: point(event.x, event.y)) else { return .ignored }
            scope.extend(to: position)
            return .handled
        }
        registry.setKey(node) { event, phase in
            guard phase != .capture else { return .ignored }
            let primary = !event.modifiers.isDisjoint(with: .gui) || !event.modifiers.isDisjoint(with: .ctrl)
            switch event.scancode {
            case Scancode.c where primary:
                guard scope.hasSelection else { return .ignored }
                scope.copy(); return .handled
            case Scancode.a where primary:
                scope.selectAll(); return .handled
            case Scancode.escape:
                guard scope.hasSelection else { return .ignored }
                scope.clear(); return .handled
            default:
                return .ignored
            }
        }
    }

    /// Returns which endpoint handle (if any) sits under a pressed point, so a drag
    /// can adjust the selection instead of starting a fresh one.
    private func handleHitTest(node: Node, event: MouseButtonEvent, scope: TextSelectionScope) -> DragEnd? {
        guard let selection = scope.range(for: node), !selection.isEmpty else { return nil }
        let length = string.utf8.count
        guard selection.lowerBound != 0 || selection.upperBound != length else { return nil }
        guard let layout = layoutResult(node) else { return nil }
        let local = CGPoint(x: CGFloat(event.x) - node.absoluteFrame.minX, y: CGFloat(event.y) - node.absoluteFrame.minY)
        let padding = node.layoutNode?.resolvedPadding
        let padL = CGFloat(padding?.left ?? 0)
        let padT = CGFloat(padding?.top ?? 0)
        let threshold: CGFloat = 16
        if selection.lowerBound != 0 {
            let c = Self.handleCenter(layout, offset: selection.lowerBound, bottom: true, padL: padL, padT: padT)
            if hypot(local.x - c.x, local.y - c.y) <= threshold { return .start }
        }
        if selection.upperBound != length {
            let c = Self.handleCenter(layout, offset: selection.upperBound, bottom: false, padL: padL, padT: padT)
            if hypot(local.x - c.x, local.y - c.y) <= threshold { return .end }
        }
        return nil
    }

    /// Node-local center (in draw-list coordinates) of an endpoint handle.
    private static func handleCenter(_ layout: TextLayoutResult, offset: Int, bottom: Bool,
                                     padL: CGFloat, padT: CGFloat) -> CGPoint {
        let r = layout.caretRect(at: offset)
        let radius: CGFloat = 6
        let cx = padL + r.origin.x
        let cy = padT + (bottom ? r.maxY : r.minY)
        return CGPoint(x: cx, y: bottom ? cy + radius : cy - radius)
    }

    private static func drawHandle(into list: DrawList, at center: CGPoint, color: Color, border: Color) {
        let radius: Float = 6
        let rect = UIRect(x: Float(center.x) - radius, y: Float(center.y) - radius, width: 2 * radius, height: 2 * radius)
        list.addRoundedRect(rect, radius: radius, color: color)
        list.addRoundedRectStroke(rect, radius: radius, width: 1, color: border)
    }

    /// Circular magnifier that re-renders the text scaled up around the finger.
    private static func drawLoupe(into list: DrawList, layout: TextLayoutResult, env: TextEnvironment,
                                  color: Color, finger: CGPoint, padL: CGFloat, padT: CGFloat,
                                  surface: Color, border: Color, origin: CGPoint) {
        let fX = Float(origin.x) + Float(finger.x)
        let fY = Float(origin.y) + Float(finger.y)
        let lensRadius: Float = 44
        let lensCX = fX
        let lensCY = fY - lensRadius - 28
        let lensRect = UIRect(x: lensCX - lensRadius, y: lensCY - lensRadius, width: 2 * lensRadius, height: 2 * lensRadius)
        list.addRoundedRect(lensRect, radius: lensRadius, color: surface)
        let clip = UIClip(lensRect, mask: UIRoundedRect(lensRect, radius: lensRadius))
        list.pushClip(clip)
        let scale: Float = 1.8
        let shiftY = (lensCY - fY) / scale
        let drawX = Float(origin.x) + Float(padL)
        let drawY = Float(origin.y) + Float(padT)
        let originForScale = CGPoint(x: CGFloat(fX), y: CGFloat(fY))
        paintScaled({ temp, _ in
            temp.addText(layout, origin: (drawX, drawY + shiftY), color: color,
                         textureID: env.atlasTextureID, atlas: env.atlas)
        }, into: list, origin: originForScale, scale: scale)
        list.popClip(clip)
        list.addRoundedRectStroke(lensRect, radius: lensRadius, width: 1.5, color: border)
        // Stem from the lens down to the finger.
        let stemW: Float = 3
        let stemTop = lensCY + lensRadius
        list.addRoundedRect(UIRect(x: lensCX - stemW / 2, y: stemTop, width: stemW,
                                   height: max(0, fY - stemTop)), radius: stemW / 2, color: border)
    }

    /// The laid-out text for offset/rect math, recomputed the same way the painter
    /// does so the caret and highlight line up with what is drawn.
    private func layoutResult(_ node: Node) -> TextLayoutResult? {
        guard let env = TextEnvironmentHolder.current else { return nil }
        let padding = node.layoutNode?.resolvedPadding
        let maxWidth = max(0, Float(node.frame.width) - (padding?.left ?? 0) - (padding?.right ?? 0))
        let fontOverride = node.textStyleValue(StyleAttachmentKey.font) as Font?
        let lineHeightOverride = node.textStyleValue(StyleAttachmentKey.lineHeight) as Float?
        let resolvedFont = fontOverride ?? node.textScaler.scale(env.defaultFont)
        let resolvedLineHeight = env.resolvedLineHeight(font: resolvedFont, override: lineHeightOverride)
        return displayLayout(env: env, layout: node.layoutNode, font: resolvedFont,
                             lineHeight: resolvedLineHeight, maxWidth: maxWidth,
                             letterSpacing: node.textStyleValue(StyleAttachmentKey.letterSpacing) ?? 0).layout
    }

    // MARK: - Layout cache

    static func installMeasureFunc(on layout: LayoutNode, snapshot: Text) {
        layout.setMeasureFunc { [weak layout] width, widthMode, _, _ in
            guard let env = TextEnvironmentHolder.current else {
                return CGSize(width: 0, height: 0)
            }
            let constraint: Float = (widthMode == .undefined) ? .infinity : width
            let fontOverride = layout?.textStyleValue(StyleAttachmentKey.font) as Font?
            let lineHeightOverride = layout?.textStyleValue(StyleAttachmentKey.lineHeight) as Float?
            let resolvedFont = fontOverride ?? (layout?.textScaler ?? .identity).scale(env.defaultFont)
            let resolvedLineHeight = env.resolvedLineHeight(font: resolvedFont,
                                                            override: lineHeightOverride)
            let result = snapshot.displayLayout(
                env: env,
                layout: layout,
                font: resolvedFont,
                lineHeight: resolvedLineHeight,
                maxWidth: constraint,
                letterSpacing: layout?.textStyleValue(StyleAttachmentKey.letterSpacing) ?? 0
            )
            return CGSize(width: CGFloat(result.layout.totalWidth),
                          height: CGFloat(result.layout.totalHeight))
        }
    }

    /// Shape + layout cache. Phase 6: per-`LayoutNode` cache lives directly
    /// on `LayoutNode.textMeasure` (a typed stored property in Runtime).
    /// Falls through to the process-wide cache on `TextEnvironment` when no
    /// LayoutNode is supplied (unit tests without a host).
    static func cachedLayout(
        env: TextEnvironment,
        layout: LayoutNode?,
        text: String,
        font: Font,
        lineHeight: Float,
        maxWidth: Float,
        alignment: TextAlignment,
        letterSpacing: Float = 0
    ) -> TextLayoutResult {
        let normalizedMaxWidth: Float = (maxWidth.isFinite && maxWidth > 0) ? maxWidth : .infinity
        let key = TextLayoutCacheKey(
            text: text,
            font: font,
            lineHeight: lineHeight,
            alignment: alignment,
            maxWidth: normalizedMaxWidth,
            atlasID: ObjectIdentifier(env.atlas),
            letterSpacing: letterSpacing
        )
        if let layout, let cached = layout.textMeasure, cached.key == key {
            return cached.result
        }
        let result = env.cachedLayout(
            text: text,
            font: font,
            lineHeight: lineHeight,
            maxWidth: normalizedMaxWidth,
            alignment: alignment,
            letterSpacing: letterSpacing
        )
        if let layout {
            layout.textMeasure = TextLayoutCacheEntry(key: key, result: result)
        }
        return result
    }

    private func resolvedMaxWidth(_ maxWidth: Float) -> Float {
        guard let lineLimit, lineLimit == 1 else {
            return maxWidth
        }
        return .infinity
    }

    private func displayLayout(env: TextEnvironment, layout: LayoutNode?, font: Font,
                               lineHeight: Float, maxWidth: Float, letterSpacing: Float)
        -> (layout: TextLayoutResult, truncated: Bool) {
        let normalized = string.split(whereSeparator: \.isNewline).joined(separator: " ")
        let text = truncates ? SingleLineText.fitting(string, width: maxWidth) { candidate in
            env.cachedLayout(text: candidate, font: font, lineHeight: lineHeight,
                             letterSpacing: letterSpacing).totalWidth
        } : string
        let result = Self.cachedLayout(env: env, layout: layout, text: text, font: font,
                                       lineHeight: lineHeight, maxWidth: truncates ? maxWidth : resolvedMaxWidth(maxWidth),
                                       alignment: alignment, letterSpacing: letterSpacing)
        return (result, truncates && text != normalized)
    }
}

enum TextTruncation {
    static let attachmentKey = "__text_truncated"
}

extension Node {
    /// True when a text descendant had to clip its line. Set while drawing, so
    /// tooltip presentation reads the same fit the renderer used.
    var containsTruncatedText: Bool {
        if attachments[TextTruncation.attachmentKey] as? Bool == true { return true }
        return children.contains { $0.containsTruncatedText }
    }
}

// MARK: - Plain-text selection geometry

extension TextLayoutResult {
    /// UTF-8 byte offset of the caret nearest to a point, in the text block's
    /// local space (origin at its top-left).
    func offset(at point: CGPoint) -> Int {
        guard !lines.isEmpty else { return 0 }
        let px = Float(point.x), py = Float(point.y)
        let lineHeight = totalHeight / Float(lines.count)
        let index = lines.firstIndex(where: { py >= $0.topY && py < $0.topY + lineHeight })
            ?? (py < lines[0].topY ? 0 : lines.count - 1)
        let line = lines[index]
        let glyphs = line.glyphs.sorted { $0.x < $1.x }
        guard !glyphs.isEmpty else { return Int(line.startCluster) }
        if px <= glyphs[0].x { return Int(line.startCluster) }
        if px >= glyphs.last!.x { return Int(line.endCluster) }
        for i in 0..<glyphs.count - 1 {
            let a = glyphs[i], b = glyphs[i + 1]
            if px >= a.x, px < b.x {
                let mid = (a.x + b.x) / 2
                return Int(px < mid ? a.cluster : b.cluster)
            }
        }
        return Int(line.endCluster)
    }

    /// Highlight rectangles (local space) for a UTF-8 byte `range`.
    func selectionRects(_ range: Range<Int>) -> [CGRect] {
        guard !range.isEmpty else { return [] }
        let lineHeight = totalHeight / Float(max(1, lines.count))
        var rects: [CGRect] = []
        for line in lines {
            let ls = Int(line.startCluster), le = Int(line.endCluster)
            let lo = max(range.lowerBound, ls)
            let hi = min(range.upperBound, le)
            guard lo < hi else { continue }
            let glyphs = line.glyphs.sorted { $0.x < $1.x }
            let firstX: Float = glyphs.first(where: { Int($0.cluster) >= lo })?.x
                ?? (lo <= ls ? 0 : (glyphs.last?.x ?? 0))
            let lastGlyph = glyphs.last(where: { Int($0.cluster) < hi })
            let lastX: Float
            if let g = lastGlyph {
                lastX = glyphs.first(where: { $0.x > g.x })?.x ?? max(g.x, line.width)
            } else {
                lastX = glyphs.last?.x ?? line.width
            }
            rects.append(CGRect(x: CGFloat(firstX), y: CGFloat(line.topY),
                                width: CGFloat(max(0, lastX - firstX)), height: CGFloat(lineHeight)))
        }
        return rects
    }

    /// Zero-width caret rectangle (full line height) at a UTF-8 `offset`, in the
    /// text block's local space. Used to place the draggable selection handles.
    func caretRect(at offset: Int) -> CGRect {
        guard !lines.isEmpty else { return .zero }
        let lineHeight = totalHeight / Float(max(1, lines.count))
        for line in lines {
            let ls = Int(line.startCluster), le = Int(line.endCluster)
            if offset >= ls, offset <= le {
                let glyphs = line.glyphs.sorted { $0.x < $1.x }
                let x: Float = glyphs.first(where: { Int($0.cluster) >= offset })?.x
                    ?? (offset <= ls ? 0 : (glyphs.last?.x ?? line.width))
                return CGRect(x: CGFloat(x), y: CGFloat(line.topY), width: 0, height: CGFloat(lineHeight))
            }
        }
        let last = lines.last!
        let glyphs = last.glyphs.sorted { $0.x < $1.x }
        let x = offset <= Int(last.startCluster) ? 0 : (glyphs.last?.x ?? last.width)
        return CGRect(x: CGFloat(x), y: CGFloat(last.topY), width: 0, height: CGFloat(lineHeight))
    }
}

// Phase 6: TextLayoutCacheKey / TextLayoutCacheEntry / TextMeasureInputs
// moved to CoveUIRuntime (`Text/TextMeasureSlot.swift`) so the typed
// measure slot can live on `LayoutNode` directly.

/// Thin one-pixel separator. Renders as a coloured rect; defaults to a flexible
/// horizontal line when placed in a Column. When `color` is omitted, the
/// divider resolves `theme.colors.divider` from the active theme so themed
/// scopes paint a coherent rule colour without any plumbing at the call site.
public struct Divider: _PrimitiveView {
    public let color: Color?
    public let thickness: Float
    public let axis: Axis

    public enum Axis { case horizontal, vertical }

    public init(color: Color? = nil,
                thickness: Float = 1,
                axis: Axis = .horizontal) {
        self.color = color
        self.thickness = thickness
        self.axis = axis
    }

    public func _makeNode() -> Node {
        let n = Node()
        n.isHitTestable = false
        return n
    }

    public func _updateNode(_ node: Node) {
        node.animatableSet(\.backgroundColor, to: color ?? node.theme.colors.divider)
    }

    public func _makeLayoutNode() -> LayoutNode? { LayoutNode() }
    public func _updateLayout(_ layout: LayoutNode) {
        switch axis {
        case .horizontal:
            layout.height = thickness
            layout.setWidthPercent(100)
        case .vertical:
            layout.width = thickness
            layout.setHeightPercent(100)
        }
    }
}
