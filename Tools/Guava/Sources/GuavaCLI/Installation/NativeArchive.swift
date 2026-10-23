internal import Foundation
internal import ToolSupport

enum NativeArchive {
    static func create(root: URL, output: URL) throws {
        try NativeSDK.prepare(root: root, fromSource: true)
        try Files.directory(output)
        let release = try SDKRelease.read(at: root)
        let revision = ProcessInfo.processInfo.environment["GUAVA_NATIVE_RELEASE_REVISION"] ?? "1"
        guard let number = Int(revision), number > 0, String(number) == revision else {
            throw ToolError.invalid("Native release revision must be a positive integer")
        }
        let tag = "native-v\(release.version)-r\(revision)"
        let repository = release.repository.replacingOccurrences(of: ".git", with: "")
        var archives: [String: NativeArtifacts.Archive] = [:]
        for directory in ["Runtime/vendor", "vendor"] {
            let vendor = root.appendingPathComponent(directory)
            for bundle in try Files.children(vendor) where bundle.pathExtension == "artifactbundle" {
                let name = bundle.deletingPathExtension().lastPathComponent
                let filename = "\(name)-\(CLIUpdate.platform).zip"
                let archive = output.appendingPathComponent(filename)
                try Files.remove(archive)
                #if os(Windows)
                let escape = { (value: String) in value.replacingOccurrences(of: "'", with: "''") }
                try Shell.run(["powershell.exe", "-NoProfile", "-Command",
                    "Compress-Archive -LiteralPath '\(escape(bundle.path))' -DestinationPath '\(escape(archive.path))'"])
                #else
                try Shell.run(["zip", "-qr", archive.path, bundle.lastPathComponent], at: vendor)
                #endif
                archives[name] = NativeArtifacts.Archive(
                    url: URL(string: "\(repository)/releases/download/\(tag)/\(filename)")!,
                    sha256: try SHA256.hex(file: archive), directory: directory)
            }
        }
        guard archives.count == 8 else { throw ToolError.invalid("Expected eight SDK native libraries") }
        try Files.json(NativeArtifacts(version: release.version, platforms: [CLIUpdate.platform: archives]),
            to: output.appendingPathComponent("native-\(CLIUpdate.platform).json"))
    }
}
