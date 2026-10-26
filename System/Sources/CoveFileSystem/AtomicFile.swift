public import Foundation

public enum AtomicFile {
    public enum Access: Sendable { case standard, ownerOnly }

    /// Writes a sibling staging file and publishes it with one replacement operation.
    public static func write(_ data: Data, to destination: URL, access: Access = .standard) throws {
        let staging = destination.deletingLastPathComponent()
            .appendingPathComponent(".cove-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: staging) }
        if access == .ownerOnly {
            try Data().write(to: staging, options: .withoutOverwriting)
            try FilePermissions.restrictToCurrentUser(staging)
            let handle = try FileHandle(forWritingTo: staging)
            do { try handle.write(contentsOf: data); try handle.close() }
            catch { try? handle.close(); throw error }
        } else {
            try data.write(to: staging, options: .withoutOverwriting)
            #if !os(Windows)
            if let mode = try? FileManager.default.attributesOfItem(atPath: destination.path)[.posixPermissions] {
                try FileManager.default.setAttributes([.posixPermissions: mode], ofItemAtPath: staging.path)
            }
            #endif
        }
        try FileReplacement.replace(destination, with: staging)
    }
}
