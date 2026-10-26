public import Foundation
#if canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#elseif canImport(Android)
private import Android
#elseif os(Windows)
private import WinSDK
#endif

public enum PipeRead {
    /// Waits for one available chunk or EOF, never for a completely filled buffer.
    /// The caller owns the handle and must not close it while a read is pending.
    public static func available(from handle: FileHandle, maximumBytes: Int = 65_536) throws -> Data {
        guard maximumBytes > 0, maximumBytes <= 33_554_432 else { throw ProcessError.invalidBudget }
        var buffer = [UInt8](repeating: 0, count: maximumBytes)
        #if os(Windows)
        var count: DWORD = 0
        let succeeded = buffer.withUnsafeMutableBytes {
            ReadFile(handle._handle, $0.baseAddress, DWORD($0.count), &count, nil)
        }
        guard succeeded else {
            let code = GetLastError()
            if code == ERROR_BROKEN_PIPE || code == ERROR_HANDLE_EOF { return Data() }
            throw NSError(domain: "NSWin32ErrorDomain", code: Int(code))
        }
        return Data(buffer.prefix(Int(count)))
        #else
        while true {
            let count = buffer.withUnsafeMutableBytes { bytes in
                #if canImport(Darwin)
                Darwin.read(handle.fileDescriptor, bytes.baseAddress, bytes.count)
                #elseif canImport(Glibc)
                Glibc.read(handle.fileDescriptor, bytes.baseAddress, bytes.count)
                #elseif canImport(Android)
                Android.read(handle.fileDescriptor, bytes.baseAddress, bytes.count)
                #else
                -1
                #endif
            }
            if count < 0, errno == EINTR { continue }
            guard count >= 0 else { throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno == 0 ? EIO : errno)) }
            return Data(buffer.prefix(count))
        }
        #endif
    }
}
