#if os(macOS)
import Foundation
import NativeRHI
import Testing
@testable import CoveUIRuntime
@testable import CoveUIGraphics
@testable import CoveUIImaging
@testable import CoveUIText

@Suite("Circular reveal pixels", .serialized)
struct CircularRevealPixelTests {
    @Test("The first frame holds the old theme, the new theme grows from the origin, and a reversal retains the mixed frame", arguments: [1, 2], [1, 4, 0])
    func revealAndReverse(scale: Int, mode: Int) throws {
        let context = try NativeUIDrawTestContext()
        let viewport = NativeUIViewport(pixels: SIMD2(64 * scale, 64 * scale), logical: SIMD2(64, 64))
        let output = try context.nativeOutput(format: .bgra8Unorm, size: viewport.pixels)
        try context.native.configure(format: .bgra8Unorm, sampleCount: mode == 1 ? 1 : 4)
        let raster = mode == 0 ? try NativeUIRasterWorkspace(device: context.device, pixels: viewport.pixels,
            format: .bgra8Unorm, samples: 4, budget: viewport.pixels.x * 4 * 4 * 13, preferMemoryless: false) : nil
        func render(_ color: Color, reveal: UIReveal? = nil) throws -> Data {
            let list = DrawList()
            list.addRect(UIRect(x: 0, y: 0, width: 64, height: 64), color: color)
            if let reveal { list.beginReveal(reveal) }
            try context.device.beginFrame()
            do {
                let commands = CommandBuffer()
                let frame: NativeUIDrawFrame
                if mode == 4 {
                    frame = try context.native.recordPresentation(list: list, into: commands, output: output.texture,
                        viewport: viewport, clear: .zero)
                    #if arch(arm64)
                    #expect(context.native.presentationMSAAMemoryless)
                    #expect(context.native.presentationMSAAAllocatedBytes == 0)
                    #endif
                } else {
                    frame = try context.native.record(list: list, into: commands,
                        target: RenderColorTarget(texture: output.texture, loadAction: .clear(.zero), store: true),
                        viewport: viewport, backdropSource: output.texture, raster: raster)
                }
                try context.device.submit(commands); frame.didSubmit()
                context.device.endFrame()
                try context.device.waitUntilIdle()
                return try context.read(output)
            } catch { context.device.endFrame(); throw error }
        }
        func pixel(_ image: Data, x: Int, y: Int) -> [UInt8] {
            let offset = ((y * scale) * viewport.pixels.x + x * scale) * 4
            return Array(image[offset..<(offset + 4)])
        }
        let initial = try render(Color(r: 0, g: 1, b: 0),
            reveal: UIReveal(sequence: 999, centerX: 12, centerY: 20, radius: 0))
        #expect(pixel(initial, x: 60, y: 60) == [0, 255, 0, 255])
        let old = try render(Color(r: 0, g: 0, b: 1))
        let start = try render(Color(r: 1, g: 0, b: 0),
            reveal: UIReveal(sequence: 1, centerX: 12, centerY: 20, radius: 0))
        #expect(start == old)
        let middle = try render(Color(r: 1, g: 0, b: 0),
            reveal: UIReveal(sequence: 1, centerX: 12, centerY: 20, radius: 18))
        #expect(pixel(middle, x: 12, y: 20) == [0, 0, 255, 255])
        #expect(pixel(middle, x: 60, y: 60) == [255, 0, 0, 255])
        let reversal = try render(Color(r: 0, g: 1, b: 0),
            reveal: UIReveal(sequence: 2, centerX: 12, centerY: 20, radius: 0))
        #expect(reversal == middle)
        let finished = try render(Color(r: 0, g: 1, b: 0),
            reveal: UIReveal(sequence: 2, centerX: 12, centerY: 20, radius: 100))
        #expect(pixel(finished, x: 0, y: 0) == [0, 255, 0, 255])
        #expect(pixel(finished, x: 63, y: 63) == [0, 255, 0, 255])
        let cleared = try render(Color(r: 0, g: 1, b: 0))
        #expect(cleared == finished)
    }
}
#endif
