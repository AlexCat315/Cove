internal import Foundation

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

    static func isVersion(_ value: String) -> Bool {
        value.range(of: #"^[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?$"#, options: .regularExpression) != nil
    }
}
