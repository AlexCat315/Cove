// NativeRHI — pipeline interning and compilation metrics.
//
// Identical pipeline descriptors share one backend object. The cache is
// reference-counted: every successful `make…Pipeline` adds a reference and
// every `destroy` removes one, so the backend object is retired only after the
// last owner releases it. Labels are diagnostic and never take part in a key.
//
// Creation is measured separately from cache hits. A miss is the cold path
// that may compile native GPU code; a hit is only a dictionary lookup.

import Foundation

// MARK: - Keys

struct ShaderModuleKey: Hashable {
    let stage: ShaderStage
    let format: ShaderFormat
    let code: Data
    let entryPoint: String
    let workgroup: [Int]
    let constants: [String]
    init(_ descriptor: ShaderModuleDescriptor) {
        stage = descriptor.stage; format = descriptor.format; code = descriptor.code; entryPoint = descriptor.entryPoint
        workgroup = [descriptor.threadgroupSize.x, descriptor.threadgroupSize.y, descriptor.threadgroupSize.z]
        constants = descriptor.specializationConstants.sorted { $0.id < $1.id }.map {
            "\($0.id):\($0.value.type):\($0.value.bits)"
        }
    }
}

/// Immutable graphics pipeline state. Every field that changes the native
/// pipeline object is present; the label is not.
struct GraphicsPipelineKey: Hashable {
    let sampleCount: Int
    let layout: UInt32
    let vertex: UInt32
    let fragment: UInt32?
    let colorAttachments: [ColorAttachmentDescriptor]
    let depthFormat: TextureFormat?
    let stencilFormat: TextureFormat?
    let primitive: PrimitiveType
    let rasterization: RasterizationState
    let depthStencil: DepthStencilState?
    let vertexLayout: VertexLayoutDescriptor?

    init(_ descriptor: GraphicsPipelineDescriptor) {
        sampleCount = descriptor.sampleCount
        layout = descriptor.layout.id
        vertex = descriptor.vertex.id
        fragment = descriptor.fragment?.id
        colorAttachments = descriptor.colorAttachments
        depthFormat = descriptor.depthFormat
        stencilFormat = descriptor.stencilFormat
        primitive = descriptor.primitive
        rasterization = descriptor.rasterization
        depthStencil = descriptor.depthStencil
        vertexLayout = descriptor.vertexLayout
    }
}

struct ComputePipelineKey: Hashable {
    let layout: UInt32
    let shader: UInt32

    init(_ descriptor: ComputePipelineDescriptor) {
        layout = descriptor.layout.id
        shader = descriptor.shader.id
    }
}

struct MeshPipelineKey: Hashable {
    let layout: UInt32
    let mesh: UInt32
    let task: UInt32?
    let fragment: UInt32?
    let colorAttachments: [ColorAttachmentDescriptor]
    let depthFormat: TextureFormat?
    let rasterization: RasterizationState
    let depthStencil: DepthStencilState?

    init(_ descriptor: MeshPipelineDescriptor) {
        layout = descriptor.layout.id
        mesh = descriptor.mesh.id
        task = descriptor.task?.id
        fragment = descriptor.fragment?.id
        colorAttachments = descriptor.colorAttachments
        depthFormat = descriptor.depthFormat
        rasterization = descriptor.rasterization
        depthStencil = descriptor.depthStencil
    }
}

// MARK: - Interning cache

struct PipelineLease<Handle: RHIHandle> {
    let handle: Handle
    /// True when this call created the backend object.
    let created: Bool
    let discarded: Handle?
    init(handle: Handle, created: Bool, discarded: Handle? = nil) {
        self.handle = handle; self.created = created; self.discarded = discarded
    }
}

/// Reference-counted value-keyed pipeline cache.
///
/// `release` returning true means the last owner is gone and the backend object
/// must be retired. Unknown handles are ignored so a repeated destroy cannot
/// affect a live pipeline.
final class PipelineCache<Key: Hashable, Handle: RHIHandle> {
    private var entries: [Key: Handle] = [:]
    private var keysByHandle: [UInt32: Key] = [:]
    private var references: [UInt32: Int] = [:]

    var count: Int { entries.count }

    func acquire(_ key: Key, create: () throws -> Handle) rethrows -> PipelineLease<Handle> {
        if let existing = entries[key] {
            references[existing.id, default: 0] += 1
            return PipelineLease(handle: existing, created: false)
        }
        let handle = try create()
        // A backend compile may release Device's lock. Another caller can
        // publish this key first; keep its lease and retire our duplicate.
        if let existing = entries[key] {
            references[existing.id, default: 0] += 1
            return PipelineLease(handle: existing, created: false, discarded: handle)
        }
        entries[key] = handle
        keysByHandle[handle.id] = key
        references[handle.id] = 1
        return PipelineLease(handle: handle, created: true)
    }

    func release(_ handle: Handle) -> Bool {
        guard let referenceCount = references[handle.id] else { return false }
        guard referenceCount > 1 else {
            references[handle.id] = nil
            if let key = keysByHandle.removeValue(forKey: handle.id) {
                entries.removeValue(forKey: key)
            }
            return true
        }
        references[handle.id] = referenceCount - 1
        return false
    }
}

// MARK: - Statistics

