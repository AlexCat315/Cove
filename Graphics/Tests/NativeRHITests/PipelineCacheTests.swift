import Foundation
import XCTest

@testable import NativeRHI

private final class PipelineTestUploader: FrameUploader {
    func reset() {}
    func write(_ data: Data, alignment: Int) throws -> UploadLocation { throw RHIError.outOfMemory }
}

/// Records backend calls without a GPU so interning and refcounting are
/// observable independently of native pipeline compilation.
class PipelineTestBackend: RHIBackend, @unchecked Sendable {
    let api: GraphicsAPI = .metal
    let deviceName = "pipeline cache test"

    private let lock = NSLock()
    private(set) var createdShaderModules: [ShaderModule] = []
    private(set) var createdGraphics: [GraphicsPipeline] = []
    private(set) var createdCompute: [ComputePipeline] = []
    private(set) var createdMesh: [MeshPipeline] = []
    private(set) var destroyedGraphics: [GraphicsPipeline] = []
    private(set) var destroyedCompute: [ComputePipeline] = []
    private(set) var destroyedMesh: [MeshPipeline] = []

    func queryCapabilities() -> Capabilities {
        Capabilities {
            $0.graphics = true
            $0.compute = true
            $0.meshShading.mesh = true
        }
    }

    func queryAdapterCapabilities() -> AdapterCapabilities { AdapterCapabilities() }

    func configureSwapchain(_ handle: Swapchain, descriptor: SurfaceDescriptor) throws {}
    func presentationMode(for handle: Swapchain) throws -> PresentationMode { .fifo }
    func destroySwapchain(_ handle: Swapchain) {}
    func acquireSwapchainImage(_ handle: Swapchain) throws -> SwapchainImage {
        throw RHIError.invalidArgument("pipeline tests do not present")
    }
    func present(_ image: SwapchainImage, completion: @escaping () -> Void) throws {}
    func makeFrameUploader(slot: Int) -> FrameUploader { PipelineTestUploader() }
    func submit(_ submit: PlannedSubmit, cpuProfile: SubmissionCPUProfile?,
                completion: @escaping () -> Void) throws { completion() }
    func waitUntilIdle() throws {}

    func createBuffer(_ handle: Buffer, descriptor: BufferDescriptor) throws {}
    func createTexture(_ handle: Texture, descriptor: TextureDescriptor) throws {}
    func createSampler(_ handle: Sampler, descriptor: SamplerDescriptor) throws {}
    func createShaderModule(_ handle: ShaderModule, descriptor: ShaderModuleDescriptor) throws {
        lock.withLock { createdShaderModules.append(handle) }
    }

    func createGraphicsPipeline(_ handle: GraphicsPipeline,
                                descriptor: GraphicsPipelineDescriptor) throws {
        lock.withLock { createdGraphics.append(handle) }
    }

    func createComputePipeline(_ handle: ComputePipeline,
                               descriptor: ComputePipelineDescriptor) throws {
        lock.withLock { createdCompute.append(handle) }
    }

    func createMeshPipeline(_ handle: MeshPipeline, descriptor: MeshPipelineDescriptor) throws {
        lock.withLock { createdMesh.append(handle) }
    }

    func registerBindingSet(_ handle: BindingSet, layout: BindingLayout,
                            layoutEntries: [BindingLayoutEntry],
                            setEntries: [BindingSetEntry]) throws {}
    func unregisterBindingSet(_ handle: BindingSet) {}
    func uploadBufferData(_ buffer: Buffer, offset: Int, data: Data) throws {}
    func uploadTextureData(_ texture: Texture, data: Data, region: TextureUploadRegion,
                           bytesPerRow: Int, subresource: TextureSubresource) throws {}
    func readTextureData(_ texture: Texture, width: Int, height: Int, bytesPerRow: Int,
                         subresource: TextureSubresource,
                         into destination: UnsafeMutableRawBufferPointer) throws {}
    func destroyBuffer(_ handle: Buffer) {}
    func destroyTexture(_ handle: Texture) {}
    func destroySampler(_ handle: Sampler) {}
    func destroyShaderModule(_ handle: ShaderModule) {}

