import CoveUICore
import Foundation
#if canImport(CoreText)
import CoreText
#endif

/// A font resolved for rendering, with its runtime identity.
///
/// Face ownership and all native font handles live in `FontFace`. This type
/// carries only what the runtime needs on top of that: a stable font ID, the
/// requested size, and — on Apple platforms — a CoreText rasterizer for native
/// colour glyphs.
public final class ManagedFont {
    public let id: Int
    public let postScriptName: String
    public let pointSize: Float
    public let rasterScale: Float

    /// The text-layer face backing this font.
    public let face: FontFace

#if canImport(CoreText)
    /// CoreText rasterizer for native colour glyphs, when the face has them.
    ///
    /// Storage lives on the face, so whichever layer resolves the font decides
    /// how colour glyphs are rasterized.
    public var colorGlyphFont: NativeColorGlyphFont? {
        get { face.colorGlyphFont }
        set { face.colorGlyphFont = newValue }
    }
#endif

    init(id: Int, face: FontFace, pointSize: Float, rasterScale: Float) {
        self.id = id
        self.face = face
        self.postScriptName = face.postScriptName
        self.pointSize = pointSize
        self.rasterScale = rasterScale
    }

    /// The colour-glyph source to register with an atlas, if this face has one.
    var colorSource: ColorGlyphSource? { face.colorSource }
}

/// A contiguous text segment that should be shaped with a specific font.
public struct FontRun {
    public let font: ManagedFont
    public let text: String
    public let utf8Offset: Int
    /// Resolved bidi direction; `nil` lets HarfBuzz guess from the script.
    public var isRightToLeft: Bool?
}

/// Resolves system fonts and provides shaping for text runs.
///
/// Font loading, rasterization and shaping all live in `CoveUIText`. This type
/// only knows how to *find* fonts on the current platform — CoreText on Apple
/// platforms, a directory probe elsewhere — and therefore never binds FreeType
/// or HarfBuzz.
///
/// Like `FontAtlas`, an instance belongs to one render thread.
public final class FontProvider {
    private var fonts: [String: ManagedFont] = [:]
    private var nextFontID: Int
    private let size: Float
    private let rasterScale: Float
    private var primaryPSName: String?

#if canImport(CoreText)
    private var primaryCTFont: CTFont?
    private var styledFallbackFonts: [CTFont]?
    private var fallbackNamesByCluster: [String: String] = [:]
    private var failedFontNames: Set<String> = []
#else
    /// Lazily-loaded system fallback faces (CJK etc.) and the paths still to try.
    private var fallbackChain: [ManagedFont] = []
    private var triedFallbackPaths: Set<String> = []
    private lazy var fallbackFontPaths: [String] = Self.systemFallbackFontPaths()
#endif

    public init(size: Float, rasterScale: Float = 1, idBase: Int = 0) {
        self.size = size
        self.rasterScale = max(1, rasterScale)
        self.nextFontID = idBase
    }

    public var primaryFont: ManagedFont? {
        guard let name = primaryPSName else { return nil }
        return fonts[name]
    }

    public var allFonts: [ManagedFont] { Array(fonts.values) }

    /// Registers every resolved face with `atlas`.
    ///
    /// Faces with a colour source use it instead of the alpha plane, so colour
    /// glyphs never fall back to monochrome rasterization.
    public func registerAllFonts(in atlas: FontAtlas) {
        for font in fonts.values {
            if let source = font.colorSource {
                atlas.registerColorSource(source, fontID: font.id)
                continue
            }
            atlas.registerFace(font.face, fontID: font.id)
        }
    }

    /// Assigns a stable font ID and caches `face` under `psName` and aliases.
    private func register(_ face: FontFace, psName: String,
                          extraAliases: [String] = []) -> ManagedFont {
        let id = nextFontID
        nextFontID += 1
        let managed = ManagedFont(id: id, face: face, pointSize: size,
                                   rasterScale: rasterScale)
        let resolved = face.postScriptName.isEmpty ? psName : face.postScriptName
        for alias in Set([psName, resolved] + extraAliases) {
            fonts[alias] = managed
        }
        if primaryPSName == nil { primaryPSName = resolved }
        return managed
    }

