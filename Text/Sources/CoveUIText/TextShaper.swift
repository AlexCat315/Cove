import CoveUICore
internal import CHarfBuzz

/// Text direction for shaping.
public enum TextDirection {
    case ltr, rtl, ttb, btt

    var hbValue: hb_direction_t {
        switch self {
        case .ltr: return HB_DIRECTION_LTR
        case .rtl: return HB_DIRECTION_RTL
        case .ttb: return HB_DIRECTION_TTB
        case .btt: return HB_DIRECTION_BTT
        }
    }
}

/// Shapes text with HarfBuzz through a face owned by `CoveUIText`.
///
/// The shaper holds no native handles of its own. It delegates to `FontFace`,
/// which is the only type in the tree that owns an `hb_font_t` — that is what
/// lets callers such as the desktop runtime work with text without binding
/// HarfBuzz themselves.
public final class TextShaper {
    private var face: FontFace?

    public init() {}

    /// Associates a face with this shaper.
    ///
    /// - Parameters:
    ///   - face: A face loaded by `CoveUIText`.
    ///   - size: Font size in logical points.
    public func setFace(_ face: FontFace, size: Float) {
        self.face = face
    }

    /// Shapes a string of text and returns positioned glyphs.
    ///
    /// - Parameters:
    ///   - text: The input string.
    ///   - direction: Writing direction (default LTR); `nil` lets HarfBuzz guess.
    ///   - script: ISO 15924 tag (default Latin); `nil` lets HarfBuzz guess.
    ///   - language: BCP 47 tag (default "en"); `nil` uses HarfBuzz's default.
    /// - Returns: Array of shaped glyphs with positions.
    public func shape(
        text: String,
        direction: TextDirection? = .ltr,
        script: ScriptTag? = Script.latin,
        language: String? = "en"
    ) -> [ShapedGlyph] {
        face?.shape(text, direction: direction, script: script, language: language) ?? []
    }
}
