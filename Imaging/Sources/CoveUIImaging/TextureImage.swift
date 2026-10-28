import CoveUICore

/// Backend-owned texture retained by draw-list snapshots.
public protocol TextureImageBacking: AnyObject, Sendable {}

public final class TextureImage: Sendable, Equatable {
    public let textureID: TextureID
    public let pixelSize: SIMD2<Int>
    public let backing: any TextureImageBacking
    public let mipmaps: [TextureImage]

    public init(backing: any TextureImageBacking, pixelSize: SIMD2<Int>, mipmaps: [TextureImage] = []) throws {
        guard pixelSize.x > 0, pixelSize.y > 0 else {
            throw ImageDecodeError.decodeFailure("invalid texture image dimensions")
        }
        self.backing = backing
        self.pixelSize = pixelSize
        self.mipmaps = mipmaps
        textureID = try ImageAssetIDs.shared.allocate()
    }

    public func resolution(for target: SIMD2<Float>, source: SIMD2<Float>? = nil) -> TextureImage {
        let size = source ?? SIMD2(Float(pixelSize.x), Float(pixelSize.y))
        let level = ImageLOD.level(source: size, target: target, count: mipmaps.count)
        return level == 0 ? self : mipmaps[level - 1]
    }

    public static func == (lhs: TextureImage, rhs: TextureImage) -> Bool { lhs === rhs }
}
