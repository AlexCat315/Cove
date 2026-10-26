internal import Foundation
#if canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#endif

#if os(macOS) || os(Linux) || os(Windows)
enum ProcessPlatform {
    static func prepareInput(_ handle: FileHandle) {
        #if os(macOS)
        _ = fcntl(handle.fileDescriptor, F_SETNOSIGPIPE, 1)
        #endif
    }
    static func write(_ data: Data, to handle: FileHandle) throws {
        #if os(Linux)
        var blocked = sigset_t(), original = sigset_t(), pending = sigset_t()
        sigemptyset(&blocked)
        sigaddset(&blocked, SIGPIPE)
        sigpending(&pending)
        let alreadyPending = sigismember(&pending, SIGPIPE) == 1
        pthread_sigmask(SIG_BLOCK, &blocked, &original)
        defer {
            if !alreadyPending, sigismember(&original, SIGPIPE) != 1 {
                var timeout = timespec(tv_sec: 0, tv_nsec: 0)
                _ = sigtimedwait(&blocked, nil, &timeout)
            }
            pthread_sigmask(SIG_SETMASK, &original, nil)
        }
        #endif
        try handle.write(contentsOf: data)
    }
}
#endif
