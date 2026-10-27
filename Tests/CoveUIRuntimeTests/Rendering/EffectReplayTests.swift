#if os(macOS)
import Foundation
import NativeRHI
import Testing
@testable import CoveUIRuntime

@Suite("Effect replay", .serialized)
struct EffectReplayTests {
    @Test("Overlapping frost preserves coverage and alpha", arguments: [1, 2])
    func overlappingFrost(scale: Int) throws {
        let context = try NativeUIDrawTestContext()
        let viewport = NativeUIViewport(pixels: SIMD2(128 * scale, 96 * scale), logical: SIMD2(128, 96))
        let list = DrawList()
        list.pixelScale = Float(scale)
        for x in stride(from: 0, to: 128, by: 8) {
            list.addRect(UIRect(x: Float(x), y: 0, width: 4, height: 96), color: .white)
        }
        list.addTriangle(2.3, 3.7, 123.7, 84.2, 7.6, 93.2, color: Color(r: 0.8, g: 0.1, b: 0.2, a: 0.6))
        for rect in [UIRect(x: 12.3, y: 8.7, width: 68.2, height: 52.1),
                     UIRect(x: 49.7, y: 35.2, width: 65.9, height: 53.3)] {
            list.beginBackdrop(UIBackdrop(rect: rect, cornerRadius: 7, blurRadius: 5, opacity: 0.8))
            list.addRoundedRect(rect, radius: 7, color: Color(r: 0.2, g: 0.3, b: 0.6, a: 0.3))
            list.addRoundedRectStroke(rect, radius: 7, width: 1.2, color: Color(r: 1, g: 1, b: 1, a: 0.5))
        }
        let pixels = try context.nativeImage(list: list, format: NativeUITestFormat(native: .bgra8UnormSRGB),
            samples: 4, viewport: viewport, backdropSource: true).0
        let output = try context.nativeOutput(format: .bgra8UnormSRGB, size: viewport.pixels)
        // Exercise the bounded path on a tile GPU too: private storage models
        // a backend that cannot discard multisample backing into tile memory.
        let budget = viewport.pixels.x * 4 * 4 * 13
        let raster = try NativeUIRasterWorkspace(device: context.device, pixels: viewport.pixels,
            format: .bgra8UnormSRGB, samples: 4, budget: budget, preferMemoryless: false)
        #expect(raster.tiled)
        #expect(raster.logicalBytes <= budget)
        try context.device.beginFrame()
        do {
            let commands = CommandBuffer()
            let frame = try context.native.record(list: list, into: commands,
                target: RenderColorTarget(texture: output.texture, loadAction: .clear(SIMD4(0.07, 0.09, 0.12, 0.5)), store: true),
                viewport: viewport, backdropSource: output.texture, raster: raster)
            try context.device.submit(commands); frame.didSubmit(); context.device.endFrame()
            try context.device.waitUntilIdle()
        } catch { context.device.endFrame(); throw error }
        let bounded = try context.read(output)
        #expect(bounded == pixels)
        if let directory = ProcessInfo.processInfo.environment["COVE_EFFECT_BASELINE"] {
            let url = URL(fileURLWithPath: directory, isDirectory: true)
            try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
            let file = url.appendingPathComponent("frost-\(scale).bgra")
            if FileManager.default.fileExists(atPath: file.path) {
                let baseline = try Data(contentsOf: file)
                try pixels.write(to: url.appendingPathComponent("frost-\(scale)-replay.bgra"))
                var sum = 0, maximum = 0, changed = 0
                for index in pixels.indices {
                    let error = abs(Int(pixels[index]) - Int(baseline[index]))
                    sum += error; maximum = max(maximum, error)
                    if error > 0 { changed += 1 }
                }
                print("effect-baseline scale=\(scale) changedBytes=\(changed) maximum=\(maximum) mean=\(Double(sum) / Double(pixels.count))")
            } else { try pixels.write(to: file) }
        }
        #expect(pixels.count == viewport.pixels.x * viewport.pixels.y * 4)
    }
}
#endif