    // MARK: - Primary font resolution

    @discardableResult
    public func loadPrimaryFont(name: String,
                                weight: FontWeight = .regular,
                                isItalic: Bool = false) -> ManagedFont? {
#if canImport(CoreText)
        failedFontNames.removeAll(keepingCapacity: true)
        let ctFont = configuredCTFont(named: name, weight: weight, isItalic: isItalic)
        let font = loadFont(ctFont)
        if let font {
            primaryPSName = font.postScriptName
            primaryCTFont = ctFont
            styledFallbackFonts = nil
            fallbackNamesByCluster.removeAll(keepingCapacity: true)
        }
        return font
#else
        return loadPrimaryFontDirect(name: name)
#endif
    }

    // MARK: - Font fallback

    /// Splits text into runs, each assigned the font that can render it.
    public func resolveRuns(text: String) -> [FontRun] {
#if canImport(CoreText)
        guard !text.isEmpty, let ctPrimary = primaryCTFont else { return [] }
        var result: [FontRun] = []

        let nsText = text as NSString
        var currentFont: ManagedFont?
        var currentPSName: String?
        var currentText = ""
        var currentUTF8Offset = 0
        var runningUTF8Offset = 0

        func flushCurrentRun() {
            guard let currentFont, !currentText.isEmpty else { return }
            result.append(FontRun(font: currentFont, text: currentText, utf8Offset: currentUTF8Offset))
            currentText = ""
        }

        nsText.enumerateSubstrings(in: NSRange(location: 0, length: nsText.length),
                                   options: .byComposedCharacterSequences) { substring, range, _, _ in
            guard let substring else { return }

            guard let managedFont = self.coveringManagedFont(for: substring,
                                                             in: text,
                                                             range: range,
                                                             primary: ctPrimary) else {
                runningUTF8Offset += substring.utf8.count
                return
            }
            let psName = managedFont.postScriptName

            if currentPSName != psName {
                flushCurrentRun()
                currentFont = managedFont
                currentPSName = psName
                currentUTF8Offset = runningUTF8Offset
            }

            currentText += substring
            runningUTF8Offset += substring.utf8.count
        }

        flushCurrentRun()

        return result
#else
        // Off Apple there is no CoreText cascade, so walk the text by grapheme
        // cluster and route each to the primary font or, when the primary lacks
        // the glyph (e.g. CJK in the Latin bundled face), a lazily-loaded system
        // fallback. Consecutive clusters sharing a font are merged into one run.
        guard !text.isEmpty,
              let primaryName = primaryPSName,
              let primary = fonts[primaryName] else { return [] }

        var result: [FontRun] = []
        var currentFont: ManagedFont?
        var currentText = ""
        var currentUTF8Offset = 0
        var runningUTF8Offset = 0

        func flushCurrentRun() {
            guard let currentFont, !currentText.isEmpty else { return }
            result.append(FontRun(font: currentFont, text: currentText, utf8Offset: currentUTF8Offset))
            currentText = ""
        }

        for character in text {
            let cluster = String(character)
            let font = fallbackFont(for: cluster, primary: primary)
            if currentFont?.id != font.id {
                flushCurrentRun()
                currentFont = font
                currentUTF8Offset = runningUTF8Offset
            }
            currentText += cluster
            runningUTF8Offset += cluster.utf8.count
        }

        flushCurrentRun()
        return result
#endif
    }

    // MARK: - Shaping

