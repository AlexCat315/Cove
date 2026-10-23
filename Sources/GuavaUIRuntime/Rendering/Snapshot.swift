import Foundation

/// Atlas upload payload captured on the main thread and consumed by the render thread.
public struct DrawListAtlasDirty: Sendable {
    public var pixels: [UInt8]
    public var regionX: UInt32
    public var regionY: UInt32
    public var regionWidth: UInt32
    public var regionHeight: UInt32
    public var textureWidth: UInt32
    public var textureHeight: UInt32
    public var textureID: TextureID
    public var format: GlyphAtlasFormat = .alpha

    public init(pixels: [UInt8], regionX: UInt32, regionY: UInt32,
                regionWidth: UInt32, regionHeight: UInt32,
                textureWidth: UInt32, textureHeight: UInt32,
                textureID: TextureID, format: GlyphAtlasFormat = .alpha) {
        self.pixels = pixels
        self.regionX = regionX
        self.regionY = regionY
        self.regionWidth = regionWidth
        self.regionHeight = regionHeight
        self.textureWidth = textureWidth
        self.textureHeight = textureHeight
        self.textureID = textureID
        self.format = format
    }
}

/// Immutable snapshot of one frame's in-game UI draw commands, safe to pass
/// across the main→render thread boundary.
public struct DrawListSnapshot: Sendable {
    public var vertices: [UIVertex]
    public var indices: [UInt32]
    public var batches: [DrawBatch]
    public var logicalSize: SIMD2<Float>
    public var atlasUpdates: [DrawListAtlasDirty] = []
    public var resources = DrawListResources()

    public var isEmpty: Bool { batches.isEmpty }

    public init(vertices: [UIVertex], indices: [UInt32], batches: [DrawBatch],
                logicalSize: SIMD2<Float>,
                atlasUpdates: [DrawListAtlasDirty] = [], resources: DrawListResources = .init()) {
        self.vertices = vertices
        self.indices = indices
        self.batches = batches
        self.logicalSize = logicalSize
        self.atlasUpdates = atlasUpdates
        self.resources = resources
    }
}

