import CoveUIScene
import CoveUIText
import CoveUIImaging
import CoveUIImaging
import NativeRHI

final class NativeTextureImageBacking: TextureImageBacking, @unchecked Sendable {
    let resource: TextureResource
    init(_ resource: TextureResource) { self.resource = resource }
}

public extension TextureImage {
    convenience init(texture: TextureResource, mipmaps: [TextureResource] = []) throws {
        let descriptor = texture.descriptor
        guard descriptor.dimension == .texture2D, descriptor.sampleCount == 1,
              descriptor.usage.contains(.sampled),
              [.rgba8Unorm, .bgra8Unorm].contains(descriptor.format) else {
            throw RHIError.invalidArgument("UI images require a sampled RGBA/BGRA 2D texture")
        }
        guard mipmaps.allSatisfy({ $0.device === texture.device && $0.descriptor.format == descriptor.format }) else {
            throw RHIError.invalidArgument("Image mip levels must use the same device and color format")
        }
        try self.init(backing: NativeTextureImageBacking(texture), pixelSize: SIMD2(descriptor.width, descriptor.height),
                      mipmaps: mipmaps.map { try TextureImage(texture: $0) })
    }
}
