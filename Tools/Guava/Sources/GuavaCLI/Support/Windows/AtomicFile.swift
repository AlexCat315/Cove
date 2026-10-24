#if os(Windows)
import Foundation
private import WinSDK

/// Replace a file beside its staging file, retaining the old contents on failure.
enum WindowsAtomicFile {
    static func write(_ data: Data, to destination: URL) throws {
        let staging = destination.deletingLastPathComponent().appendingPathComponent(".guava-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: staging) }
        try data.write(to: staging, options: .withoutOverwriting)
        try staging.path.withCString(encodedAs: UTF16.self) { source in
            try destination.path.withCString(encodedAs: UTF16.self) { target in
                for attempt in 0..<5 {
                    if MoveFileExW(source, target, DWORD(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) { return }
                    let code = GetLastError()
                    guard attempt < 4,
                          code == DWORD(ERROR_SHARING_VIOLATION) || code == DWORD(ERROR_LOCK_VIOLATION) else {
                        throw ToolError.invalid("Could not replace \(destination.path) (Windows error \(code))")
                    }
                    // Bound retries to the two transient file-lock errors.
                    Thread.sleep(forTimeInterval: 0.025 * Double(1 << attempt))
                }
            }
        }
    }
}
#endif
