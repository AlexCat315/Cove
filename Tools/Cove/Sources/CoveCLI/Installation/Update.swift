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
                ? "Update this installation with brew upgrade cove"
                : "Use the package manager that installed cove, or install with the official standalone installer")
        }
        let repository = try SDKRelease.current.repository.replacingOccurrences(of: ".git", with: "")
        let version: String
        if let requested {
            guard SDKRelease.isVersion(requested) else { throw ToolError.invalid("Supply an exact CLI version") }
            version = requested
        } else {
            let latest = try SDKRelease.latestVersion(repository: repository)
            guard latest.compare(receipt.version, options: .numeric) != .orderedAscending else {
                throw ToolError.invalid("The release channel did not provide a newer supported version")
            }
            version = latest
        }
        if version == receipt.version { print("cove \(version) is current"); return }
        let store = SDKStore(directory: receipt.directory)
        try store.install(version: version)
        let payload = store.installed(version)
        #if os(Windows)
        let binary = payload.appendingPathComponent("cove.exe")
        #else
        let binary = payload.appendingPathComponent("cove")
        #endif
        let actual = try Shell.run([binary.path, "--version"], capture: true, quiet: true)
        guard actual == "cove \(version)" else { throw ToolError.invalid("Downloaded CLI failed its startup check") }
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
