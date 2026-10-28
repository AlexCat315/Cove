#if os(macOS)
import Foundation
import CoreGraphics
import ImageIO
import NativeRHI
import Testing
@testable import CoveUIRuntime
@testable import CoveUIGraphics
@testable import CoveUIImaging
@testable import CoveUIText

/// Opt-in measurement, independent from normal CI regression runs.
@Suite("MSAA evaluation", .serialized)
struct MSAAEvaluationTests {
    struct Measurement: Codable {
        let scale: Int
        let samples: Int
        let transientAttachment: Bool
        let allocatedMSAABytes: Int?
        let vertices: Int
        let indices: Int
        let drawCalls: Int
        let msaaTargetBytes: Int
        let cpuMedianMilliseconds: Double
        let gpuMedianMilliseconds: Double?
        let gpuTimingStatus: String
        let meanRGBError: Double
        let edgeRGBError: Double
        let triangleRGBError: Double
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["COVE_MSAA_EVALUATION"] == "1"))
    func compareCoverageAndCost() throws {
        let context = try NativeUIDrawTestContext(validation: false)
        let directory = URL(fileURLWithPath: ProcessInfo.processInfo.environment["COVE_MSAA_OUTPUT"] ?? "/tmp/cove-msaa-evaluation")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var report: [Measurement] = []
        for scale in [1, 2] {
            let logical = SIMD2<Float>(512, 320)
            let pixels = SIMD2(512 * scale, 320 * scale)
            let referenceViewport = NativeUIViewport(pixels: SIMD2(pixels.x * 4, pixels.y * 4), logical: logical)
            let referenceOutput = try context.nativeOutput(format: .bgra8Unorm, size: referenceViewport.pixels)
            let referenceList = scene(scale: Float(scale * 4), analytic: true)
            try context.native.configure(format: .bgra8Unorm, sampleCount: 1)
            try render(context, list: referenceList, output: referenceOutput, multisample: nil, viewport: referenceViewport)
            let reference = downsample(try context.read(referenceOutput), width: pixels.x, height: pixels.y, factor: 4)
            try writePNG(reference, width: pixels.x, height: pixels.y, to: directory.appendingPathComponent("reference-\(scale)x.png"))
            var persistentPixels: Data?
            for (samples, transient) in [(1, false), (4, false), (4, true)] {
                let list = scene(scale: Float(scale), analytic: samples == 1)
                let viewport = NativeUIViewport(pixels: pixels, logical: logical)
                let output = try context.nativeOutput(format: .bgra8Unorm, size: pixels)
                let multisample = samples == 1 ? nil : try TextureResource(device: context.device,
                    descriptor: TextureDescriptor(width: pixels.x, height: pixels.y, format: .bgra8Unorm,
                        usage: .colorTarget, sampleCount: samples, transientAttachment: transient))
                try context.native.configure(format: .bgra8Unorm, sampleCount: samples)
                // Warm shader/pipeline and upload buffers before sampling.
                for _ in 0..<4 { try render(context, list: list, output: output, multisample: multisample, viewport: viewport, storeSamples: !transient) }
                let profile = GPUTimestampProfile(markerLimit: 100)
                context.device.setGPUTimestampProfile(profile)
                defer { context.device.setGPUTimestampProfile(nil) }
                var cpu: [Double] = []
                var statistics = NativeUIDrawStatistics()
                for _ in 0..<30 {
                    let start = ContinuousClock.now
                    statistics = try render(context, list: list, output: output, multisample: multisample,
                        viewport: viewport, wait: false, storeSamples: !transient)
                    let duration = start.duration(to: .now).components
                    cpu.append(Double(duration.seconds) * 1000 + Double(duration.attoseconds) / 1e15)
                    try context.device.waitUntilIdle()
                }
                context.device.setGPUTimestampProfile(nil)
                let image = try context.read(output)
                if transient, let baseline = persistentPixels { #expect(image == baseline) }
                else if samples == 4 { persistentPixels = image }
                try writePNG(image, width: pixels.x, height: pixels.y,
                    to: directory.appendingPathComponent("\(samples)sample\(transient ? "-transient" : "")-\(scale)x.png"))
                let errors = error(image, reference, width: pixels.x, height: pixels.y, scale: scale)
                let gpu = profile.recordedMarkers.map { Double($0.nanoseconds) / 1e6 }
                let measurement = Measurement(scale: scale, samples: samples, transientAttachment: transient,
                    allocatedMSAABytes: multisample == nil ? 0 : multisample?.allocation?.allocatedBytes, vertices: statistics.vertices,
                    indices: statistics.indices, drawCalls: statistics.drawCalls,
                    msaaTargetBytes: samples == 1 ? 0 : pixels.x * pixels.y * 4 * samples,
                    cpuMedianMilliseconds: median(cpu), gpuMedianMilliseconds: gpu.isEmpty ? nil : median(gpu),
                    gpuTimingStatus: profile.status, meanRGBError: errors.0, edgeRGBError: errors.1,
                    triangleRGBError: errors.2)
                report.append(measurement)
                #expect(statistics.vertices > 0 && image.count == reference.count)
                print("msaa-evaluation scale=\(scale) samples=\(samples) transient=\(transient) edgeMAE=\(errors.1) triangleMAE=\(errors.2) gpuMs=\(measurement.gpuMedianMilliseconds ?? -1)")
            }
            let one = try #require(report.first { $0.scale == scale && $0.samples == 1 })
            let four = try #require(report.first { $0.scale == scale && $0.samples == 4 })
            // Raw custom triangles currently have no analytic boundary. This
            // guards the reason the global default cannot yet become 1x.
            #expect(four.triangleRGBError < one.triangleRGBError)
        }
        let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(report).write(to: directory.appendingPathComponent("report.json"))
        print("msaa-evaluation artifacts: \(directory.path)")
    }

    @discardableResult
    private func render(_ context: NativeUIDrawTestContext, list: DrawList, output: TextureResource,
                        multisample: TextureResource?, viewport: NativeUIViewport, wait: Bool = true, storeSamples: Bool = false) throws -> NativeUIDrawStatistics {
        try context.device.beginFrame()
        do {
            var target = RenderColorTarget(texture: (multisample ?? output).texture,
                loadAction: .clear(SIMD4(0.07, 0.09, 0.12, 1)), store: multisample == nil || storeSamples)
            target.resolveTexture = multisample == nil ? nil : output.texture
            let commands = CommandBuffer()
            let frame = try context.native.record(list: list, into: commands, target: target, viewport: viewport)
            try context.device.submit(commands); frame.didSubmit(); context.device.endFrame()
            if wait { try context.device.waitUntilIdle() }
            return frame.statistics
        } catch { context.device.endFrame(); throw error }
    }

    private func scene(scale: Float, analytic: Bool) -> DrawList {
        let list = DrawList()
        list.pixelScale = scale
        list.geometryAntialiasing = analytic ? .analytic : .multisample
        for i in 0..<10 {
            let x = Float(i % 5) * 98 + 10.3, y = Float(i / 5) * 75 + 8.7
            list.addRoundedRect(UIRect(x: x, y: y, width: 87.4, height: 58.2), radius: 11.7,
                color: Color(r: 0.15 + Float(i) * 0.06, g: 0.55, b: 0.8, a: 0.7))
            list.addRoundedRectStroke(UIRect(x: x + 4.2, y: y + 4.5, width: 78.2, height: 48.3),
                radius: 9.3, width: 1.1, color: .white)
        }
        list.addPathStroke([SIMD2(12.3, 184.7), SIMD2(92.1, 249.4), SIMD2(172.8, 180.3), SIMD2(262.2, 248.9)],
            width: 1.2, color: Color(r: 0.9, g: 0.55, b: 0.15, a: 0.8))
        list.addArcStroke(center: SIMD2(64, 286), radius: 22.3, start: 0.3, sweep: 4.8, width: 1.3, color: .white)
        let mask = UIRoundedRect(UIRect(x: 154.3, y: 264.6, width: 151.4, height: 43.8), radius: 14.2)
        list.pushMask(mask)
        list.addRect(UIRect(x: 142, y: 255, width: 180, height: 65), color: Color(r: 0.2, g: 0.8, b: 0.5, a: 0.65))
        list.popMask()
        list.addTriangle(345.2, 188.4, 493.6, 290.7, 353.8, 308.3, color: Color(r: 0.9, g: 0.3, b: 0.55))
        return list
    }

    private func median(_ values: [Double]) -> Double {
        let sorted = values.sorted(); return sorted[sorted.count / 2]
    }

    private func downsample(_ source: Data, width: Int, height: Int, factor: Int) -> Data {
        var output = Data(count: width * height * 4)
        for y in 0..<height { for x in 0..<width { for channel in 0..<4 {
            var sum = 0
            for sy in 0..<factor { for sx in 0..<factor {
                sum += Int(source[((y * factor + sy) * width * factor + x * factor + sx) * 4 + channel])
            } }
            output[(y * width + x) * 4 + channel] = UInt8((sum + factor * factor / 2) / (factor * factor))
        } } }
        return output
    }

    private func error(_ image: Data, _ reference: Data, width: Int, height: Int, scale: Int) -> (Double, Double, Double) {
        var total = 0.0, edge = 0.0, triangle = 0.0, edges = 0, triangles = 0
        for y in 1..<(height - 1) { for x in 1..<(width - 1) {
            let offset = (y * width + x) * 4
            var delta = 0.0, contrast = 0
            for c in 0..<3 {
                delta += Double(abs(Int(image[offset + c]) - Int(reference[offset + c]))) / 3
                contrast += abs(Int(reference[offset + c]) - Int(reference[offset - 4 + c]))
                contrast += abs(Int(reference[offset + c]) - Int(reference[offset - width * 4 + c]))
            }
            total += delta
            if contrast > 12 { edge += delta; edges += 1 }
            if x > 335 * scale && y > 175 * scale { triangle += delta; triangles += 1 }
        } }
        return (total / Double((width - 2) * (height - 2)), edge / Double(max(1, edges)), triangle / Double(max(1, triangles)))
    }

    private func writePNG(_ pixels: Data, width: Int, height: Int, to url: URL) throws {
        let provider = try #require(CGDataProvider(data: pixels as CFData))
        let bitmap = CGBitmapInfo.byteOrder32Little.union(CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue))
        let image = try #require(CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
            bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: bitmap, provider: provider,
            decode: nil, shouldInterpolate: false, intent: .defaultIntent))
        let destination = try #require(CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        #expect(CGImageDestinationFinalize(destination))
    }
}
#endif
