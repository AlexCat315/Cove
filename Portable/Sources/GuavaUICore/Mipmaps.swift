import Foundation

public enum ImageLOD {
    /// Nearest resolution level; bilinear filtering then operates near a
    /// one-to-one pixel footprint. Invalid dimensions always use the base image.
    public static func level(source: SIMD2<Float>, target: SIMD2<Float>, count: Int) -> Int {
        guard count > 0, source.x.isFinite, source.y.isFinite,
              target.x.isFinite, target.y.isFinite, target.x > 0, target.y > 0 else { return 0 }
        var ratio = min(source.x / target.x, source.y / target.y), level = 0
        while ratio > 1.414214 && level < count { ratio *= 0.5; level += 1 }
        return level
    }
}

public extension DecodedImage {
    /// Area filtering includes odd final rows/columns. Average premultiplied
    /// colors, then restore straight alpha to avoid fringes from transparent RGB.
    func reducedMip() -> DecodedImage? {
        guard width > 0, height > 0, width > 1 || height > 1,
              width <= 16_384, height <= 16_384, pixels.count == width * height * 4 else { return nil }
        let w = max(1, width / 2), h = max(1, height / 2)
        let dx = Double(width) / Double(w), dy = Double(height) / Double(h)
        var output = [UInt8](repeating: 0, count: w * h * 4)
        for y in 0..<h {
            let lowY = Double(y) * dy, highY = Double(y + 1) * dy
            for x in 0..<w {
                let lowX = Double(x) * dx, highX = Double(x + 1) * dx
                var red = 0.0, green = 0.0, blue = 0.0, alpha = 0.0, area = 0.0
                for sy in Int(lowY)..<min(height, Int(ceil(highY))) {
                    let wy = min(highY, Double(sy + 1)) - max(lowY, Double(sy))
                    for sx in Int(lowX)..<min(width, Int(ceil(highX))) {
                        let weight = wy * (min(highX, Double(sx + 1)) - max(lowX, Double(sx)))
                        let index = (sy * width + sx) * 4
                        let a = Double(pixels[index + 3]) * weight
                        red += Double(pixels[index]) * a
                        green += Double(pixels[index + 1]) * a
                        blue += Double(pixels[index + 2]) * a
                        alpha += a; area += weight
                    }
                }
                let index = (y * w + x) * 4
                if alpha > 0 {
                    output[index] = UInt8(clamping: Int((red / alpha).rounded()))
                    output[index + 1] = UInt8(clamping: Int((green / alpha).rounded()))
                    output[index + 2] = UInt8(clamping: Int((blue / alpha).rounded()))
                }
                output[index + 3] = UInt8(clamping: Int((alpha / max(area, 0.00001)).rounded()))
            }
        }
        return DecodedImage(pixels: output, width: w, height: h)
    }
}
