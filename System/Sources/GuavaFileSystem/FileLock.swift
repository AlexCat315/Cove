public import Foundation
private import CGuavaSystem

/// An operating-system lock shared by threads and processes, released on process exit.
public enum FileLock {
    public static func withLock<T>(at file: URL, _ body: () throws -> T) throws -> T {
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        var code: UInt32 = 0
        let handle = file.path.withCString { guava_file_lock($0, &code) }
        guard handle != -1 else {
            #if os(Windows)
            throw NSError(domain: "NSWin32ErrorDomain", code: Int(code), userInfo: [NSFilePathErrorKey: file.path])
            #else
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(code), userInfo: [NSFilePathErrorKey: file.path])
            #endif
        }
        defer { guava_file_unlock(handle) }
        return try body()
    }
}
