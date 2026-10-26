internal import Foundation
private import ToolSupport

enum SDKDownload {
    static func fetch(version: String, into staging: URL) throws -> URL {
        let repository = try SDKRelease.current.repository.replacingOccurrences(of: ".git", with: "")
        let base = repository + "/releases/download/v\(version)/"
        let name = "cove-\(CLIUpdate.platform).tar.gz"
        try Files.directory(staging)
        let sums = staging.appendingPathComponent("SHA256SUMS")
        try Download.metadata(from: URL(string: base + "SHA256SUMS")!, to: sums)
        let digest = try CLIUpdate.checksum(for: name, in: String(contentsOf: sums, encoding: .utf8))
        let archive = staging.appendingPathComponent(name)
        try Download.file(from: URL(string: base + name)!, to: archive, sha256: digest)
        let expanded = staging.appendingPathComponent("expanded")
        try TarArchive.extract(archive, to: expanded)
        let payload = expanded.appendingPathComponent("cove")
        try SDKPayload.validate(payload, version: version)
        return payload
    }
}
