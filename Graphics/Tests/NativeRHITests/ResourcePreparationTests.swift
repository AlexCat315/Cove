import Foundation
import XCTest
@testable import NativeRHI

private final class PreparationGate: @unchecked Sendable {
    let entered = DispatchSemaphore(value: 0)
    let release = DispatchSemaphore(value: 0)
    func wait() throws {
        entered.signal()
        guard release.wait(timeout: .now() + 5) == .success else { throw RHIError.invalidArgument("test compiler timed out") }
    }
}
private final class PreparationEvents: @unchecked Sendable {
    private let lock = NSLock()
    private var values: [ResourcePreparationProgress] = []
    func append(_ value: ResourcePreparationProgress) { lock.withLock { values.append(value) } }
    var snapshot: [ResourcePreparationProgress] { lock.withLock { values } }
}
private final class PreparationBackend: PipelineTestBackend, RHIBackgroundCompilation, @unchecked Sendable {
    private let stateLock = NSLock()
    var gate: PreparationGate?
    private var buffers: Set<UInt32> = []
    private var published = 0
    private var discarded = 0
    private var destroyedShaders: Set<UInt32> = []
    var counts: (buffers: Int, published: Int, discarded: Int, shadersDestroyed: Int) {
        stateLock.withLock { (buffers.count, published, discarded, destroyedShaders.count) }
    }
    override func createBuffer(_ handle: Buffer, descriptor: BufferDescriptor) throws { _ = stateLock.withLock { buffers.insert(handle.id) } }
    override func destroyBuffer(_ handle: Buffer) { _ = stateLock.withLock { buffers.remove(handle.id) } }
    override func destroyShaderModule(_ handle: ShaderModule) { _ = stateLock.withLock { destroyedShaders.insert(handle.id) } }
    func prepareCompilation(_ request: RHICompilationRequest) throws -> RHICompilationWork {
        let gate = self.gate
        return RHICompilationWork { [self] in
            try gate?.wait()
            let discard: () -> Void
            switch request {
            case let .shader(handle, descriptor):
                try createShaderModule(handle, descriptor: descriptor); discard = { self.destroyShaderModule(handle) }
            case let .graphics(handle, descriptor):
                try createGraphicsPipeline(handle, descriptor: descriptor); discard = { self.destroyGraphicsPipeline(handle) }
            case let .compute(handle, descriptor):
                try createComputePipeline(handle, descriptor: descriptor); discard = { self.destroyComputePipeline(handle) }
            case let .mesh(handle, descriptor):
                try createMeshPipeline(handle, descriptor: descriptor); discard = { self.destroyMeshPipeline(handle) }
            }
            return RHICompiledObject(publish: { self.stateLock.withLock { self.published += 1 } }, discard: {
                self.stateLock.withLock { self.discarded += 1 }; discard()
            })
        }
    }
}

final class ResourcePreparationTests: XCTestCase {
    private func device(_ backend: PreparationBackend) throws -> Device {
        try Device(backend: backend, config: DeviceConfig(preferredBackends: [.metal], enableValidation: false))
    }
    private static func shader() -> ShaderModuleDescriptor {
        ShaderModuleDescriptor(stage: .vertex, format: .mslSource, code: Data([1]), entryPoint: "testVertex")
    }

    func testDriverCompilationDoesNotHoldTheRenderLock() async throws {
        let backend = PreparationBackend(), gate = PreparationGate()
        backend.gate = gate
        let device = try device(backend)
        let task = Task {
            try await device.prepareResources(label: "Test", totalSteps: 1) { context in
                try context.step("Shader") { try device.makeShaderModule(Self.shader()) }
            }
        }
        defer { gate.release.signal() }
        XCTAssertEqual(gate.entered.wait(timeout: .now() + 3), .success)
        // These require Device's lock. They must finish while the compiler is
        // still deliberately blocked, rather than waiting for its driver call.
        XCTAssertEqual(device.pipelineStatistics.shaderModules.count, 0)
        try device.beginFrame(); device.endFrame()
        XCTAssertEqual(backend.counts.published, 0)
        gate.release.signal()
        let shader = try await task.value
        device.destroy(shader)
    }

