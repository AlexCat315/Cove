public import Foundation
#if os(Windows)
private import WinSDK
#elseif canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#elseif canImport(Android)
private import Android
#endif

/// Publishes a staged file on the same filesystem. Failure never deletes the destination.
public enum FileReplacement {
    public static func replace(_ destination: URL, with staging: URL) throws {
        guard destination.isFileURL, staging.isFileURL,
              destination.standardizedFileURL != staging.standardizedFileURL else {
            throw CocoaError(.fileWriteInvalidFileName)
        }
        #if os(Windows)
        try staging.path.withCString(encodedAs: UTF16.self) { source in
            try destination.path.withCString(encodedAs: UTF16.self) { target in
                for attempt in 0..<5 {
                    if MoveFileExW(source, target, DWORD(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) { return }
                    let code = GetLastError()
                    guard attempt < 4, code == ERROR_SHARING_VIOLATION || code == ERROR_LOCK_VIOLATION else {
                        throw NSError(domain: "NSWin32ErrorDomain", code: Int(code),
                                      userInfo: [NSFilePathErrorKey: destination.path])
                    }
                    Thread.sleep(forTimeInterval: 0.025 * Double(1 << attempt))
                }
            }
        }
        #else
        let status = staging.path.withCString { source in
            destination.path.withCString { target in rename(source, target) }
        }
        guard status == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        #endif
    }
}
