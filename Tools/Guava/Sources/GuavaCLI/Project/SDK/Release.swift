internal import Foundation
private import ToolSupport

struct SDKRelease: Codable, Equatable {
    let version: String
    let swift: String
    let repository: String

    static let manifest = "Tools/Guava/Sources/GuavaCLI/SDK.json"
    static var current: Self {
        get throws { try JSONDecoder().decode(Self.self, from: Data(contentsOf: Bundle.module.url(forResource: "SDK", withExtension: "json")!)) }
    }

    static func read(at root: URL) throws -> Self {
        try JSONDecoder().decode(Self.self, from: Data(contentsOf: root.appendingPathComponent(manifest)))
    }

    /// A project may pin "latest" instead of an exact version to float to the
    /// newest locally installed SDK; `sdk install latest` updates what that is.
    static let latestTag = "latest"

    static func isVersion(_ value: String) -> Bool {
        value.range(of: #"^[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?$"#, options: .regularExpression) != nil
    }

    static func isVersionRequirement(_ value: String) -> Bool {
        isVersion(value) || value == latestTag
    }

    /// Resolves GitHub's "latest" release pointer (e.g. `sdk install latest`)
    /// without the api.github.com rate limit, by following the redirect from
    /// /releases/latest to /releases/tag/vX.Y.Z.
    static func latestVersion(repository: String) throws -> String {
        let base = repository.hasSuffix(".git") ? String(repository.dropLast(4)) : repository
        let resolved = try Download.redirectedURL(from: URL(string: base + "/releases/latest")!)
        let tag = resolved.lastPathComponent
        let version = String(tag.dropFirst(tag.hasPrefix("v") ? 1 : 0))
        guard isVersion(version) else {
            throw ToolError.invalid("The release channel did not resolve to a version: \(tag)")
        }
        return version
    }
}
