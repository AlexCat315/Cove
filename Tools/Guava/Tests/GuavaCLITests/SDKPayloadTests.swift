private import Foundation
private import Testing
private import ToolSupport
@testable import GuavaCLI

@Test func sourcePayloadExcludesBuildCachesAndNativeSourceTrees() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(root) }
    let source = root.appendingPathComponent("source"), payload = root.appendingPathComponent("payload")
    try Files.text("// package", to: source.appendingPathComponent("Package.swift"))
    try Files.text("// runtime", to: source.appendingPathComponent("Sources/Runtime/Runtime.swift"))
    try Files.text("cache", to: source.appendingPathComponent("Tools/Guava/.build/cache"))
    try Files.text("native source", to: source.appendingPathComponent("Runtime/third-party/sdl3/src/SDL.c"))
    try Files.text("java host", to: source.appendingPathComponent("Runtime/third-party/sdl3/android-project/app/src/main/java/SDL.java"))
    try SDKPayload.stageSources(from: source, into: payload)
    #expect(FileManager.default.fileExists(atPath: payload.appendingPathComponent("Sources/Runtime/Runtime.swift").path))
    #expect(!FileManager.default.fileExists(atPath: payload.appendingPathComponent("Tools/Guava/.build").path))
    #expect(!FileManager.default.fileExists(atPath: payload.appendingPathComponent("Runtime/third-party/sdl3/src").path))
    #expect(FileManager.default.fileExists(atPath: payload.appendingPathComponent("Runtime/third-party/sdl3/android-project/app/src/main/java/SDL.java").path))
}

@Test func mobileNativeLibrariesArePreparedOnceInTheSharedSDK() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let sdk = base.appendingPathComponent("SDK"), source = base.appendingPathComponent("native")
    let target = "ios-arm64-simulator"
    try makeSDK(at: sdk)
    try Files.text("0.1.0\n", to: source.appendingPathComponent("VERSION"))
    for path in NativeArtifacts.bundles(for: target) {
        try Files.text("{}", to: source.appendingPathComponent(path + ".artifactbundle/info.json"))
    }
    let archive = base.appendingPathComponent("native.tar.gz")
    try Shell.run([TarArchive.executable, "-czf", archive.path, "-C", source.path, "."], quiet: true)
    let digest = try SHA256.hex(file: archive)
    let cached = base.appendingPathComponent("cache/native/\(digest)/native.tar.gz")
    try Files.copy(archive, to: cached)
    let metadata = NativeArtifacts(version: "0.1.0", platforms: [target: .init(url: URL(string: "https://invalid.example/native.tar.gz")!, sha256: digest)])
    try Files.json(metadata, to: sdk.appendingPathComponent("Artifacts/native.json"))
    #expect(try NativeArtifacts.install(at: sdk, target: target, cacheDirectory: base))
    try Files.remove(cached)
    // A second project must use the prepared SDK, even without its download cache.
    #expect(try NativeArtifacts.install(at: sdk, target: target, cacheDirectory: base))
    #expect(NativeArtifacts.isPrepared(at: sdk, target: target))
    #expect(!FileManager.default.fileExists(atPath: cached.path))
}
