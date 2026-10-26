internal import Foundation
private import GuavaFileSystem
#if canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#endif

enum CLIInstaller {
    static func install(payload: URL, directory: URL, commands: URL) throws {
        let version = try String(contentsOf: payload.appendingPathComponent("VERSION"), encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard SDKRelease.isVersion(version) else { throw ToolError.invalid("Invalid CLI version in installation payload") }
        try SDKPayload.validate(payload, version: version)
        try FileLock.withLock(at: directory.appendingPathComponent("locks/sdk-\(version).lock")) {
            try installLocked(payload: payload, directory: directory, commands: commands, version: version)
        }
    }

    private static func installLocked(payload: URL, directory: URL, commands: URL, version: String) throws {
        #if os(Windows)
        let binary = payload.appendingPathComponent("guava.exe")
        #else
        let binary = payload.appendingPathComponent("guava")
        #endif
        guard FileManager.default.fileExists(atPath: binary.path) else { throw ToolError.invalid("CLI payload is missing its executable") }
        let receipt = InstallationReceipt(version: version, directory: directory, commands: commands)
        let destination = directory.appendingPathComponent("versions/\(version)")
        try Files.directory(destination.deletingLastPathComponent())
        if FileManager.default.fileExists(atPath: destination.path) {
            let existing = try InstallationReceipt.read(at: destination)
            guard existing.version == version, samePath(existing.directory, directory), samePath(existing.commands, commands) else {
                throw ToolError.invalid("Installation directory already contains an unmanaged version")
            }
            try SDKPayload.validate(destination, version: version)
        } else {
            let staging = directory.appendingPathComponent("versions/." + UUID().uuidString)
            defer { try? Files.remove(staging) }
            try Files.copy(payload, to: staging)
            try Files.json(receipt, to: staging.appendingPathComponent(InstallationReceipt.filename))
            try FileManager.default.moveItem(at: staging, to: destination)
        }
        try activate(version: destination, commands: commands, directory: directory)
        try Files.json(receipt, to: directory.appendingPathComponent(InstallationReceipt.filename))
        try SDKStore(directory: directory).use(destination)
        print("Installed Guava SDK \(version): \(commands.path)")
    }

    private static func activate(version: URL, commands: URL, directory: URL) throws {
        try Files.directory(commands)
        #if os(Windows)
        if FileManager.default.fileExists(atPath: commands.appendingPathComponent("guava.cmd").path) {
            let existing = try InstallationReceipt.read(at: directory)
            guard samePath(existing.directory, directory), samePath(existing.commands, commands) else {
                throw ToolError.invalid("The guava command is managed by another installation")
            }
        }
        let executable = version.appendingPathComponent("guava.exe").path.replacingOccurrences(of: "%", with: "%%")
        try Files.text("@\"\(executable)\" %*\r\n", to: commands.appendingPathComponent("guava.cmd"))
        #else
        let entry = commands.appendingPathComponent("guava")
        if FileManager.default.fileExists(atPath: entry.path) {
            let existing = try InstallationReceipt.read(at: entry.resolvingSymlinksInPath().deletingLastPathComponent())
            guard samePath(existing.directory, directory) else { throw ToolError.invalid("The guava command is managed by another installation") }
        }
        let temporary = commands.appendingPathComponent(".guava-" + UUID().uuidString)
        defer { try? Files.remove(temporary) }
        try FileManager.default.createSymbolicLink(at: temporary, withDestinationURL: version.appendingPathComponent("guava"))
        guard rename(temporary.path, entry.path) == 0 else { throw ToolError.invalid("Could not activate the new guava command") }
        #endif
    }

    private static func samePath(_ lhs: URL, _ rhs: URL) -> Bool {
        lhs.resolvingSymlinksInPath().path == rhs.resolvingSymlinksInPath().path
    }
}
