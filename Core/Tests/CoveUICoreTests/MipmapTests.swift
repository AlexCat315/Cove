import Testing
import CoveUICore

struct MipmapTests {
    @Test func oddDimensionsIncludeTheLastPixel() {
        let image = DecodedImage(pixels: [255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255], width: 3, height: 1)
        #expect(image.reducedMip()?.pixels == [170, 0, 85, 255])
    }
    @Test func transparentColorsDoNotBleed() {
        let image = DecodedImage(pixels: [255, 0, 0, 0, 0, 0, 255, 255], width: 2, height: 1)
        #expect(image.reducedMip()?.pixels == [0, 0, 255, 128])
        #expect(ImageLOD.level(source: SIMD2(1920, 1080), target: SIMD2(480, 270), count: 10) == 2)
        #expect(ImageLOD.level(source: SIMD2(1920, 1080), target: SIMD2(.nan, 0), count: 10) == 0)
    }
}
