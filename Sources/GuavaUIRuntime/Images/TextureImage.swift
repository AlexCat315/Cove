public import NativeRHI

/// An owned image produced on the GPU. Draw lists retain it and renderers
/// establish residency automatically, using the same lifetime as file images.
public final class TextureImage: @unchecked Sendable, Equatable {
    public let textureID: TextureID
    public let pixelSize: SIMD2<Int>
    let resource: TextureResource
    public let mipmaps: [TextureImage]

    public init(texture: TextureResource, mipmaps: [TextureResource] = []) throws {
        let descriptor = texture.descriptor
        guard descriptor.dimension == .texture2D, descriptor.sampleCount == 1,
              descriptor.usage.contains(.sampled),
              [.rgba8Unorm, .bgra8Unorm].contains(descriptor.format) else {
            throw RHIError.invalidArgument("UI images require a sampled RGBA/BGRA 2D texture")
        }
        guard mipmaps.allSatisfy({ $0.device === texture.device && $0.descriptor.format == descriptor.format }) else {
            throw RHIError.invalidArgument("Image mip levels must use the same device and color format")
        }
        resource = texture
        pixelSize = SIMD2(descriptor.width, descriptor.height)
        textureID = try ImageAssetIDs.shared.allocate()
        self.mipmaps = try mipmaps.map { try TextureImage(texture: $0) }
    }

    public func resolution(for target: SIMD2<Float>, source: SIMD2<Float>? = nil) -> TextureImage {
        let size = source ?? SIMD2(Float(pixelSize.x), Float(pixelSize.y))
        let level = ImageLOD.level(source: size, target: target, count: mipmaps.count)
        return level == 0 ? self : mipmaps[level - 1]
    }

    public static func == (lhs: TextureImage, rhs: TextureImage) -> Bool { lhs === rhs }
}
