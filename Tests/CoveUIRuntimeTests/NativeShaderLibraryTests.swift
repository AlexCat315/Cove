#if os(macOS)
import Foundation
import NativeRHI
import Testing
@testable import CoveUIRuntime
@testable import CoveUIGraphics
@testable import CoveUIImaging
@testable import CoveUIText

@Suite("Packaged Metal libraries", .serialized)
struct NativeShaderLibraryTests {
    @Test("UI loads platform metallib and renders while custom shaders prepare")
    func backgroundPreparation() async throws {
        for name in ["ui", "ui_backdrop"] {
            for stage in [ShaderStage.vertex, .fragment] {
                let artifact = try ShaderArtifact.load(from: ShaderResourcesBundle.bundle, name: name, stage: stage, api: .metal)
                #expect(artifact.format == .metallib)
                #expect(artifact.code.prefix(4) == Data("MTLB".utf8))
            }
        }
        let context = try NativeUIDrawTestContext()
        let device = context.device
        let progress = NativePreparationEvents()
        let task = Task {
            try await device.prepareResources(label: "Custom shaders", totalSteps: 12, onProgress: progress.append) { preparation in
                var result: [ShaderModule] = []
                for index in 0..<12 {
                    result.append(try preparation.step("Custom \(index)") {
                        let source = "#include <metal_stdlib>\nusing namespace metal;\nkernel void custom\(index)(device uint *out [[buffer(0)]], uint id [[thread_position_in_grid]]) { out[id] = id + \(index); }"
                        return try device.makeShaderModule(ShaderModuleDescriptor(
                            metalSource: source, stage: .compute, entryPoint: "custom\(index)"))
                    })
                }
                return result
            }
        }
        let list = DrawList()
        list.addRect(UIRect(x: 0, y: 0, width: 16, height: 16), color: .white)
        var frames = 0
        repeat {
            let (pixels, _) = try context.nativeImage(list: list, format: NativeUITestFormat(native: .bgra8Unorm),
                samples: 1, viewport: NativeUIViewport(pixels: SIMD2(16, 16), logical: SIMD2(16, 16)))
            #expect(pixels.allSatisfy { $0 == 255 })
            frames += 1
        } while !progress.finished && frames < 120
        let modules = try await task.value
        modules.forEach { context.device.destroy($0) }
        #expect(progress.finished)
        #expect(frames > 0)
        print("metallib-ui background preparation: \(frames) frames, \(modules.count) custom shader modules")
    }
}

private final class NativePreparationEvents: @unchecked Sendable {
    private let lock = NSLock()
    private var complete = false
    func append(_ progress: ResourcePreparationProgress) { lock.withLock { complete = progress.phase == .finished } }
    var finished: Bool { lock.withLock { complete } }
}
#endif
