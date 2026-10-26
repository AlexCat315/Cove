internal import Foundation
internal import ToolSupport

enum CLIArchive {
    static func create(root: URL, output: URL) throws {
        let package = root.appendingPathComponent("Tools/Cove")
        let build = [CoveSDK.swift, "build", "--package-path", package.path,
                     "--configuration", "release", "--product", "cove", "--jobs", "4"]
        try Shell.run(build)
        let binaries = URL(fileURLWithPath: try Shell.run(build + ["--show-bin-path"], capture: true, quiet: true))
        let staging = FileManager.default.temporaryDirectory.appendingPathComponent("cove-package-" + UUID().uuidString)
        defer { try? Files.remove(staging) }
        let payload = staging.appendingPathComponent("cove")
        try Files.directory(payload)
        try SDKPayload.stageSources(from: root, into: payload)
        try NativeSDK.prepare(root: root)
        for path in NativeArtifacts.bundles(for: CLIUpdate.platform) {
            let relative = path + ".artifactbundle"
            try Files.copy(root.appendingPathComponent(relative), to: payload.appendingPathComponent(relative))
        }
        let manifest = root.appendingPathComponent("Artifacts/native.json")
        guard FileManager.default.fileExists(atPath: manifest.path) else {
            throw ToolError.invalid("Merge native release metadata into Artifacts/native.json before packaging the SDK")
        }
        try Files.copy(manifest, to: payload.appendingPathComponent("Artifacts/native.json"))
        #if os(Windows)
        let executable = binaries.appendingPathComponent("cove.exe")
        #else
        let executable = binaries.appendingPathComponent("cove")
        #endif
        try Files.copy(executable, to: payload.appendingPathComponent(executable.lastPathComponent))
        for file in try Files.children(binaries) where ["bundle", "resources"].contains(file.pathExtension) {
            try Files.copy(file, to: payload.appendingPathComponent(file.lastPathComponent))
        }
        try CLIRuntimeLibraries.stage(executable: executable, into: payload)
        try Files.text(try SDKRelease.current.version + "\n", to: payload.appendingPathComponent("VERSION"))
        try SDKPayload.validate(payload, version: SDKRelease.read(at: root).version)
        try Shell.run([payload.appendingPathComponent(executable.lastPathComponent).path, "--version"])
        try Files.directory(output)
        let archive = output.appendingPathComponent("cove-\(CLIUpdate.platform).tar.gz")
        #if os(Windows)
        let tar = URL(fileURLWithPath: ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows")
            .appendingPathComponent("System32/tar.exe").path
        #else
        let tar = "tar"
        #endif
        try Shell.run([tar, "-C", staging.path, "-czf", archive.path, "cove"])
        try Files.text(try SHA256.hex(file: archive) + "  " + archive.lastPathComponent + "\n",
                       to: output.appendingPathComponent("SHA256SUMS"))
        print(archive.path)
    }
}
