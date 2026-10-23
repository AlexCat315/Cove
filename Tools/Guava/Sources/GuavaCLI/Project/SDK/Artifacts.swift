internal import Foundation
internal import ToolSupport

struct NativeArtifacts: Codable {
    struct Archive: Codable {
        let url: URL
        let sha256: String
        let directory: String
    }
    let version: String
    let platforms: [String: [String: Archive]]

    static func install(at root: URL) throws -> Bool {
        let file = root.appendingPathComponent("Artifacts/native.json")
        guard let data = try? Data(contentsOf: file) else { return false }
        let manifest = try JSONDecoder().decode(Self.self, from: data)
        guard let archives = manifest.platforms[CLIUpdate.platform], !archives.isEmpty else { return false }
        guard manifest.version == (try SDKRelease.read(at: root).version) else { throw ToolError.invalid("Native artifacts and SDK versions disagree") }
        let cache = Installation.directory.appendingPathComponent("cache/native/\(manifest.version)/\(CLIUpdate.platform)")
        for (name, archive) in archives.sorted(by: { $0.key < $1.key }) {
            guard ["vendor", "Runtime/vendor"].contains(archive.directory),
                  name.range(of: #"^[A-Za-z][A-Za-z0-9_]*$"#, options: .regularExpression) != nil else {
                throw ToolError.invalid("Invalid native artifact location")
            }
            let directory = root.appendingPathComponent(archive.directory)
            let bundle = directory.appendingPathComponent("\(name).artifactbundle")
            if FileManager.default.fileExists(atPath: bundle.appendingPathComponent("info.json").path) { continue }
            let zip = cache.appendingPathComponent("\(name).zip")
            try Download.file(from: archive.url, to: zip, sha256: archive.sha256)
            let staging = cache.appendingPathComponent(UUID().uuidString)
            defer { try? Files.remove(staging) }
            try Files.directory(staging)
            #if os(Windows)
            let escaped = { (value: String) in value.replacingOccurrences(of: "'", with: "''") }
            try Shell.run(["powershell.exe", "-NoProfile", "-Command",
                "Expand-Archive -LiteralPath '\(escaped(zip.path))' -DestinationPath '\(escaped(staging.path))'"])
            #else
            try Shell.run(["unzip", "-q", zip.path, "-d", staging.path])
            #endif
            let expanded = staging.appendingPathComponent("\(name).artifactbundle")
            guard FileManager.default.fileExists(atPath: expanded.appendingPathComponent("info.json").path) else {
                throw ToolError.invalid("Native archive is missing \(name).artifactbundle")
            }
            try Files.copy(expanded, to: bundle)
        }
        return true
    }
}
