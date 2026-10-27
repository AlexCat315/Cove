import NativeRHI

/// Reused attachment workspace. Hardware with memoryless attachments can keep
/// the entire frame on chip; other hardware rasterizes bounded horizontal strips.
/// Both paths use the same viewport, sample positions, geometry and resolve.
final class NativeUIRasterWorkspace {
    static let defaultBudget = 16 * 1024 * 1024
    let pixels: SIMD2<Int>
    let format: TextureFormat
    let samples: Int
    let budget: Int
    let multisample: TextureResource?
    let strip: TextureResource?
    var stripHeight: Int { multisample?.descriptor.height ?? pixels.y }
    var tiled: Bool { strip != nil }

    var logicalBytes: Int {
        guard let multisample else { return 0 }
        return multisample.descriptor.width * multisample.descriptor.height * (format == .rgba16Float ? 8 : 4) * samples
    }
    var estimatedBytes: Int { logicalBytes + (strip.map { $0.descriptor.width * $0.descriptor.height * (format == .rgba16Float ? 8 : 4) } ?? 0) }
    var allocatedBytes: Int? { multisample == nil ? 0 : multisample?.allocation?.allocatedBytes }
    var memoryless: Bool { multisample?.allocation?.isMemoryless ?? false }

    init(device: Device, pixels: SIMD2<Int>, format: TextureFormat, samples: Int,
         budget: Int = defaultBudget, preferMemoryless: Bool = true) throws {
        self.pixels = pixels; self.format = format; self.samples = samples; self.budget = budget
        guard samples > 1 else { multisample = nil; strip = nil; return }
        // Reserve at most the budget before probing actual storage. Unknown
        // allocation reporting takes the bounded path, never an assumed zero.
        let bytesPerPixel = format == .rgba16Float ? 8 : 4
        let height = max(1, min(pixels.y, budget / max(1, pixels.x * bytesPerPixel * samples)))
        func attachment(height: Int) throws -> TextureResource {
            try TextureResource(device: device, descriptor: TextureDescriptor(width: pixels.x, height: height,
                format: format, usage: .colorTarget, sampleCount: samples, label: "ui-raster-workspace",
                transientAttachment: preferMemoryless))
        }
        let bounded = try attachment(height: height)
        if bounded.allocation?.isMemoryless == true && height < pixels.y {
            multisample = try attachment(height: pixels.y)
            strip = nil
        } else {
            multisample = bounded
            strip = height < pixels.y ? try TextureResource(device: device,
                descriptor: TextureDescriptor(width: pixels.x, height: height, format: format,
                    usage: [.colorTarget, .sampled], label: "ui-strip-resolve")) : nil
        }
    }

    func target(output: Texture, load: ColorLoadAction) -> RenderColorTarget {
        var result = RenderColorTarget(texture: multisample?.texture ?? output,
            loadAction: load, store: multisample == nil)
        result.resolveTexture = multisample == nil ? nil : strip?.texture ?? output
        return result
    }
}
