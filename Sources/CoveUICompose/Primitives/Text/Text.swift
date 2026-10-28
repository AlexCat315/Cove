import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif
import CoveUIRuntime

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
            // Composite button labels and other containers supply inherited
            // foregrounds; standalone text falls back to the active theme.
            let baseColor = node.debugForegroundColor ?? snapshot.color ?? node.inheritedForegroundColor ?? node.theme.colors.onSurface
            let drawColor = baseColor.multipliedAlpha(node.opacity)
            list.addText(result.layout,
                         origin: (Float(origin.x) + (padding?.left ?? 0), Float(origin.y) + (padding?.top ?? 0)),
                         color: drawColor,
                         textureID: env.atlasTextureID,
                         atlas: env.atlas)
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