    func testCancellationDiscardsNativeWorkAndEarlierResources() async throws {
        let backend = PreparationBackend(), gate = PreparationGate(), events = PreparationEvents()
        backend.gate = gate
        let device = try device(backend)
        let task = Task {
            try await device.prepareResources(label: "Test", totalSteps: 2, onProgress: events.append) { context in
                _ = try context.step("Buffer") { try device.makeBuffer(BufferDescriptor(size: 64, usage: .vertex)) }
                return try context.step("Shader") { try device.makeShaderModule(Self.shader()) }
            }
        }
        defer { gate.release.signal() }
        XCTAssertEqual(gate.entered.wait(timeout: .now() + 3), .success)
        task.cancel(); gate.release.signal()
        do { _ = try await task.value; XCTFail("cancelled preparation succeeded") }
        catch { XCTAssertTrue(error is CancellationError) }
        XCTAssertEqual(backend.counts.buffers, 0)
        XCTAssertEqual(backend.counts.published, 0)
        XCTAssertEqual(backend.counts.discarded, 1)
        XCTAssertEqual(events.snapshot.last?.phase, .cancelled)
        XCTAssertEqual(events.snapshot.map(\.sequence), events.snapshot.map(\.sequence).sorted())
    }

    func testFailureReleasesOnlyThePreparationsCacheLeases() async throws {
        let backend = PreparationBackend(), events = PreparationEvents()
        let device = try device(backend)
        let shader = try device.makeShaderModule(Self.shader())
        let layout = try device.makePipelineLayout(PipelineLayoutDescriptor(setLayouts: []))
        let descriptor = GraphicsPipelineDescriptor(layout: layout, vertex: shader, fragment: nil,
            colorAttachments: [ColorAttachmentDescriptor(format: .bgra8Unorm)], depthFormat: nil)
        let existing = try device.makeGraphicsPipeline(descriptor)
        enum Failure: Error { case expected }
        do {
            try await device.prepareResources(label: "Test", totalSteps: 1, onProgress: events.append) { context in
                try context.step("Cached pipeline") {
                    _ = try device.makeGraphicsPipeline(descriptor)
                    throw Failure.expected
                }
            }
            XCTFail("failed preparation succeeded")
        } catch { XCTAssertTrue(error is Failure) }
        XCTAssertEqual(device.pipelineStatistics.livePipelines, 1)
        XCTAssertTrue(backend.destroyedGraphics.isEmpty)
        XCTAssertEqual(events.snapshot.last?.phase, .failed)
        device.destroy(existing); device.destroy(shader)
        XCTAssertEqual(device.pipelineStatistics.livePipelines, 0)
    }

    func testShaderRetirementWaitsForTheBackgroundCompiler() async throws {
        let backend = PreparationBackend(), gate = PreparationGate()
        let device = try device(backend)
        let shader = try device.makeShaderModule(Self.shader())
        let layout = try device.makePipelineLayout(PipelineLayoutDescriptor(setLayouts: []))
        let descriptor = GraphicsPipelineDescriptor(layout: layout, vertex: shader, fragment: nil,
            colorAttachments: [ColorAttachmentDescriptor(format: .bgra8Unorm)], depthFormat: nil)
        backend.gate = gate
        let task = Task {
            try await device.prepareResources(label: "Test", totalSteps: 1) { context in
                try context.step("Pipeline") { try device.makeGraphicsPipeline(descriptor) }
            }
        }
        defer { gate.release.signal() }
        XCTAssertEqual(gate.entered.wait(timeout: .now() + 3), .success)
        device.destroy(shader)
        try device.waitUntilIdle()
        XCTAssertEqual(backend.counts.shadersDestroyed, 0)
        gate.release.signal()
        let pipeline = try await task.value
        XCTAssertEqual(backend.counts.shadersDestroyed, 1)
        device.destroy(pipeline)
    }
}
