internal import Foundation
#if os(Windows)
private import CGuavaSystem
#elseif canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#endif

#if os(macOS) || os(Linux) || os(Windows)
/// Owns the native child and its process group; IO handles remain with the session's readers/writer.
final class NativeProcess: @unchecked Sendable {
    #if os(Windows)
    private var child: UnsafeMutableRawPointer?
    #else
    private let process = Process()
    private var pid: Int32 = 0
    #endif

    var processIdentifier: Int32 {
        #if os(Windows)
        child.map { guava_process_pid($0) } ?? 0
        #else
        pid
        #endif
    }
    var isRunning: Bool {
        #if os(Windows)
        child.map { guava_process_running($0) != 0 } ?? false
        #else
        process.isRunning
        #endif
    }

    func start(_ command: ProcessCommand, input: Pipe, output: Pipe, errors: Pipe,
               onExit: @escaping @Sendable (Int32) -> Void) throws {
        #if os(Windows)
        let launch = try WindowsLaunch(command)
        var error: UInt32 = 0
        child = launch.executable.withCString(encodedAs: UTF16.self) { executable in
            launch.command.withCString(encodedAs: UTF16.self) { arguments in
                launch.environment.withCString(encodedAs: UTF16.self) { environment in
                    launch.directory.withCString(encodedAs: UTF16.self) { directory in
                        guava_process_start(executable, arguments, environment, directory,
                            input.fileHandleForReading._handle, output.fileHandleForWriting._handle,
                            errors.fileHandleForWriting._handle, &error)
                    }
                }
            }
        }
        guard child != nil else { throw NSError(domain: "NSWin32ErrorDomain", code: Int(error)) }
        // Only the child keeps these ends. Readers must observe EOF when the job exits.
        try? input.fileHandleForReading.close()
        try? output.fileHandleForWriting.close()
        try? errors.fileHandleForWriting.close()
        let monitor = Thread { [self] in
            if let child { onExit(guava_process_wait(child)) }
        }
        monitor.name = "guava.process.exit"
        monitor.start()
        #else
        process.executableURL = command.executable
        process.arguments = command.arguments
        process.currentDirectoryURL = command.directory
        process.environment = command.environment
        process.standardInput = input
        process.standardOutput = output
        process.standardError = errors
        process.terminationHandler = { process in onExit(process.terminationStatus) }
        try process.run()
        pid = process.processIdentifier
        _ = setpgid(process.processIdentifier, process.processIdentifier)
        #endif
    }

    func terminate(force: Bool) {
        #if os(Windows)
        if let child { guava_process_terminate(child) }
        #else
        guard pid > 0 else { return }
        _ = kill(-pid, force ? SIGKILL : SIGTERM)
        if process.isRunning { _ = kill(pid, force ? SIGKILL : SIGTERM) }
        #endif
    }

    deinit {
        #if os(Windows)
        if let child { guava_process_destroy(child) }
        #else
        process.terminationHandler = nil
        #endif
    }
}
#endif
