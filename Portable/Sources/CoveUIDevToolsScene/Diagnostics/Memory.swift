import Foundation

/// Logical owned-resource estimates, not process physical footprint.
public struct UIDiagnosticMemorySnapshot: Codable, Sendable {
    public let counters: [String: Int]
    public let windows: [String: [String: Int]]
    public init(counters: [String: Int], windows: [String: [String: Int]]) {
        self.counters = counters
        self.windows = windows
    }
}
