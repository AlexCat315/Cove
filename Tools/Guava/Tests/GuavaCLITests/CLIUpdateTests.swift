import Foundation
import Testing
@testable import GuavaCLI

@Test func updateRequiresOneExactChecksumEntry() throws {
    let digest = String(repeating: "a", count: 64)
    let name = "guava-macos-arm64.tar.gz"
    #expect(try CLIUpdate.checksum(for: name, in: "\(digest)  \(name)\n") == digest)
    for contents in ["", "bad  \(name)", "\(digest)  prefix-\(name)",
        "\(digest)  \(name)\n\(digest)  \(name)"] {
        #expect(throws: (any Error).self) { try CLIUpdate.checksum(for: name, in: contents) }
    }
}

@Test func installingNewVersionSwitchesCommandAndKeepsPreviousPayload() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let directory = base.appendingPathComponent("Guava home")
    let commands = base.appendingPathComponent("commands")
    #if os(Windows)
    let executable = "guava.exe"
    #else
    let executable = "guava"
    #endif
    for version in ["0.1.0", "0.1.1"] {
        let payload = base.appendingPathComponent("payload-\(version)")
        try Files.text(version, to: payload.appendingPathComponent("VERSION"))
        try Files.text(version, to: payload.appendingPathComponent(executable))
        try CLIInstaller.install(payload: payload, directory: directory, commands: commands)
        let installed = directory.appendingPathComponent("versions/\(version)")
        #expect(try InstallationReceipt.read(at: installed).version == version)
        #if os(Windows)
        #expect(try String(contentsOf: commands.appendingPathComponent("guava.cmd"), encoding: .utf8).contains(installed.appendingPathComponent(executable).path))
        #else
        #expect(commands.appendingPathComponent("guava").resolvingSymlinksInPath() == installed.appendingPathComponent(executable))
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
    let executable = "guava.exe", entry = "guava.cmd"
    #else
    let executable = "guava", entry = "guava"
    #endif
    try Files.text("0.1.0", to: payload.appendingPathComponent("VERSION"))
    try Files.text("binary", to: payload.appendingPathComponent(executable))
    try Files.text("owned by user", to: commands.appendingPathComponent(entry))
    #expect(throws: (any Error).self) {
        try CLIInstaller.install(payload: payload, directory: base.appendingPathComponent("install"), commands: commands)
    }
    #expect(try String(contentsOf: commands.appendingPathComponent(entry), encoding: .utf8) == "owned by user")
}
