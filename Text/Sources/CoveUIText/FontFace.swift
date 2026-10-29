import CoveUICore
internal import CFreeType
internal import CHarfBuzz
import Foundation

/// A four-byte ISO 15924 script tag, big-endian (`'Latn' == 0x4C61746E`).
///
/// Shaping takes a raw tag rather than HarfBuzz's `hb_script_t` so that no
/// module outside `CoveUIText` ever has to import `CHarfBuzz`.
public typealias ScriptTag = UInt32

/// Common ISO 15924 script tags, spelled out so callers never need HarfBuzz.
public enum Script {
    /// `'Latn'`.
    public static let latin: ScriptTag = 0x4C61746E
    /// `'Zyyy'` — punctuation and other characters shared across scripts.
    public static let common: ScriptTag = 0x5A797979
    /// `'Zinh'` — combining marks that inherit their base character's script.
    public static let inherited: ScriptTag = 0x5A696E68
    /// `'Zzzz'` — codepoints with no known script.
    public static let unknown: ScriptTag = 0x5A7A7A7A
}

/// One design axis of a variable font, in normalized design units.
public struct FontAxis: Hashable, Sendable {
    public struct ID: Hashable, Sendable {
        public let rawValue: UInt32
        public init(rawValue: UInt32) { self.rawValue = rawValue }
    }

    public let id: ID
    public let name: String
    public let minimum: Double
    public let defaultValue: Double
    public let maximum: Double

    public init(id: ID, name: String, minimum: Double, defaultValue: Double, maximum: Double) {
        self.id = id
        self.name = name
        self.minimum = minimum
        self.defaultValue = defaultValue
        self.maximum = maximum
    }
}

/// A rasterized glyph together with the metrics layout needs for it.
public struct GlyphBitmap {
    public let pixels: [UInt8]
    public let width: Int
    public let height: Int
    public let metrics: GlyphMetrics
    public let format: GlyphAtlasFormat

    public init(pixels: [UInt8], width: Int, height: Int, metrics: GlyphMetrics, format: GlyphAtlasFormat) {
        self.pixels = pixels
        self.width = width
        self.height = height
        self.metrics = metrics
        self.format = format
    }
}

/// A selector identifying one face inside a font file.
///
/// The low 16 bits are the collection face index; the high 16 bits are a
/// 1-based named-instance index for variable fonts. CoreText's descriptor
/// array lists named instances, and its index is *not* a FreeType collection
/// face index, so both must travel together.
public struct FontFaceSelector: Hashable, Sendable {
    public let rawValue: Int

    public init(faceIndex: Int, namedInstance: Int = 0) {
        rawValue = (faceIndex & 0xFFFF) | ((namedInstance & 0xFFFF) << 16)
    }

    public var faceIndex: Int { rawValue & 0xFFFF }
    public var namedInstance: Int { (rawValue >> 16) & 0xFFFF }
}

/// The process-wide FreeType library handle.
///
/// FreeType libraries are long-lived by design. `FontAtlas` and the desktop
/// runtime's font provider used to each own one, which is what forced both
/// manifests to declare FreeType and HarfBuzz. A single shared handle removes
/// that coupling: only `CoveUIText` binds them, everyone else consumes
/// `FontFace`.
enum FreeTypeLibrary {
    nonisolated(unsafe) static let shared: FT_Library = {
        var handle: FT_Library?
        let status = FT_Init_FreeType(&handle)
        precondition(status == 0, "FT_Init_FreeType failed: \(status)")
        return handle!
    }()

    /// Opens a face from immutable storage.
    ///
    /// The storage must outlive the face: FreeType borrows its bytes rather
    /// than copying them.
    static func makeFace(storage: NSData, selector: FontFaceSelector) -> FT_Face? {
        var face: FT_Face?
        let status = FT_New_Memory_Face(
            shared,
            storage.bytes.assumingMemoryBound(to: UInt8.self),
            FT_Long(storage.length),
            FT_Long(selector.faceIndex),
            &face
        )
        guard status == 0, let face else { return nil }
        if selector.namedInstance > 0 {
            _ = FT_Set_Named_Instance(face, FT_UInt(selector.namedInstance))
        }
        _ = FT_Select_Charmap(face, FT_ENCODING_UNICODE)
        return face
    }

