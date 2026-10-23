public import NativeRHI

/// An owned image produced on the GPU. Draw lists retain it and renderers
/// establish residency automatically, using the same lifetime as file images.
public final class TextureImage: @unchecked Sendable, Equatable {
    public let textureID: TextureID
    public let pixelSize: SIMD2<Int>
    let resource: TextureResource

    public init(texture: TextureResource) throws {
        let descriptor = texture.descriptor
        guard descriptor.dimension == .texture2D, descriptor.sampleCount == 1,
              descriptor.usage.contains(.sampled),
              [.rgba8Unorm, .bgra8Unorm].contains(descriptor.format) else {
            throw RHIError.invalidArgument("UI images require a sampled RGBA/BGRA 2D texture")
        }
        resource = texture
        pixelSize = SIMD2(descriptor.width, descriptor.height)
        textureID = try ImageAssetIDs.shared.allocate()
    }

    public static func == (lhs: TextureImage, rhs: TextureImage) -> Bool { lhs === rhs }
}