/// Successful creations of one resource kind, measured on the calling thread.
public struct PipelineCompileTiming: Sendable, Equatable {
    /// Number of successful creations.
    public var count: UInt64 = 0
    /// Sum of creation durations.
    public var totalNanoseconds: UInt64 = 0
    /// Longest single creation.
    public var maxNanoseconds: UInt64 = 0
    /// Creations that happened between `beginFrame` and `endFrame`.
    public var duringFrame: UInt64 = 0

    public init() {}

    public var averageMicroseconds: Double {
        guard count > 0 else { return 0 }
        return Double(totalNanoseconds) / Double(count) / 1_000
    }

    mutating func record(_ duration: Duration, duringFrame: Bool) {
        let nanoseconds = duration.rhiNanoseconds
        count += 1
        let (total, overflow) = totalNanoseconds.addingReportingOverflow(nanoseconds)
        totalNanoseconds = overflow ? .max : total
        maxNanoseconds = max(maxNanoseconds, nanoseconds)
        if duringFrame { self.duringFrame += 1 }
    }
}

/// Cold compiles plus warm cache hits for one pipeline kind.
public struct PipelineKindStatistics: Sendable, Equatable {
    /// Cache misses, including the backend pipeline creation.
    public var cold = PipelineCompileTiming()
    /// Cache hits and the time spent looking them up.
    public var warmCount: UInt64 = 0
    public var warmTotalNanoseconds: UInt64 = 0
    public var warmMaxNanoseconds: UInt64 = 0
    /// Handles currently interned in the cache.
    public var live: Int = 0

    public init() {}

    public var warmAverageMicroseconds: Double {
        guard warmCount > 0 else { return 0 }
        return Double(warmTotalNanoseconds) / Double(warmCount) / 1_000
    }

    mutating func record(_ duration: Duration, created: Bool, duringFrame: Bool) {
        if created {
            cold.record(duration, duringFrame: duringFrame)
        } else {
            warmCount += 1
            let nanoseconds = duration.rhiNanoseconds
            let (total, overflow) = warmTotalNanoseconds.addingReportingOverflow(nanoseconds)
            warmTotalNanoseconds = overflow ? .max : total
            warmMaxNanoseconds = max(warmMaxNanoseconds, nanoseconds)
        }
    }
}

/// Snapshot of pipeline creation and cache behavior for one `Device`.
public struct PipelineStatistics: Sendable, Equatable {
    /// Cold shader module creations; identical modules share a leased handle.
    public var shaderModules = PipelineCompileTiming()
    public var shaderModuleCacheHits: UInt64 = 0
    public var liveShaderModules: Int = 0
    public var graphics = PipelineKindStatistics()
    public var compute = PipelineKindStatistics()
    public var mesh = PipelineKindStatistics()

    public init() {}

    public var cacheHits: UInt64 {
        graphics.warmCount + compute.warmCount + mesh.warmCount
    }

    public var cacheMisses: UInt64 {
        graphics.cold.count + compute.cold.count + mesh.cold.count
    }

    public var livePipelines: Int {
        graphics.live + compute.live + mesh.live
    }

    public var creationsDuringFrame: UInt64 {
        shaderModules.duringFrame + graphics.cold.duringFrame
            + compute.cold.duringFrame + mesh.cold.duringFrame
    }
}

// MARK: - Device pipeline store

/// Groups the three pipeline caches and their counters, keeping `Device` small.
final class PipelineStore {
    let shaders = PipelineCache<ShaderModuleKey, ShaderModule>()
    let graphics = PipelineCache<GraphicsPipelineKey, GraphicsPipeline>()
    let compute = PipelineCache<ComputePipelineKey, ComputePipeline>()
    let mesh = PipelineCache<MeshPipelineKey, MeshPipeline>()

    private var statistics = PipelineStatistics()

    var snapshot: PipelineStatistics {
        var result = statistics
        result.liveShaderModules = shaders.count
        result.graphics.live = graphics.count
        result.compute.live = compute.count
        result.mesh.live = mesh.count
        return result
    }

    func recordShaderModule(_ duration: Duration, duringFrame: Bool) {
        statistics.shaderModules.record(duration, duringFrame: duringFrame)
    }
    func recordShaderCacheHit() { statistics.shaderModuleCacheHits += 1 }

    func recordGraphics(_ duration: Duration, created: Bool, duringFrame: Bool) {
        statistics.graphics.record(duration, created: created, duringFrame: duringFrame)
    }

    func recordCompute(_ duration: Duration, created: Bool, duringFrame: Bool) {
        statistics.compute.record(duration, created: created, duringFrame: duringFrame)
    }

    func recordMesh(_ duration: Duration, created: Bool, duringFrame: Bool) {
        statistics.mesh.record(duration, created: created, duringFrame: duringFrame)
    }

    func resetStatistics() {
        statistics = PipelineStatistics()
    }
}

// MARK: - Duration

extension Duration {
    /// Whole nanoseconds, saturating instead of trapping on overflow.
    var rhiNanoseconds: UInt64 {
        let parts = components
        let seconds = UInt64(max(0, parts.seconds))
        let (whole, overflow) = seconds.multipliedReportingOverflow(by: 1_000_000_000)
        guard !overflow else { return .max }
        let fraction = UInt64(max(0, parts.attoseconds)) / 1_000_000_000
        let (total, addOverflow) = whole.addingReportingOverflow(fraction)
        return addOverflow ? .max : total
    }
}
