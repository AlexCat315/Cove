import Foundation
import Testing
@testable import CoveCLI

@Test func desktopCompilerAcceptsAppleAndOfficialSwiftDistributions() {
    #expect(ToolchainRequirements.supportsCompiler("Apple Swift version 6.4 (swiftlang-6.4.1.42 clang-1800.0)"))
    #expect(ToolchainRequirements.supportsCompiler("Swift version 6.4.1 (swift-6.4.1-RELEASE)"))
    #expect(!ToolchainRequirements.supportsCompiler("Swift version 6.3.1 (swift-6.3.1-RELEASE)"))
}

@Test func updateRequiresOneExactChecksumEntry() throws {
    let digest = String(repeating: "a", count: 64)
    let name = "cove-macos-arm64.tar.gz"
    #expect(try CLIUpdate.checksum(for: name, in: "\(digest)  \(name)\n") == digest)
    for contents in ["", "bad  \(name)", "\(digest)  prefix-\(name)",
        "\(digest)  \(name)\n\(digest)  \(name)"] {
        #expect(throws: (any Error).self) { try CLIUpdate.checksum(for: name, in: contents) }
    }
}

@Test func installingNewVersionSwitchesCommandAndKeepsPreviousPayload() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let directory = base.appendingPathComponent("Cove home")
    let commands = base.appendingPathComponent("commands")
    #if os(Windows)
    let executable = "cove.exe"
    #else
    let executable = "cove"
    #endif
    for version in ["0.1.0", "0.1.1"] {
        let payload = base.appendingPathComponent("payload-\(version)")
        try makeSDK(at: payload, version: version)
        for path in NativeArtifacts.bundles(for: CLIUpdate.platform) {
            try Files.text("{}", to: payload.appendingPathComponent(path + ".artifactbundle/info.json"))
        }
        try Files.json(NativeArtifacts(version: version, platforms: [CLIUpdate.platform:
            .init(url: URL(string: "https://invalid.example/native.tar.gz")!, sha256: String(repeating: "a", count: 64))]),
            to: payload.appendingPathComponent("Artifacts/native.json"))
        try Files.text(version, to: payload.appendingPathComponent("VERSION"))
        try Files.text(version, to: payload.appendingPathComponent(executable))
        let installation = version == "0.1.0" ? directory : URL(fileURLWithPath: directory.path, isDirectory: true)
        try CLIInstaller.install(payload: payload, directory: installation, commands: commands)
        let installed = directory.appendingPathComponent("versions/\(version)")
        #expect(try InstallationReceipt.read(at: installed).version == version)
        #expect(SDKStore(directory: directory).selected == installed)
        #expect(CoveSDK.isSDK(installed))
        #if os(Windows)
        #expect(try String(contentsOf: commands.appendingPathComponent("cove.cmd"), encoding: .utf8).contains(installed.appendingPathComponent(executable).path))
        #else
        #expect(commands.appendingPathComponent("cove").resolvingSymlinksInPath() == installed.appendingPathComponent(executable))
        #endif
    }
    #expect(try String(contentsOf: directory.appendingPathComponent("versions/0.1.0/\(executable)"), encoding: .utf8) == "0.1.0")
    #expect(try InstallationReceipt.read(at: directory).version == "0.1.1")
}

@Test func installationPreservesAnUnmanagedCommand() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let payload = base.appendingPathComponent("payload"), commands = base.appendingPathComponent("bin")
    #if os(Windows)
    let executable = "cove.exe", entry = "cove.cmd"
    #else
    let executable = "cove", entry = "cove"
    #endif
    try Files.text("0.1.0", to: payload.appendingPathComponent("VERSION"))
    try Files.text("binary", to: payload.appendingPathComponent(executable))
    try makeSDK(at: payload)
    for path in NativeArtifacts.bundles(for: CLIUpdate.platform) {
        try Files.text("{}", to: payload.appendingPathComponent(path + ".artifactbundle/info.json"))
    }
    try Files.json(NativeArtifacts(version: "0.1.0", platforms: [CLIUpdate.platform:
        .init(url: URL(string: "https://invalid.example/native.tar.gz")!, sha256: String(repeating: "a", count: 64))]),
        to: payload.appendingPathComponent("Artifacts/native.json"))
    try Files.text("owned by user", to: commands.appendingPathComponent(entry))
    #expect(throws: (any Error).self) {
        try CLIInstaller.install(payload: payload, directory: base.appendingPathComponent("install"), commands: commands)
    }
    #expect(try String(contentsOf: commands.appendingPathComponent(entry), encoding: .utf8) == "owned by user")
}
