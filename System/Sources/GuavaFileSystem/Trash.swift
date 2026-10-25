public import Foundation

public enum FileTrash {
    /// Uses the native macOS trash, or the caller's recoverable storage on other hosts.
    /// Never falls back to permanent deletion.
    @discardableResult
    public static func move(_ url: URL, recoveryDirectory: URL) throws -> URL {
        #if os(macOS)
        var destination: NSURL?
        try FileManager.default.trashItem(at: url, resultingItemURL: &destination)
        return (destination as URL?) ?? url
        #else
        guard !PathIdentity.contains(recoveryDirectory, in: url) else { throw CocoaError(.fileWriteInvalidFileName) }
        try FileManager.default.createDirectory(at: recoveryDirectory, withIntermediateDirectories: true)
        let destination = recoveryDirectory.appendingPathComponent(url.lastPathComponent + "-" + UUID().uuidString)
        try FileManager.default.moveItem(at: url, to: destination)
        return destination
        #endif
    }
}