    func destroyGraphicsPipeline(_ handle: GraphicsPipeline) {
        lock.withLock { destroyedGraphics.append(handle) }
    }

    func destroyComputePipeline(_ handle: ComputePipeline) {
        lock.withLock { destroyedCompute.append(handle) }
    }

    func destroyMeshPipeline(_ handle: MeshPipeline) {
        lock.withLock { destroyedMesh.append(handle) }
    }
}

final class PipelineCacheTests: XCTestCase {
    private func makeDevice() throws -> (Device, PipelineTestBackend) {
        let backend = PipelineTestBackend()
        let device = try Device(backend: backend,
                                config: DeviceConfig(preferredBackends: [.metal], enableValidation: false))
        return (device, backend)
    }

    private func shader(_ device: Device, stage: ShaderStage) throws -> ShaderModule {
        try device.makeShaderModule(ShaderModuleDescriptor(
            stage: stage, format: .mslSource, code: Data([1, 2, 3, 4]), entryPoint: "main"))
    }

    private func graphicsDescriptor(_ device: Device) throws -> GraphicsPipelineDescriptor {
        let layout = try device.makePipelineLayout(PipelineLayoutDescriptor(setLayouts: []))
        return GraphicsPipelineDescriptor(
            layout: layout,
            vertex: try shader(device, stage: .vertex),
            fragment: try shader(device, stage: .fragment),
            colorAttachments: [ColorAttachmentDescriptor(format: .rgba8Unorm)],
            depthFormat: nil, depthStencil: nil)
    }

    func testGraphicsPipelinesInternAndRefCount() throws {
        let (device, backend) = try makeDevice()
        let descriptor = try graphicsDescriptor(device)

        let first = try device.makeGraphicsPipeline(descriptor)
        var relabeled = descriptor
        relabeled.label = "label does not affect state"
        let second = try device.makeGraphicsPipeline(relabeled)

        XCTAssertEqual(first, second)
        XCTAssertEqual(backend.createdGraphics, [first])
        XCTAssertEqual(device.pipelineStatistics.graphics.cold.count, 1)
        XCTAssertEqual(device.pipelineStatistics.graphics.cold.duringFrame, 0)
        XCTAssertEqual(device.pipelineStatistics.graphics.warmCount, 1)
        XCTAssertEqual(device.pipelineStatistics.graphics.live, 1)

        device.destroy(first)
        XCTAssertTrue(backend.destroyedGraphics.isEmpty)
        XCTAssertEqual(device.pipelineStatistics.graphics.live, 1)

        device.destroy(second)
        XCTAssertEqual(backend.destroyedGraphics, [first])
        XCTAssertEqual(device.pipelineStatistics.graphics.live, 0)

        let recreated = try device.makeGraphicsPipeline(descriptor)
        XCTAssertNotEqual(recreated, first)
        XCTAssertEqual(backend.createdGraphics.count, 2)
        XCTAssertEqual(device.pipelineStatistics.graphics.cold.count, 2)
        device.destroy(recreated)
    }

    func testDifferentGraphicsStateGetsItsOwnPipeline() throws {
        let (device, backend) = try makeDevice()
        let descriptor = try graphicsDescriptor(device)
        let singleSample = try device.makeGraphicsPipeline(descriptor)
        var multisampled = descriptor
        multisampled.sampleCount = 2
        let fourSamples = try device.makeGraphicsPipeline(multisampled)

        XCTAssertNotEqual(singleSample, fourSamples)
        XCTAssertEqual(backend.createdGraphics.count, 2)
        XCTAssertEqual(device.pipelineStatistics.graphics.live, 2)
        device.destroy(singleSample)
        device.destroy(fourSamples)
    }

    func testComputePipelinesInternAndRefCount() throws {
        let (device, backend) = try makeDevice()
        let layout = try device.makePipelineLayout(PipelineLayoutDescriptor(setLayouts: []))
        let descriptor = ComputePipelineDescriptor(layout: layout, shader: try shader(device, stage: .compute))

        let first = try device.makeComputePipeline(descriptor)
        let second = try device.makeComputePipeline(descriptor)
        XCTAssertEqual(first, second)
        XCTAssertEqual(backend.createdCompute, [first])

        device.destroy(first)
        XCTAssertTrue(backend.destroyedCompute.isEmpty)
        device.destroy(second)
        XCTAssertEqual(backend.destroyedCompute, [first])
        XCTAssertEqual(device.pipelineStatistics.compute.live, 0)
    }

