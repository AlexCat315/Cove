internal import Foundation
internal import ToolSupport

enum CLIUpdate {
    static var platform: String {
        #if arch(arm64)
        let architecture = "arm64"
        #else
        let architecture = "x86_64"
        #endif
        return "\(AppPlatform.host == .mac ? "macos" : AppPlatform.host == .win ? "windows" : "linux")-\(architecture)"
    }

    static func run(version requested: String?) throws {
        let executable = InstallationReceipt.executableDirectory
        guard let receipt = try? InstallationReceipt.read(at: executable) else {
            throw ToolError.invalid(executable.path.contains("/Cellar/")
                ? "Update this installation with brew upgrade guava"
                : "Use the package manager that installed guava, or install with the official standalone installer")
        }
        let temporary = FileManager.default.temporaryDirectory.appendingPathComponent("guava-update-" + UUID().uuidString)
        try Files.directory(temporary)
        defer { try? Files.remove(temporary) }
        let repository = try SDKRelease.current.repository.replacingOccurrences(of: ".git", with: "")
        let version: String
        if let requested {
            guard SDKRelease.isVersion(requested) else { throw ToolError.invalid("Supply an exact CLI version") }
            version = requested
        } else {
            // Follow the /releases/latest pointer on github.com instead of
            // hitting api.github.com, whose unauthenticated quota is 60 calls/hour.
            let resolved = try Download.redirectedURL(from: URL(string: repository + "/releases/latest")!)
            let tag = resolved.lastPathComponent
            version = String(tag.dropFirst(tag.hasPrefix("v") ? 1 : 0))
            guard SDKRelease.isVersion(version), version.compare(receipt.version, options: .numeric) != .orderedAscending else {
                throw ToolError.invalid("The release channel did not provide a newer supported version")
            }
        }
        if version == receipt.version { print("guava \(version) is current"); return }
        let name = "guava-\(platform).tar.gz"
        let base = repository + "/releases/download/v\(version)/"
        let checksums = temporary.appendingPathComponent("SHA256SUMS")
        try Download.metadata(from: URL(string: base + "SHA256SUMS")!, to: checksums)
        let digest = try checksum(for: name, in: String(contentsOf: checksums, encoding: .utf8))
        let archive = temporary.appendingPathComponent(name)
        try Download.file(from: URL(string: base + name)!, to: archive, sha256: digest)
        let expanded = temporary.appendingPathComponent("payload")
        try Files.directory(expanded)
        #if os(Windows)
        let tar = URL(fileURLWithPath: ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows").appendingPathComponent("System32/tar.exe").path
        #else
        let tar = "tar"
        #endif
        try Shell.run([tar, "-xf", archive.path, "-C", expanded.path])
        let payload = expanded.appendingPathComponent("guava")
        let packaged = try String(contentsOf: payload.appendingPathComponent("VERSION"), encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        guard packaged == version else { throw ToolError.invalid("CLI archive and requested version disagree") }
        #if os(Windows)
        let binary = payload.appendingPathComponent("guava.exe")
        #else
        let binary = payload.appendingPathComponent("guava")
        #endif
        let actual = try Shell.run([binary.path, "--version"], capture: true, quiet: true)
        guard actual == "guava \(version)" else { throw ToolError.invalid("Downloaded CLI failed its startup check") }
        try CLIInstaller.install(payload: payload, directory: receipt.directory, commands: receipt.commands)
    }

    static func checksum(for name: String, in contents: String) throws -> String {
        let matches = contents.split(whereSeparator: \.isNewline).compactMap { line -> String? in
            let fields = line.split(whereSeparator: \.isWhitespace)
            guard fields.count == 2, fields[1] == name,
                  String(fields[0]).range(of: #"^[0-9a-fA-F]{64}$"#, options: .regularExpression) != nil else { return nil }
            return String(fields[0])
        }
        guard matches.count == 1 else { throw ToolError.invalid("Release has no unique checksum for \(name)") }
        return matches[0]
    }
}
