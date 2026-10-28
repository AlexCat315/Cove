#if (os(Windows) || os(Linux) || os(Android)) && canImport(CVulkanHeaders)
extension VulkanBackend: RHIBackgroundCompilation {
    func prepareCompilation(_ request: RHICompilationRequest) throws -> RHICompilationWork {
        switch request {
        case let .shader(handle, descriptor): try prepareShaderCompilation(handle, descriptor: descriptor)
        case let .graphics(handle, descriptor): try prepareGraphicsCompilation(handle, descriptor: descriptor)
        case let .compute(handle, descriptor): try prepareComputeCompilation(handle, descriptor: descriptor)
        case let .mesh(handle, descriptor): try prepareMeshCompilation(handle, descriptor: descriptor)
        }
    }
}
#endif
