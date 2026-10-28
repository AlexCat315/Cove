import CDX12Bridge

extension DX12Device: RHIBackgroundCompilation {
    public func prepareCompilation(_ request: RHICompilationRequest) throws -> RHICompilationWork {
        // Native registries have their own mutex. The bridge snapshots inputs
        // and releases that mutex during the expensive driver call.
        RHICompilationWork { [self] in
            switch request {
            case let .shader(handle, descriptor):
                try createShaderModule(handle, descriptor: descriptor)
                return RHICompiledObject(publish: {}, discard: { [self] in destroyShaderModule(handle) })
            case let .graphics(handle, descriptor):
                try createGraphicsPipeline(handle, descriptor: descriptor)
                return RHICompiledObject(publish: {}, discard: { [self] in destroyGraphicsPipeline(handle) })
            case let .compute(handle, descriptor):
                try createComputePipeline(handle, descriptor: descriptor)
                return RHICompiledObject(publish: {}, discard: { [self] in destroyComputePipeline(handle) })
            case let .mesh(handle, descriptor):
                try createMeshPipeline(handle, descriptor: descriptor)
                return RHICompiledObject(publish: {}, discard: { [self] in destroyMeshPipeline(handle) })
            }
        }
    }
}