    /// Shapes one run. Colour faces route through their native rasterizer;
    /// everything else goes to the face's HarfBuzz shaper.
    public func shapeRun(_ run: FontRun) -> [ShapedGlyph] {
#if canImport(CoreText)
        if let colorFont = run.font.colorGlyphFont {
            return colorFont.shape(run.text, fontID: run.font.id, utf8Offset: run.utf8Offset)
        }
#endif
        let direction: TextDirection? = run.isRightToLeft.map { $0 ? .rtl : .ltr }
        return run.font.face.shape(run.text, direction: direction, script: nil,
                                        language: nil, utf8Offset: run.utf8Offset,
                                        fontID: run.font.id)
    }

    private func managedFontCanRenderText(_ font: ManagedFont, text: String) -> Bool {
        guard !text.isEmpty else { return true }
        return font.face.canRender(text)
    }

    // MARK: - Direct file loading (non-Apple)

#if !canImport(CoreText)
    private func loadPrimaryFontDirect(name: String) -> ManagedFont? {
        if name.hasPrefix("/"), let data = try? NSData(contentsOf: URL(fileURLWithPath: name), options: .mappedIfSafe) {
            return loadFontFromData(data, psName: name, faceIndex: 0)
        }
        // 1. If `name` happens to match a font file by basename, load it.
        if !name.isEmpty {
            let extensions = ["ttc", "ttf", "otf"]
            let systemDirs = [
                "/system/fonts/",
                "C:\\Windows\\Fonts\\",
                "/usr/share/fonts/",
                "/usr/share/fonts/truetype/dejavu/",
                "/usr/share/fonts/TTF/",
                "/usr/local/share/fonts/",
            ]
            for dir in systemDirs {
                for ext in extensions {
                    let path = dir + name + "." + ext
                    if let data = try? NSData(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe) {
                        return loadFontFromData(data, psName: name, faceIndex: 0)
                    }
                }
            }
        }
        // 2. Fall back to the platform's default UI font (Segoe UI on Windows,
        //    a common sans on Linux). The system pairs this with its own CJK
        //    fallback (e.g. YaHei) so mixed scripts stay visually consistent.
        for path in Self.systemPrimaryFontPaths() {
            if let data = try? NSData(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe) {
                return loadFontFromData(data, psName: name.isEmpty ? "system-ui" : name, faceIndex: 0)
            }
        }
        return nil
    }

    /// Path(s) to the platform's default UI font, probed in order.
    private static func systemPrimaryFontPaths() -> [String] {
#if os(Windows)
        let dir = "C:\\Windows\\Fonts\\"
        return [
            "segoeui.ttf",   // Segoe UI — the Windows shell UI font
            "tahoma.ttf",
            "arial.ttf",
        ].map { dir + $0 }
#elseif os(Android)
        return ["/system/fonts/Roboto-Regular.ttf", "/system/fonts/NotoSans-Regular.ttf"]
#elseif os(Linux)
        return [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ]
#else
        return []
#endif
    }

    private func loadFontFromData(_ data: NSData, psName: String, faceIndex: Int) -> ManagedFont? {
        if let existing = fonts[psName] { return existing }
        guard let face = FontFace(storage: data, size: size,
                                  rasterScale: rasterScale,
                                  selector: FontFaceSelector(faceIndex: faceIndex)) else {
            return nil
        }
        return register(face, psName: psName)
    }