    /// Reads a face's PostScript name, falling back to an empty string.
    static func resolvedPostScriptName(of face: FT_Face) -> String {
        guard let pointer = FT_Get_Postscript_Name(face) else { return "" }
        return String(cString: pointer)
    }

    /// Reads the variable axes of an already opened face.
    static func designAxes(of face: FT_Face) -> [FontAxis] {
        guard face.pointee.face_flags & FT_Long(FT_FACE_FLAG_MULTIPLE_MASTERS) != 0 else { return [] }
        var descriptor: UnsafeMutablePointer<FT_MM_Var>?
        guard FT_Get_MM_Var(face, &descriptor) == 0, let descriptor else { return [] }
        defer { FT_Done_MM_Var(shared, descriptor) }

        let axes = UnsafeBufferPointer(start: descriptor.pointee.axis,
                                       count: Int(descriptor.pointee.num_axis))
        return axes.map { axis in
            FontAxis(
                id: FontAxis.ID(rawValue: UInt32(axis.tag)),
                name: withUnsafePointer(to: axis.name) { pointer in
                    // FT_Var_Axis.name is a `FT_String*` (nul-terminated, may be nil).
                    guard let raw = pointer.pointee else { return "" }
                    return String(cString: raw)
                },
                minimum: Double(axis.minimum) / 65_536,
                defaultValue: Double(axis.def) / 65_536,
                maximum: Double(axis.maximum) / 65_536
            )
        }
    }

    /// Resolves the selector whose PostScript name matches `targetPSName`.
    ///
    /// Named variable instances are encoded into the high bits, because
    /// CoreText's descriptor index does not map onto a FreeType face index.
    static func probeSelector(matchingPostScriptName targetPSName: String, in data: NSData) -> FontFaceSelector? {
        let probe = data.bytes.assumingMemoryBound(to: UInt8.self)
        var face: FT_Face?
        let status = FT_New_Memory_Face(shared, probe, FT_Long(data.length), 0, &face)
        guard status == 0, let face else { return nil }
        let numFaces = Int(face.pointee.num_faces)
        FT_Done_Face(face)

        for index in 0..<numFaces {
            var candidate: FT_Face?
            guard FT_New_Memory_Face(shared, probe, FT_Long(data.length),
                                     FT_Long(index), &candidate) == 0,
                  let candidate else { continue }
            defer { FT_Done_Face(candidate) }

            if let name = FT_Get_Postscript_Name(candidate),
               String(cString: name) == targetPSName { return FontFaceSelector(faceIndex: index) }

            var variation: UnsafeMutablePointer<FT_MM_Var>?
            guard FT_Get_MM_Var(candidate, &variation) == 0, let variation else { continue }
            defer { FT_Done_MM_Var(shared, variation) }

            for instance in 1...max(1, Int(variation.pointee.num_namedstyles)) {
                guard FT_Set_Named_Instance(candidate, FT_UInt(instance)) == 0 else { continue }
                if let name = FT_Get_Postscript_Name(candidate),
                   String(cString: name) == targetPSName {
                    return FontFaceSelector(faceIndex: index, namedInstance: instance)
                }
            }
        }
        return nil
    }
}

/// An owned font face: FreeType for outlines and rasterization, HarfBuzz for
/// shaping.
///
/// Consumers use Swift font values while native handles remain inside Text.
/// FreeType and HarfBuzz are declared once by the Text package; consumers still
/// inherit those deployment requirements through their dependency closure.
///
/// Each instance owns its backing storage and belongs to one render thread.
public final class FontFace {
    internal let ftFace: FT_Face
    internal let hbFont: OpaquePointer

    /// Immutable file storage; FreeType borrows these bytes for the face's lifetime.
    private let storage: NSData

    public let postScriptName: String
    public private(set) var size: Float
    public private(set) var rasterScale: Float

