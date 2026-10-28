import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Foregrounds for text that communicates availability rather than content.
/// Nil inherits the palette's muted foreground.
public struct TextEmphasis: Sendable {
    public var placeholder: Color? = nil
    public var disabled: Color? = nil

    public init(_ configure: (inout Self) -> Void = { _ in }) { configure(&self) }
}
