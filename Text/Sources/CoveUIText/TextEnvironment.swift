import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif
import CoveUICore
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
            shaper.setFace(primaryFont.face, size: defaultFont.size)
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