    /// The selector this face was opened with.
    private let selector: FontFaceSelector

    /// Design coordinates currently applied to a variable face.
    private let designCoordinates: [FontAxis.ID: Double]

#if canImport(CoreText)
    /// CoreText rasterizer for native colour glyphs; attached by the host,
    /// which owns the `CTFont` this face was resolved from.
    public var colorGlyphFont: NativeColorGlyphFont?
#endif

    /// COLR / bitmap-strike colour glyphs rasterized by FreeType, used where
    /// no native rasterizer exists.
    private var freeTypeColorFace: FreeTypeColorGlyphFace?

    /// Whether this face provides colour glyph rasterization.
    public var hasColorSource: Bool { colorSource != nil }

    /// The colour-glyph source to register with an atlas, if this face has one.
    public var colorSource: ColorGlyphSource? {
#if canImport(CoreText)
        if let colorGlyphFont { return colorGlyphFont.source }
#endif
        return freeTypeColorFace?.source
    }

    /// The collection face index this face was opened from.
    public var faceIndex: Int { selector.faceIndex }

    private init(ftFace: FT_Face,
                 hbFont: OpaquePointer,
                 storage: NSData,
                 postScriptName: String,
                 size: Float,
                 rasterScale: Float,
                 selector: FontFaceSelector,
                 designCoordinates: [FontAxis.ID: Double]) {
        self.ftFace = ftFace
        self.hbFont = hbFont
        self.storage = storage
        self.postScriptName = postScriptName
        self.size = size
        self.rasterScale = rasterScale
        self.selector = selector
        self.designCoordinates = designCoordinates
        self.freeTypeColorFace = nil
        applyDesignCoordinates()
        hb_ft_font_changed(hbFont)
#if !canImport(CoreText)
        self.freeTypeColorFace = FreeTypeColorGlyphFace(face: ftFace, size: size, rasterScale: rasterScale)
#endif
    }

    deinit {
        hb_font_destroy(hbFont)
        FT_Done_Face(ftFace)
    }

    // MARK: - Construction

    /// Loads a face from an in-memory font file.
    ///
    /// - Parameters:
    ///   - bytes: Raw contents of a .ttf/.otf/.ttc file.
    ///   - size: Requested size in logical points.
    ///   - rasterScale: Physical-to-logical pixel ratio (1 for a 1x display).
    ///   - selector: Which face / named instance to open.
    ///   - designCoordinates: Axis values to apply to a variable face.
    public convenience init?(bytes: [UInt8], size: Float, rasterScale: Float = 1,
                             selector: FontFaceSelector = FontFaceSelector(faceIndex: 0),
                             designCoordinates: [FontAxis.ID: Double] = [:]) {
        guard !bytes.isEmpty else { return nil }
        self.init(storage: NSData(bytes: bytes, length: bytes.count), size: size,
                  rasterScale: rasterScale, selector: selector, designCoordinates: designCoordinates)
    }

    /// Retain immutable file storage directly; iterating NSData as a byte
    /// sequence performs an Objective-C range lookup for every byte.
    internal convenience init?(storage: NSData, size: Float, rasterScale: Float = 1,
                               selector: FontFaceSelector = FontFaceSelector(faceIndex: 0),
                               designCoordinates: [FontAxis.ID: Double] = [:]) {
        guard storage.length > 0, size.isFinite, size > 0, size <= 512,
              rasterScale.isFinite, rasterScale >= 1, rasterScale <= 4 else { return nil }
        guard let face = FreeTypeLibrary.makeFace(storage: storage, selector: selector) else { return nil }
        let scale = max(1, rasterScale)
        FT_Set_Char_Size(face, 0, FT_F26Dot6(size * scale * 64), 72, 72)
        guard let hbFont = hb_ft_font_create_referenced(face) else {
            FT_Done_Face(face)
            return nil
        }
        hb_ft_font_set_load_flags(hbFont, FT_Int32(FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP))
        self.init(ftFace: face, hbFont: hbFont, storage: storage,
                  postScriptName: FreeTypeLibrary.resolvedPostScriptName(of: face), size: size,
                  rasterScale: scale, selector: selector,
                  designCoordinates: designCoordinates)
    }

