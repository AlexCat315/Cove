extension Device {
    public func makeShaderModuleAsync(_ descriptor: ShaderModuleDescriptor,
        onProgress: @escaping @Sendable (ResourcePreparationProgress) -> Void = { _ in }) async throws -> ShaderModule {
        try await prepareResources(label: descriptor.entryPoint, totalSteps: 1, onProgress: onProgress) { context in
            try context.step("Shader") { try self.makeShaderModule(descriptor) }
        }
    }
    public func makeGraphicsPipelineAsync(_ descriptor: GraphicsPipelineDescriptor,
        onProgress: @escaping @Sendable (ResourcePreparationProgress) -> Void = { _ in }) async throws -> GraphicsPipeline {
        try await prepareResources(label: descriptor.label ?? "Graphics pipeline", totalSteps: 1, onProgress: onProgress) { context in
            try context.step("Graphics pipeline") { try self.makeGraphicsPipeline(descriptor) }
        }
    }
    public func makeComputePipelineAsync(_ descriptor: ComputePipelineDescriptor,
        onProgress: @escaping @Sendable (ResourcePreparationProgress) -> Void = { _ in }) async throws -> ComputePipeline {
        try await prepareResources(label: descriptor.label ?? "Compute pipeline", totalSteps: 1, onProgress: onProgress) { context in
            try context.step("Compute pipeline") { try self.makeComputePipeline(descriptor) }
        }
    }
    public func makeMeshPipelineAsync(_ descriptor: MeshPipelineDescriptor,
        onProgress: @escaping @Sendable (ResourcePreparationProgress) -> Void = { _ in }) async throws -> MeshPipeline {
        try await prepareResources(label: descriptor.label ?? "Mesh pipeline", totalSteps: 1, onProgress: onProgress) { context in
            try context.step("Mesh pipeline") { try self.makeMeshPipeline(descriptor) }
        }
    }
}
