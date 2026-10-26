internal import Foundation
private import ToolSupport
private import GuavaFileSystem

struct NativeArtifacts: Codable {
    struct Archive: Codable {
        let url: URL
        let sha256: String
    }
    let version: String
    let platforms: [String: Archive]

    static func directory(at root: URL, target: String) -> URL {
        target == CLIUpdate.platform ? root : root.appendingPathComponent("Artifacts/native/\(target)")
    }

    static func bundles(for target: String) -> [String] {
        ["Runtime/vendor/SDL3", "Runtime/vendor/lunasvg", "Runtime/vendor/plutovg",
         "Runtime/vendor/webp", "Runtime/vendor/sharpyuv", "vendor/CFreeType", "vendor/CHarfBuzz"]
            + (target.hasPrefix("ios-") || target.hasPrefix("android-") ? [] : ["vendor/GhosttyVt"])
    }

    static func isPrepared(at root: URL, target: String = CLIUpdate.platform) -> Bool {
        let base = directory(at: root, target: target)
        return bundles(for: target).allSatisfy {
            FileManager.default.fileExists(atPath: base.appendingPathComponent($0 + ".artifactbundle/info.json").path)
        }
    }

    @discardableResult
    static func install(at root: URL, target: String = CLIUpdate.platform, cacheDirectory: URL = Installation.directory) throws -> Bool {
        try FileLock.withLock(at: root.appendingPathComponent("Artifacts/locks/\(target).lock")) {
            try installLocked(at: root, target: target, cacheDirectory: cacheDirectory)
        }
    }

    private static func installLocked(at root: URL, target: String, cacheDirectory: URL) throws -> Bool {
        if isPrepared(at: root, target: target) { return true }
        let file = root.appendingPathComponent("Artifacts/native.json")
        guard FileManager.default.fileExists(atPath: file.path) else { return false }
        let manifest = try JSONDecoder().decode(Self.self, from: Data(contentsOf: file))
        guard manifest.version == (try SDKRelease.read(at: root).version),
              let archive = manifest.platforms[target],
              archive.sha256.range(of: #"^[0-9a-fA-F]{64}$"#, options: .regularExpression) != nil else {
            throw ToolError.invalid("This SDK has no native libraries for \(target); install a complete SDK release")
        }
        let cache = cacheDirectory.appendingPathComponent("cache/native/\(archive.sha256)")
        let tar = cache.appendingPathComponent("native.tar.gz")
        try Download.file(from: archive.url, to: tar, sha256: archive.sha256)
        let staging = cache.appendingPathComponent(UUID().uuidString)
        defer { try? Files.remove(staging) }
        try TarArchive.extract(tar, to: staging)
        let version = try String(contentsOf: staging.appendingPathComponent("VERSION"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard version == manifest.version else { throw ToolError.invalid("Native archive and SDK versions disagree") }
        let base = directory(at: root, target: target)
        for path in bundles(for: target) {
            let relative = path + ".artifactbundle"
            let source = staging.appendingPathComponent(relative)
            guard FileManager.default.fileExists(atPath: source.appendingPathComponent("info.json").path) else {
                throw ToolError.invalid("Native archive is missing \(relative)")
            }
            let destination = base.appendingPathComponent(relative)
            try Files.directory(destination.deletingLastPathComponent())
            // Parallel project builds may prepare the same SDK. Never replace a published bundle.
            do { try FileManager.default.moveItem(at: source, to: destination) }
            catch {
                guard FileManager.default.fileExists(atPath: destination.appendingPathComponent("info.json").path) else { throw error }
            }
        }
        return true
    }
}