    /// Loads a face from a font file on disk.
    public convenience init?(path: String, size: Float, rasterScale: Float = 1,
                             selector: FontFaceSelector = FontFaceSelector(faceIndex: 0),
                             designCoordinates: [FontAxis.ID: Double] = [:]) {
        guard let data = try? NSData(contentsOf: URL(fileURLWithPath: path),
                                     options: .mappedIfSafe) else { return nil }
        self.init(storage: data, size: size, rasterScale: rasterScale,
                  selector: selector, designCoordinates: designCoordinates)
    }

    /// Reopens the same storage as a fresh face, used to apply design
    /// coordinates after the fact.
    private static func reopening(storage: NSData, size: Float, rasterScale: Float,
                                  selector: FontFaceSelector,
                                  designCoordinates: [FontAxis.ID: Double]) -> FontFace? {
        guard let face = FreeTypeLibrary.makeFace(storage: storage, selector: selector) else { return nil }
        FT_Set_Char_Size(face, 0, FT_F26Dot6(size * rasterScale * 64), 72, 72)
        guard let hbFont = hb_ft_font_create_referenced(face) else {
            FT_Done_Face(face)
            return nil
        }
        hb_ft_font_set_load_flags(hbFont, FT_Int32(FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP))
        return FontFace(ftFace: face, hbFont: hbFont, storage: storage,
                        postScriptName: FreeTypeLibrary.resolvedPostScriptName(of: face), size: size,
                        rasterScale: rasterScale, selector: selector,
                        designCoordinates: designCoordinates)
    }

    // MARK: - Variable axes

    /// The variable axes this face exposes, or an empty array for a static face.
    public var designAxes: [FontAxis] { FreeTypeLibrary.designAxes(of: ftFace) }

    /// The design coordinates currently applied, keyed by axis tag.
    ///
    /// Exposed so hosts and tests can verify which instance a face was opened
    /// at without reaching for the native handle.
    public var appliedDesignCoordinates: [FontAxis.ID: Double] { designCoordinates }

    /// Returns a new face with `axes` applied, or `nil` when the face is not
    /// variable or the write fails.
    ///
    /// Design coordinates are in each axis' own normalized units and are
    /// clamped to the axis' supported range. Optical size stays in logical
    /// points, independent of the backing display's raster density.
    public func resolved(axes: [FontAxis.ID: Double]) -> FontFace? {
        guard !axes.isEmpty else { return self }
        let merged = designCoordinates.merging(axes) { _, requested in requested }
        return Self.reopening(storage: storage, size: size, rasterScale: rasterScale,
                              selector: selector, designCoordinates: merged)
    }

    /// True when this face carries colour glyph outlines or strikes.
    public var hasColorGlyphs: Bool {
        ftFace.pointee.face_flags & FT_Long(FT_FACE_FLAG_COLOR) != 0
    }

    /// Applies a new active size to this face.
    ///
    /// FreeType holds one active size per face, so this is visible to every
    /// consumer. Atlas caches are keyed by size, so callers re-register the
    /// face afterwards to keep cache keys in step with the face.
    @discardableResult
    public func applying(size: Float, rasterScale: Float = 1) -> Bool {
        guard size.isFinite, size > 0, size <= 512,
              rasterScale.isFinite, rasterScale >= 1, rasterScale <= 4 else { return false }
        let scale = max(1, rasterScale)
        guard FT_Set_Char_Size(ftFace, 0, FT_F26Dot6(size * scale * 64), 72, 72) == 0 else { return false }
        self.size = size
        self.rasterScale = scale
        // HarfBuzz caches the FreeType size and advances. Updating only FT
        // leaves shaping at the previous size while atlas metrics use the new one.
        hb_ft_font_changed(hbFont)
#if !canImport(CoreText)
        freeTypeColorFace = FreeTypeColorGlyphFace(face: ftFace, size: size, rasterScale: scale)
#endif
        return true
    }

