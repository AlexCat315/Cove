internal import Foundation
internal import ToolSupport

enum CLIArchive {
    static func create(root: URL, output: URL) throws {
        let package = root.appendingPathComponent("Tools/Guava")
        let build = [GuavaSDK.swift, "build", "--package-path", package.path,
                     "--configuration", "release", "--product", "guava", "--jobs", "4"]
        try Shell.run(build)
        let binaries = URL(fileURLWithPath: try Shell.run(build + ["--show-bin-path"], capture: true, quiet: true))
        let staging = FileManager.default.temporaryDirectory.appendingPathComponent("guava-package-" + UUID().uuidString)
        defer { try? Files.remove(staging) }
        let payload = staging.appendingPathComponent("guava")
        try Files.directory(payload)
        #if os(Windows)
        let executable = binaries.appendingPathComponent("guava.exe")
        #else
        let executable = binaries.appendingPathComponent("guava")
        #endif
        try Files.copy(executable, to: payload.appendingPathComponent(executable.lastPathComponent))
        for file in try Files.children(binaries) where ["bundle", "resources"].contains(file.pathExtension) {
            try Files.copy(file, to: payload.appendingPathComponent(file.lastPathComponent))
        }
        try CLIRuntimeLibraries.stage(executable: executable, into: payload)
        try Files.text(try SDKRelease.current.version + "\n", to: payload.appendingPathComponent("VERSION"))
        try Shell.run([payload.appendingPathComponent(executable.lastPathComponent).path, "--version"])
        try Files.directory(output)
        let archive = output.appendingPathComponent("guava-\(CLIUpdate.platform).tar.gz")
        #if os(Windows)
        let tar = URL(fileURLWithPath: ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows")
            .appendingPathComponent("System32/tar.exe").path
        #else
        let tar = "tar"
        #endif
        try Shell.run([tar, "-C", staging.path, "-czf", archive.path, "guava"])
        try Files.text(try SHA256.hex(file: archive) + "  " + archive.lastPathComponent + "\n",
                       to: output.appendingPathComponent("SHA256SUMS"))
        print(archive.path)
    }
}