    func testMeshPipelinesInternAndRefCount() throws {
        let (device, backend) = try makeDevice()
        let layout = try device.makePipelineLayout(PipelineLayoutDescriptor(setLayouts: []))
        var descriptor = MeshPipelineDescriptor(layout: layout, mesh: try shader(device, stage: .mesh))
        descriptor.fragment = try shader(device, stage: .fragment)

        let first = try device.makeMeshPipeline(descriptor)
        let second = try device.makeMeshPipeline(descriptor)
        XCTAssertEqual(first, second)
        XCTAssertEqual(backend.createdMesh, [first])
        XCTAssertEqual(device.pipelineStatistics.mesh.live, 1)

        device.destroy(first)
        XCTAssertTrue(backend.destroyedMesh.isEmpty)
        device.destroy(second)
        XCTAssertEqual(backend.destroyedMesh, [first])
        XCTAssertEqual(device.pipelineStatistics.mesh.live, 0)
    }

    func testShaderModuleCreationsAreMeasured() throws {
        let (device, backend) = try makeDevice()
        let vertex = try shader(device, stage: .vertex)
        let fragment = try shader(device, stage: .fragment)

        XCTAssertEqual(backend.createdShaderModules, [vertex, fragment])
        XCTAssertEqual(device.pipelineStatistics.shaderModules.count, 2)
        device.resetPipelineStatistics()
        XCTAssertEqual(device.pipelineStatistics.shaderModules.count, 0)
        device.destroy(vertex)
        device.destroy(fragment)
    }

    func testCreationDuringFrameIsCounted() throws {
        let (device, _) = try makeDevice()
        let descriptor = try graphicsDescriptor(device)

        try device.beginFrame()
        let pipeline = try device.makeGraphicsPipeline(descriptor)
        let module = try device.makeShaderModule(ShaderModuleDescriptor(
            stage: .vertex, format: .mslSource, code: Data([5, 6]), entryPoint: "newVertex"))
        device.endFrame()

        XCTAssertEqual(device.pipelineStatistics.graphics.cold.duringFrame, 1)
        XCTAssertEqual(device.pipelineStatistics.shaderModules.duringFrame, 1)
        XCTAssertEqual(device.pipelineStatistics.creationsDuringFrame, 2)
        device.destroy(pipeline)
        device.destroy(module)
    }

    func testShaderModulesShareContentAndReleaseTheirOwnLeases() throws {
        let (device, backend) = try makeDevice()
        let first = try shader(device, stage: .vertex)
        let second = try shader(device, stage: .vertex)
        XCTAssertEqual(first, second)
        XCTAssertEqual(backend.createdShaderModules.count, 1)
        XCTAssertEqual(device.pipelineStatistics.shaderModuleCacheHits, 1)
        device.destroy(first)
        XCTAssertEqual(device.pipelineStatistics.liveShaderModules, 1)
        device.destroy(second)
        XCTAssertEqual(device.pipelineStatistics.liveShaderModules, 0)
        let recreated = try shader(device, stage: .vertex)
        XCTAssertNotEqual(first, recreated)
        device.destroy(recreated)
    }

    func testResetClearsCountersButKeepsInternedPipelines() throws {
        let (device, backend) = try makeDevice()
        let descriptor = try graphicsDescriptor(device)
        let pipeline = try device.makeGraphicsPipeline(descriptor)

        device.resetPipelineStatistics()
        XCTAssertEqual(device.pipelineStatistics.graphics.cold.count, 0)
        XCTAssertEqual(device.pipelineStatistics.graphics.warmCount, 0)
        XCTAssertEqual(device.pipelineStatistics.graphics.live, 1)

        let reused = try device.makeGraphicsPipeline(descriptor)
        XCTAssertEqual(backend.createdGraphics.count, 1)
        XCTAssertEqual(device.pipelineStatistics.graphics.warmCount, 1)
        device.destroy(pipeline)
        device.destroy(reused)
    }
}