    // MARK: - Glyphs

    /// The glyph index for `scalar`, or 0 when the face has no glyph for it.
    ///
    /// Used for font fallback: a face that maps every scalar of a grapheme
    /// cluster can render that cluster.
    public func glyphIndex(for scalar: Unicode.Scalar) -> UInt32 {
        FT_Get_Char_Index(ftFace, FT_ULong(scalar.value))
    }

    /// Whether this face can render `text` in its entirety.
    public func canRender(_ text: String) -> Bool {
        text.unicodeScalars.allSatisfy { glyphIndex(for: $0) != 0 }
    }

    /// Metrics for a glyph, forcing rasterization only when necessary.
    public func metrics(glyph: UInt32) -> GlyphMetrics? {
        guard let slot = loadSlot(glyph: glyph) else { return nil }
        return Self.metrics(of: slot, rasterScale: rasterScale)
    }

    /// Ascent, descent and line height scaled to logical points.
    public func lineMetrics() -> GlyphLineMetrics {
        guard let metrics = ftFace.pointee.size?.pointee.metrics else {
            return GlyphLineMetrics(ascent: size * 0.8, descent: size * 0.2, lineHeight: size * 1.2)
        }
        let unit = 1 / rasterScale
        let ascent = Float(metrics.ascender) / 64 * unit
        let descent = Float(-metrics.descender) / 64 * unit
        let height = Float(metrics.height) / 64 * unit
        return GlyphLineMetrics(ascent: ascent, descent: descent,
                                lineHeight: max(height, ascent + descent))
    }

    /// Rasterizes `glyph` and returns its pixels plus logical metrics.
    public func rasterize(glyph: UInt32) -> GlyphBitmap? {
        guard let slot = loadSlot(glyph: glyph) else { return nil }
        return Self.bitmap(of: slot, rasterScale: rasterScale)
    }

    // MARK: - Shaping

    /// Shapes `text` with HarfBuzz and returns positioned glyphs.
    ///
    /// - Parameters:
    ///   - text: The string to shape.
    ///   - direction: Writing direction; `nil` lets HarfBuzz infer it.
    ///   - script: ISO 15924 tag; `nil` lets HarfBuzz infer it.
    ///   - language: BCP 47 tag; `nil` uses HarfBuzz's default.
    ///   - utf8Offset: Byte offset of `text` within the enclosing paragraph,
    ///     added to each cluster so hosts can map glyphs back to source text.
    ///   - fontID: Identifier stamped on every returned glyph.
    public func shape(_ text: String, direction: TextDirection? = nil,
                      script: ScriptTag? = nil, language: String? = nil,
                      utf8Offset: Int = 0, fontID: Int = 0) -> [ShapedGlyph] {
        guard let buffer = hb_buffer_create() else { return [] }
        defer { hb_buffer_destroy(buffer) }

        text.withCString(encodedAs: UTF8.self) { pointer in
            hb_buffer_add_utf8(buffer, pointer, Int32(text.utf8.count), 0, Int32(text.utf8.count))
        }
        if let direction { hb_buffer_set_direction(buffer, direction.hbValue) }
        if let script { hb_buffer_set_script(buffer, hb_script_t(rawValue: .init(truncatingIfNeeded: script))) }
        if let language {
            language.withCString { pointer in
                hb_buffer_set_language(buffer, hb_language_from_string(pointer, Int32(language.utf8.count)))
            }
        }

        hb_buffer_guess_segment_properties(buffer)
        hb_shape(hbFont, buffer, nil, 0)

        var glyphCount: UInt32 = 0
        guard let infos = hb_buffer_get_glyph_infos(buffer, &glyphCount),
              let positions = hb_buffer_get_glyph_positions(buffer, &glyphCount) else { return [] }

        let scale = max(rasterScale, 1)
        var result: [ShapedGlyph] = []
        result.reserveCapacity(Int(glyphCount))

        for index in 0..<Int(glyphCount) {
            let info = infos[index]
            let position = positions[index]
            result.append(ShapedGlyph(
                glyphID: info.codepoint,
                xOffset: Float(position.x_offset) / 64.0 / scale,
                yOffset: Float(position.y_offset) / 64.0 / scale,
                xAdvance: Float(position.x_advance) / 64.0 / scale,
                yAdvance: Float(position.y_advance) / 64.0 / scale,
                cluster: info.cluster + UInt32(utf8Offset),
                fontID: fontID
            ))
        }
        return result
    }

