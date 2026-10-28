/// A backend snapshots its inputs while Device is locked, performs driver work
/// outside that lock, and publishes the finished object under the lock again.
public struct RHICompilationWork {
    let compile: () throws -> RHICompiledObject
    public init(_ compile: @escaping () throws -> RHICompiledObject) { self.compile = compile }
    func runAndInstall() throws { try compile().install() }
}

/// Owns an unpublished native object. Cancellation/failure releases it without
/// inserting a half-built resource into a live backend registry.
public final class RHICompiledObject {
    private let publish: () -> Void
    private let discard: () -> Void
    private var installed = false
    public init(publish: @escaping () -> Void, discard: @escaping () -> Void = {}) {
        self.publish = publish; self.discard = discard
    }
    func install() { precondition(!installed); publish(); installed = true }
    deinit { if !installed { discard() } }
}

public enum RHICompilationRequest {
    case shader(ShaderModule, ShaderModuleDescriptor)
    case graphics(GraphicsPipeline, GraphicsPipelineDescriptor)
    case compute(ComputePipeline, ComputePipelineDescriptor)
    case mesh(MeshPipeline, MeshPipelineDescriptor)
}

/// All built-in backends implement this split. Custom backends must opt in
/// before a host can promise nonblocking preparation on a shared render device.
public protocol RHIBackgroundCompilation: RHIBackend {
    func prepareCompilation(_ request: RHICompilationRequest) throws -> RHICompilationWork
}
