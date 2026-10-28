import Foundation
#if canImport(CryptoKit)
import CryptoKit
#endif

extension ShaderArtifact {
    /// Prefer a matching platform library when supplied; edited source artifacts
    /// invalidate it automatically. SPIR-V/DXIL retain their offline bytecode.
    public static func load(from bundle: Bundle, name: String, stage: ShaderStage, api: GraphicsAPI,
        directory: String = "Shaders/Native", target: String? = nil) throws -> ShaderArtifact {
        let backend = target ?? (api == .metal ? "metal" : api == .vulkan ? "spirv" : "dxil")
        let file = "\(name).\(stage.rawValue)"
        guard let sourceURL = bundle.url(forResource: file, withExtension: "json", subdirectory: "\(directory)/\(backend)") else {
            throw RHIError.unsupportedFeature("missing offline \(backend) \(file) shader")
        }
        let decoder = JSONDecoder()
        let source = try decoder.decode(ShaderArtifact.self, from: Data(contentsOf: sourceURL))
        guard source.stage == stage else { throw RHIError.layoutMismatch("shader artifact stage mismatch") }
        #if canImport(CryptoKit)
        if api == .metal, let platform = metalLibraryPlatform,
           let url = bundle.url(forResource: file, withExtension: "json", subdirectory: "\(directory)/metal-\(platform)") {
            var library = try decoder.decode(ShaderArtifact.self, from: Data(contentsOf: url))
            let fingerprint = SHA256.hash(data: source.code).map { String(format: "%02x", $0) }.joined()
            if library.sourceFingerprint == fingerprint {
                guard library.format == .metallib, library.stage == stage, library.entryPoint == source.entryPoint else {
                    throw RHIError.layoutMismatch("precompiled Metal library ABI mismatch")
                }
                library.interface = source.interface
                return library
            }
        }
        #endif
        return source
    }

    private static var metalLibraryPlatform: String? {
        #if os(macOS)
        "macos"
        #elseif os(iOS) && targetEnvironment(simulator)
        "ios-simulator"
        #elseif os(iOS)
        "ios"
        #else
        nil
        #endif
    }
}