    // MARK: - Internals

    /// Writes the stored design coordinates into the FreeType face.
    ///
    /// A CoreText descriptor identifies a variable *instance*, not just a font
    /// file; FreeType opens that file at its defaults, so the resolved axes are
    /// transferred before HarfBuzz and the atlas share the face. Optical size
    /// stays in logical points, independent of raster density.
    private func applyDesignCoordinates() {
        guard !designCoordinates.isEmpty else { return }
        guard ftFace.pointee.face_flags & FT_Long(FT_FACE_FLAG_MULTIPLE_MASTERS) != 0 else { return }
        var descriptor: UnsafeMutablePointer<FT_MM_Var>?
        guard FT_Get_MM_Var(ftFace, &descriptor) == 0, let descriptor else { return }
        defer { FT_Done_MM_Var(FreeTypeLibrary.shared, descriptor) }

        let axes = UnsafeBufferPointer(start: descriptor.pointee.axis,
                                       count: Int(descriptor.pointee.num_axis))
        var coordinates = axes.map { axis -> FT_Fixed in
            guard let value = designCoordinates[FontAxis.ID(rawValue: UInt32(axis.tag))],
                  value.isFinite else { return axis.def }
            let fixed = (value * 65_536).rounded()
            return FT_Fixed(min(Double(axis.maximum), max(Double(axis.minimum), fixed)))
        }
        guard !coordinates.isEmpty else { return }
        _ = FT_Set_Var_Design_Coordinates(ftFace, descriptor.pointee.num_axis, &coordinates)
    }

    private func loadSlot(glyph: UInt32) -> FT_GlyphSlot? {
        // FT_LOAD_TARGET_LIGHT = (1 & 15) << 16: lighter vertical hinting, smoother
        //   diagonal strokes at small sizes.
        // FT_LOAD_FORCE_AUTOHINT = 1 << 5: FreeType's autohinter, which generally
        //   beats native hints for screen-rendered tool chrome (12-14 px).
        let flags = FT_Int32(FT_LOAD_RENDER) | (1 << 5) | (1 << 16)
        guard FT_Load_Glyph(ftFace, FT_UInt(glyph), flags) == 0 else { return nil }
        return ftFace.pointee.glyph
    }

    private static func metrics(of slot: FT_GlyphSlot, rasterScale: Float) -> GlyphMetrics {
        let scale = max(rasterScale, 1)
        let bitmap = slot.pointee.bitmap
        return GlyphMetrics(
            glyphIndex: slot.pointee.glyph_index,
            width: Float(bitmap.width) / scale,
            height: Float(bitmap.rows) / scale,
            bearingX: Float(slot.pointee.bitmap_left) / scale,
            bearingY: Float(slot.pointee.bitmap_top) / scale,
            advance: Float(slot.pointee.advance.x) / 64.0 / scale
        )
    }

    private static func bitmap(of slot: FT_GlyphSlot, rasterScale: Float) -> GlyphBitmap {
        let bitmap = slot.pointee.bitmap
        let width = Int(bitmap.width)
        let height = Int(bitmap.rows)
        let metrics = Self.metrics(of: slot, rasterScale: rasterScale)
        var pixels = [UInt8]()
        if let buffer = bitmap.buffer, width > 0, height > 0 {
            let pitch = Int(bitmap.pitch)
            pixels = [UInt8](repeating: 0, count: width * height)
            for row in 0..<height {
                let source = row * pitch
                let target = row * width
                for column in 0..<width {
                    pixels[target + column] = buffer[source + column]
                }
            }
        }
        return GlyphBitmap(pixels: pixels, width: width, height: height,
                           metrics: metrics, format: .alpha)
    }
}
