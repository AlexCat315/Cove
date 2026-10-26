internal import Foundation

enum SDKPayload {
    static func stageSources(from root: URL, into payload: URL) throws {
        let paths = ["Package.swift", "LICENSE", "NOTICE", "README.md", "CHANGELOG.md",
                     "Sources", "Tests", "Runtime/Sources", "Runtime/Tests", "Text", "PlatformCore",
                     "Portable", "System", "Logging", "Hosts", "Tools", "Platforms", "Terminal", "Examples"]
        let excluded: Set<String> = [".git", ".build", ".guava", "build", "vendor", "node_modules", "dist"]
        for path in paths {
            let source = root.appendingPathComponent(path)
            guard FileManager.default.fileExists(atPath: source.path) else { continue }
            var isDirectory: ObjCBool = false
            FileManager.default.fileExists(atPath: source.path, isDirectory: &isDirectory)
            if !isDirectory.boolValue { try Files.copy(source, to: payload.appendingPathComponent(path)); continue }
            try Files.directory(payload.appendingPathComponent(path))
            guard let items = FileManager.default.enumerator(at: source, includingPropertiesForKeys: [.isDirectoryKey]) else {
                throw ToolError.invalid("Cannot read SDK sources: \(source.path)")
            }
            for case let file as URL in items {
                if excluded.contains(file.lastPathComponent) { items.skipDescendants(); continue }
                let destination = payload.appendingPathComponent(Files.relative(file, to: root))
                if try file.resourceValues(forKeys: [.isDirectoryKey]).isDirectory == true {
                    try Files.directory(destination)
                } else { try Files.copy(file, to: destination) }
            }
        }
        // SDL's Java host is needed by Gradle; native dependency source trees are not shipped.
        let java = "Runtime/third-party/sdl3/android-project/app/src/main/java"
        if FileManager.default.fileExists(atPath: root.appendingPathComponent(java).path) {
            try Files.copy(root.appendingPathComponent(java), to: payload.appendingPathComponent(java))
        }
    }

    static func validate(_ payload: URL, version: String) throws {
        let manifest = try JSONDecoder().decode(NativeArtifacts.self,
            from: Data(contentsOf: payload.appendingPathComponent("Artifacts/native.json")))
        guard GuavaSDK.isSDK(payload), try SDKRelease.read(at: payload).version == version,
              manifest.version == version, manifest.platforms[CLIUpdate.platform] != nil,
              try String(contentsOf: payload.appendingPathComponent("VERSION"), encoding: .utf8)
                .trimmingCharacters(in: .whitespacesAndNewlines) == version,
              FileManager.default.fileExists(atPath: executable(at: payload).path),
              NativeArtifacts.isPrepared(at: payload) else {
            throw ToolError.invalid("Incomplete Guava SDK \(version): CLI, sources and host native libraries are required")
        }
    }

    static func executable(at root: URL) -> URL {
        root.appendingPathComponent(AppPlatform.host == .win ? "guava.exe" : "guava")
    }
}