    /// System faces that, together, cover CJK / common non-Latin scripts the
    /// primary lacks. Probed in order; the first that resolves a cluster wins.
    private static func systemFallbackFontPaths() -> [String] {
#if os(Windows)
        let dir = "C:\\Windows\\Fonts\\"
        return [
            "msyh.ttc",      // Microsoft YaHei — Simplified Chinese
            "msyhl.ttc",
            "simsun.ttc",    // SimSun
            "msjh.ttc",      // Microsoft JhengHei — Traditional Chinese
            "msgothic.ttc",  // MS Gothic — Japanese
            "yugothm.ttc",
            "malgun.ttf",    // Malgun Gothic — Korean
            "seguiemj.ttf",  // Segoe UI Emoji
            "arial.ttf",
        ].map { dir + $0 }
#elseif os(Android)
        let resources = ProcessInfo.processInfo.environment["PACKAGE_RESOURCE_BUNDLE_PATH"]
        return ["/system/fonts/NotoSansCJK-Regular.ttc", "/system/fonts/NotoSansSymbols-Regular-Subsetted.ttf",
                "/system/fonts/NotoSansSymbols-Regular-Subsetted2.ttf",
                "/system/fonts/NotoSansArabic-Regular.ttf", "/system/fonts/NotoSansDevanagari-Regular.ttf"]
            + (resources.map { [$0 + "/Fonts/NotoEmoji.ttf"] } ?? [])
#elseif os(Linux)
        return [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            // COLRv0 emoji render in color through FreeType; PNG strikes need libpng.
            "/usr/share/fonts/truetype/twemoji/TwemojiMozilla.ttf",
            "/usr/share/fonts/twemoji/TwemojiMozilla.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
#else
        return []
#endif
    }

    /// Resolves the face that should render `cluster`: the primary when it has
    /// the glyph, otherwise a system fallback (loaded on first need), or the
    /// primary as a last resort (renders .notdef rather than dropping text).
    private func fallbackFont(for cluster: String, primary: ManagedFont) -> ManagedFont {
        if managedFontCanRenderText(primary, text: cluster) { return primary }
        for font in fallbackChain where managedFontCanRenderText(font, text: cluster) {
            return font
        }
        for path in fallbackFontPaths where !triedFallbackPaths.contains(path) {
            triedFallbackPaths.insert(path)
            guard let font = loadFallbackFont(path: path) else { continue }
            fallbackChain.append(font)
            if managedFontCanRenderText(font, text: cluster) { return font }
        }
        return primary
    }

    private func loadFallbackFont(path: String) -> ManagedFont? {
        let cacheKey = "fallback:" + path
        if let existing = fonts[cacheKey] { return existing }
        guard let data = try? NSData(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe) else { return nil }
        return loadFontFromData(data, psName: cacheKey, faceIndex: 0)
    }
#endif

    // MARK: - Apple CoreText-based loading

#if canImport(CoreText)
    /// Design coordinates CoreText resolved for `font`, keyed by axis tag.
    ///
    /// A CoreText descriptor identifies a variable *instance*, not just a font
    /// file; FreeType opens that file at its defaults, so the resolved axes
    /// are transferred before shaping and atlas packing share the face.
    private func designCoordinates(from font: CTFont) -> [FontAxis.ID: Double] {
        let values = CTFontCopyVariation(font) as? [NSNumber: NSNumber] ?? [:]
        var coordinates: [FontAxis.ID: Double] = [:]
        for (key, value) in values {
            coordinates[FontAxis.ID(rawValue: key.uint32Value)] = value.doubleValue
        }
        return coordinates
    }

    private func loadFont(_ ctFont: CTFont) -> ManagedFont? {
        let psName = CTFontCopyPostScriptName(ctFont) as String
        if let existing = fonts[psName] { return existing }
        guard !failedFontNames.contains(psName) else { return nil }
        let font = loadFontFromCTFont(ctFont, psName: psName)
        if font == nil { failedFontNames.insert(psName) }
        return font
    }

    private func coveringManagedFont(for substring: String,
                                     in fullText: String,
                                     range: NSRange,
                                     primary: CTFont) -> ManagedFont? {
        if let managed = primaryFont, managedFontCanRenderText(managed, text: substring) {
            return managed
        }
        if let name = fallbackNamesByCluster[substring], let managed = fonts[name] {
            return managed
        }
        let logicalSize = CGFloat(size)

        // Stack families carry the primary's weight and slant, so bold or
        // italic text that falls back (CJK through PingFang) keeps its style.
        // They come before CoreText's own pick, which would switch bold CJK
        // to another family (Hiragino) than regular text uses.
        if styledFallbackFonts == nil {
            let traits = CTFontCopyTraits(primary) as? [CFString: Any] ?? [:]
            var styled: [CFString: Any] = [:]
            if let weight = traits[kCTFontWeightTrait] { styled[kCTFontWeightTrait] = weight }
            if let symbolic = traits[kCTFontSymbolicTrait] as? UInt32 {
                styled[kCTFontSymbolicTrait] = symbolic & CTFontSymbolicTraits.traitItalic.rawValue
            }
            styledFallbackFonts = SystemFontDefaults.fontStack.map { family in
                let descriptor = CTFontDescriptorCreateWithAttributes([
                    kCTFontFamilyNameAttribute: family, kCTFontTraitsAttribute: styled,
                ] as CFDictionary)
                return CTFontCreateWithFontDescriptor(descriptor, logicalSize, nil)
            }
        }
        var seenPostScriptNames: Set<String> = []
        for candidate in styledFallbackFonts ?? [] {
            let psName = CTFontCopyPostScriptName(candidate) as String
            guard seenPostScriptNames.insert(psName).inserted else { continue }
            guard let managedFont = loadFont(candidate) else { continue }
            if managedFontCanRenderText(managedFont, text: substring) {
                rememberFallback(managedFont, for: substring)
                return managedFont
            }
        }

        // Only consult the system cascade after the primary and styled stack
        // fail. Ordinary Latin and CJK text need no per-cluster font discovery.
        let direct = CTFontCreateForString(primary, fullText as CFString,
                                           CFRange(location: range.location, length: range.length))
        if let managed = loadFont(direct), managedFontCanRenderText(managed, text: substring) {
            return managed
        }

        // An unavailable glyph must still occupy its cluster and UTF-8
        // position. Shape .notdef in the primary face rather than dropping it.
        return primaryFont
    }

    // Styled stack choices and complete color-emoji clusters are independent
    // of surrounding text. Other system-cascade results remain contextual.
    private func rememberFallback(_ font: ManagedFont, for cluster: String) {
        guard cluster.utf8.count <= 256 else { return }
        if fallbackNamesByCluster.count >= 1_024 { fallbackNamesByCluster.removeAll(keepingCapacity: true) }
        fallbackNamesByCluster[cluster] = font.postScriptName
    }

    private func loadFontFromCTFont(_ ctFont: CTFont, psName: String) -> ManagedFont? {
        if let existing = fonts[psName] { return existing }

        if let loaded = makeManagedFont(from: ctFont, cacheAliases: [psName]) {
            return loaded
        }

        for candidateName in fallbackPostScriptCandidates(for: ctFont, requestedPSName: psName) {
            if let existing = fonts[candidateName] {
                fonts[psName] = existing
                return existing
            }

            let candidateCTFont = CTFontCreateWithName(candidateName as CFString,
                                                       CGFloat(size),
                                                       nil)
            if let loaded = makeManagedFont(from: candidateCTFont,
                                            cacheAliases: [psName, candidateName]) {
                return loaded
            }
        }

        return nil
    }

    private func makeManagedFont(from ctFont: CTFont,
                                 cacheAliases: [String]) -> ManagedFont? {
        let actualPSName = CTFontCopyPostScriptName(ctFont) as String
        if let existing = fonts[actualPSName] {
            for alias in cacheAliases { fonts[alias] = existing }
            return existing
        }

        let descriptor = CTFontCopyFontDescriptor(ctFont)
        guard let urlRef = CTFontDescriptorCopyAttribute(descriptor, kCTFontURLAttribute),
              let url = urlRef as? URL else { return nil }
        guard let data = try? NSData(contentsOf: url, options: .mappedIfSafe) else { return nil }

        let selector = FreeTypeLibrary.probeSelector(matchingPostScriptName: actualPSName, in: data)
            ?? FontFaceSelector(faceIndex: 0)

        guard let face = FontFace(storage: data, size: size,
                                   rasterScale: rasterScale,
                                   selector: selector,
                                   designCoordinates: designCoordinates(from: ctFont)) else {
            return nil
        }

#if canImport(CoreText)
        if face.hasColorGlyphs {
            face.colorGlyphFont = NativeColorGlyphFont(font: ctFont, size: size, rasterScale: rasterScale)
        }
#endif

        return register(face, psName: actualPSName, extraAliases: cacheAliases)
    }

    private func configuredCTFont(named name: String,
                                  weight: FontWeight,
                                  isItalic: Bool) -> CTFont {
        let logicalSize = CGFloat(size)
        let base: CTFont
        if name == ".AppleSystemUIFont" {
            base = CTFontCreateUIFontForLanguage(.system, logicalSize, nil)
                ?? CTFontCreateWithName("Helvetica Neue" as CFString, logicalSize, nil)
        } else {
            base = CTFontCreateWithName(name as CFString, logicalSize, nil)
        }
        let weighted = weight == .regular ? base : weightedCTFont(base, weight: weight, size: logicalSize)
        guard isItalic else { return weighted }
        // Faces without an italic style keep the upright face rather than failing.
        return CTFontCreateCopyWithSymbolicTraits(weighted, logicalSize, nil, .traitItalic, .traitItalic) ?? weighted
    }

    private func weightedCTFont(_ base: CTFont, weight: FontWeight, size logicalSize: CGFloat) -> CTFont {
        let attrs: [CFString: Any] = [
            kCTFontTraitsAttribute: [kCTFontWeightTrait: weight.coreTextWeight]
        ]
        let descriptor = CTFontDescriptorCreateCopyWithAttributes(
            CTFontCopyFontDescriptor(base),
            attrs as CFDictionary
        )
        return CTFontCreateWithFontDescriptor(descriptor, logicalSize, nil)
    }

    private func fallbackPostScriptCandidates(for ctFont: CTFont,
                                              requestedPSName: String) -> [String] {
        var candidates: [String] = []

        appendCandidate(normalizedFallbackPostScriptName(requestedPSName), to: &candidates)

        if let subfamily = CTFontCopyName(ctFont, kCTFontSubFamilyNameKey) as String? {
            let normalizedFamily = normalizedFallbackFamilyName(CTFontCopyFamilyName(ctFont) as String)
            let normalizedStyle = subfamily.replacingOccurrences(of: " ", with: "")
            if !normalizedFamily.isEmpty {
                let familyCandidate = normalizedStyle.isEmpty
                    ? normalizedFamily
                    : "\(normalizedFamily)-\(normalizedStyle)"
                appendCandidate(familyCandidate, to: &candidates)
            }
        }

        return candidates
    }

    private func normalizedFallbackPostScriptName(_ name: String) -> String {
        var normalized = name
        if normalized.hasPrefix(".") {
            normalized.removeFirst()
        }
        normalized = normalized.replacingOccurrences(of: "PingFangUIText", with: "PingFang")
        normalized = normalized.replacingOccurrences(of: "PingFangUI", with: "PingFang")
        return normalized
    }

    private func normalizedFallbackFamilyName(_ familyName: String) -> String {
        var normalized = familyName
        if normalized.hasPrefix(".") {
            normalized.removeFirst()
        }
        normalized = normalized.replacingOccurrences(of: "PingFang UI", with: "PingFang ")
        normalized = normalized.replacingOccurrences(of: " UI ", with: " ")
        normalized = normalized.replacingOccurrences(of: " UI", with: " ")
        normalized = normalized.replacingOccurrences(of: " ", with: "")
        return normalized
    }

    private func appendCandidate(_ candidate: String, to candidates: inout [String]) {
        guard !candidate.isEmpty, !candidates.contains(candidate) else { return }
        candidates.append(candidate)
    }
#endif
}
