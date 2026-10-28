/// Backend-reported allocation for one texture, excluding replacement resources
/// still retained by in-flight work. This is resource allocation, not process RSS.
public struct TextureAllocation: Sendable, Equatable {
    public let allocatedBytes: Int
    public let isMemoryless: Bool
    public init(allocatedBytes: Int, isMemoryless: Bool) {
        self.allocatedBytes = allocatedBytes
        self.isMemoryless = isMemoryless
    }
}

/// Optional backend reporting. Missing support must stay unknown, rather than zero.
public protocol TextureMemoryReporting: AnyObject {
    func textureAllocation(_ texture: Texture) -> TextureAllocation?
}
