#if os(macOS)
import Foundation
import NativeRHI
import Testing
@testable import CoveUIRuntime
@testable import CoveUIGraphics
@testable import CoveUIImaging
@testable import CoveUIText

@Suite("Transient 4x MSAA", .serialized)
struct TransientMSAATests {
    @Test("Transient attachments preserve resolved UI pixels", arguments: [1, 2])
    func pixels(scale: Int) throws {
        let context = try NativeUIDrawTestContext()
        let viewport = NativeUIViewport(pixels: SIMD2(96 * scale, 64 * scale), logical: SIMD2(96, 64))
        try context.native.configure(format: .bgra8UnormSRGB, sampleCount: 4)
        let list = DrawList()
        list.pixelScale = Float(scale)
        list.addRoundedRect(UIRect(x: 1.3, y: 2.7, width: 54.2, height: 29.8), radius: 8.2,
            color: Color(r: 0.7, g: 0.2, b: 0.5, a: 0.6))
        list.addRoundedRectStroke(UIRect(x: 3.7, y: 4.2, width: 48.4, height: 25.3), radius: 6.1,
            width: 1.2, color: Color(r: 1, g: 1, b: 1, a: 0.7))
        list.addPathStroke([SIMD2(2.3, 52.8), SIMD2(37.6, 36.2), SIMD2(64.1, 59.3)], width: 1.1, color: .white)
        list.pushMask(UIRoundedRect(UIRect(x: 59.3, y: 2.8, width: 31.4, height: 28.2), radius: 7.2))
        list.addRect(UIRect(x: 54, y: 0, width: 42, height: 36), color: Color(r: 0.1, g: 0.8, b: 0.5, a: 0.8))
        list.popMask()
        list.addTriangle(67.2, 37.3, 93.7, 60.1, 70.4, 62.6, color: Color(r: 0.8, g: 0.2, b: 0.4))
        func render(transient: Bool) throws -> (Data, TextureAllocation?) {
            let output = try context.nativeOutput(format: .bgra8UnormSRGB, size: viewport.pixels)
            let samples = try TextureResource(device: context.device, descriptor: TextureDescriptor(
                width: viewport.pixels.x, height: viewport.pixels.y, format: .bgra8UnormSRGB,
                usage: .colorTarget, sampleCount: 4, transientAttachment: transient))
            var target = RenderColorTarget(texture: samples.texture, loadAction: .clear(SIMD4(0.07, 0.09, 0.12, 1)), store: false)
            target.resolveTexture = output.texture
            try context.device.beginFrame()
            do {
                let commands = CommandBuffer()
                let frame = try context.native.record(list: list, into: commands, target: target, viewport: viewport)
                try context.device.submit(commands); frame.didSubmit(); context.device.endFrame()
                try context.device.waitUntilIdle()
                return (try context.read(output), samples.allocation)
            } catch { context.device.endFrame(); throw error }
        }
        let persistent = try render(transient: false), transient = try render(transient: true)
        #expect(persistent.0 == transient.0)
        let normal = try #require(persistent.1), temporary = try #require(transient.1)
        #expect(normal.allocatedBytes > 0 && !normal.isMemoryless)
        if temporary.isMemoryless { #expect(temporary.allocatedBytes == 0) }
        #if arch(arm64)
        #expect(temporary.isMemoryless)
        #endif
        print("transient-msaa scale=\(scale) persistent=\(normal.allocatedBytes) transient=\(temporary.allocatedBytes) equal=true")
    }

    @Test("Memoryless attachments reject loading or preserving prior samples")
    func invalidPasses() throws {
        let context = try NativeUIDrawTestContext()
        try context.native.configure(format: .bgra8Unorm, sampleCount: 4)
        let samples = try TextureResource(device: context.device, descriptor: TextureDescriptor(
            width: 32, height: 32, format: .bgra8Unorm, usage: .colorTarget, sampleCount: 4, transientAttachment: true))
        guard samples.allocation?.isMemoryless == true else { return }
        let output = try context.nativeOutput(format: .bgra8Unorm, size: SIMD2(32, 32))
        let list = DrawList(); list.addRect(UIRect(x: 0, y: 0, width: 32, height: 32), color: .white)
        for (load, store) in [(ColorLoadAction.load, false), (.clear(.zero), true)] {
            var target = RenderColorTarget(texture: samples.texture, loadAction: load, store: store)
            target.resolveTexture = output.texture
            try context.device.beginFrame()
            defer { context.device.endFrame() }
            let commands = CommandBuffer()
            _ = try context.native.record(list: list, into: commands, target: target,
                viewport: NativeUIViewport(pixels: SIMD2(32, 32), logical: SIMD2(32, 32)))
            #expect(throws: RHIError.self) { try context.device.submit(commands) }
        }
    }

    @Test("Transient storage only accepts multisample attachments")
    func invalidTexture() throws {
        let context = try NativeUIDrawTestContext()
        #expect(throws: RHIError.self) {
            _ = try TextureResource(device: context.device, descriptor: TextureDescriptor(
                width: 32, height: 32, format: .bgra8Unorm, usage: .colorTarget, transientAttachment: true))
        }
        #expect(throws: RHIError.self) {
            _ = try TextureResource(device: context.device, descriptor: TextureDescriptor(
                width: 32, height: 32, format: .bgra8Unorm, usage: [.colorTarget, .transferSource],
                sampleCount: 4, transientAttachment: true))
        }
    }
}
#endif
