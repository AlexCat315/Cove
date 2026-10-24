#if os(Windows)
internal import Foundation
internal import WinSDK

/// Windows backend: a ConPTY pseudo console (Windows 10 1809+).
///
/// ConPTY keeps its output pipe open after the program exits, so a watcher
/// thread waits for the process, closes the console, and the reader thread
/// then drains the remaining output and sees end-of-file.
final class ConPTY: PseudoTerminalBackend, @unchecked Sendable {
    let processID: Int32
    private let lock = NSLock()
    private var console: HPCON?
    private let input: HANDLE
    private let output: HANDLE
    private let process: HANDLE

    /// `ProcThreadAttributeValue(22, FALSE, TRUE, FALSE)`; the macro is not imported.
    private static let pseudoConsoleAttribute = DWORD_PTR(0x0002_0016)

    static func spawn(_ command: TerminalCommand,
                      size: PseudoTerminal.Size,
                      onOutput: @escaping @Sendable ([UInt8]) -> Void,
                      onExit: @escaping @Sendable (Int32) -> Void) throws -> ConPTY {
        let environment = command.resolvedEnvironment()
        guard let path = command.resolvedExecutable(path: environment["Path"] ?? environment["PATH"]) else {
            throw TerminalError.executableNotFound(command.executable)
        }

        var inputRead: HANDLE?, inputWrite: HANDLE?, outputRead: HANDLE?, outputWrite: HANDLE?
        guard CreatePipe(&inputRead, &inputWrite, nil, 0), CreatePipe(&outputRead, &outputWrite, nil, 0),
              let inputRead, let inputWrite, let outputRead, let outputWrite else {
            throw TerminalError.spawnFailed(code: Int32(bitPattern: GetLastError()))
        }
        var console: HPCON?
        let created = CreatePseudoConsole(coord(size), inputRead, outputWrite, 0, &console)
        // The console holds its own references to these ends.
        CloseHandle(inputRead)
        CloseHandle(outputWrite)
        guard created == S_OK, let console else {
            CloseHandle(inputWrite); CloseHandle(outputRead)
            throw TerminalError.spawnFailed(code: created)
        }

        var listSize: SIZE_T = 0
        _ = InitializeProcThreadAttributeList(nil, 1, 0, &listSize)
        let listStorage = UnsafeMutableRawPointer.allocate(byteCount: Int(listSize), alignment: 16)
        let attributes = OpaquePointer(listStorage)
        defer { DeleteProcThreadAttributeList(attributes); listStorage.deallocate() }
        guard InitializeProcThreadAttributeList(attributes, 1, 0, &listSize),
              UpdateProcThreadAttribute(attributes, 0, pseudoConsoleAttribute, console,
                                        SIZE_T(MemoryLayout<HPCON>.size), nil, nil) else {
            let error = Int32(bitPattern: GetLastError())
            ClosePseudoConsole(console); CloseHandle(inputWrite); CloseHandle(outputRead)
            throw TerminalError.spawnFailed(code: error)
        }

        var startup = STARTUPINFOEXW()
        startup.StartupInfo.cb = DWORD(MemoryLayout<STARTUPINFOEXW>.size)
        startup.lpAttributeList = attributes
        var information = PROCESS_INFORMATION()
        let launchLine = WindowsCommandLine.commandLineString([path] + command.arguments)
        // Temporary diagnostics for the Windows ConPTY launch path.
        FileHandle.standardError.write(Data("ConPTY launch [\(command.arguments.count)]: \(launchLine)\n".utf8))
        var commandLine = Array(launchLine.utf16) + [0]
        var environmentBlock = WindowsCommandLine.environmentBlockUTF16(environment)
        let directory = command.workingDirectory.map { Array($0.path.utf16) + [0] }
        let flags = DWORD(EXTENDED_STARTUPINFO_PRESENT) | DWORD(CREATE_UNICODE_ENVIRONMENT)
        // ConPTY conveys the console through the thread attribute list; the
        // parent's std handles are irrelevant. bInheritHandles must be true so
        // the child can attach to the pseudoconsole, matching the Win32 sample.
        let launched = withUnsafeMutablePointer(to: &startup.StartupInfo) { startupInfo in
            environmentBlock.withUnsafeMutableBufferPointer { block in
                if let directory {
                    return directory.withUnsafeBufferPointer { cwd in
                        CreateProcessW(nil, &commandLine, nil, nil, true, flags, block.baseAddress,
                                       cwd.baseAddress, startupInfo, &information)
                    }
                }
                return CreateProcessW(nil, &commandLine, nil, nil, true, flags, block.baseAddress,
                                      nil, startupInfo, &information)
            }
        }
        guard launched, let process = information.hProcess else {
            let error = Int32(bitPattern: GetLastError())
            ClosePseudoConsole(console); CloseHandle(inputWrite); CloseHandle(outputRead)
            throw TerminalError.spawnFailed(code: error)
        }
        if let thread = information.hThread { CloseHandle(thread) }

        let terminal = ConPTY(processID: Int32(bitPattern: information.dwProcessId),
                              console: console, input: inputWrite, output: outputRead, process: process)
        terminal.startReading(onOutput: onOutput, onExit: onExit)
        return terminal
    }

    private init(processID: Int32, console: HPCON, input: HANDLE, output: HANDLE, process: HANDLE) {
        self.processID = processID
        self.console = console
        self.input = input
        self.output = output
        self.process = process
    }

    deinit {
        CloseHandle(input)
        CloseHandle(process)
    }

    private func startReading(onOutput: @escaping @Sendable ([UInt8]) -> Void,
                              onExit: @escaping @Sendable (Int32) -> Void) {
        let reader = Thread { [self] in
            var buffer = [UInt8](repeating: 0, count: 64 * 1024)
            while true {
                var count: DWORD = 0
                let ok = buffer.withUnsafeMutableBytes { ReadFile(output, $0.baseAddress, DWORD($0.count), &count, nil) }
                guard ok, count > 0 else { break }
                onOutput(Array(buffer.prefix(Int(count))))
            }
            CloseHandle(output)
            var code: DWORD = 0
            WaitForSingleObject(process, INFINITE)
            GetExitCodeProcess(process, &code)
            self.closeConsole()
            onExit(Int32(bitPattern: code))
        }
        reader.name = "guava.terminal.conpty.read"
        reader.start()
        let watcher = Thread { [self] in
            WaitForSingleObject(process, INFINITE)
            // Let the pseudoconsole flush the child's final writes before the
            // console is closed; closing on the instant of exit can drop the
            // tail of fast-exiting output.
            Thread.sleep(forTimeInterval: 0.5)
            closeConsole()
        }
        watcher.name = "guava.terminal.conpty.exit"
        watcher.start()
    }

    func write(_ bytes: [UInt8]) {
        lock.withLock {
            guard console != nil else { return }
            var offset = 0
            while offset < bytes.count {
                var written: DWORD = 0
                let ok = bytes[offset...].withUnsafeBytes {
                    WriteFile(input, $0.baseAddress, DWORD($0.count), &written, nil)
                }
                guard ok, written > 0 else { return }
                offset += Int(written)
            }
        }
    }

    func resize(_ size: PseudoTerminal.Size) {
        lock.withLock {
            guard let console else { return }
            _ = ResizePseudoConsole(console, Self.coord(size))
        }
    }

    /// Closing the console sends console programs CTRL_CLOSE_EVENT, as
    /// closing a console window does.
    func terminate() {
        closeConsole()
    }

    private func closeConsole() {
        let closing: HPCON? = lock.withLock {
            defer { console = nil }
            return console
        }
        if let closing { ClosePseudoConsole(closing) }
    }

    private static func coord(_ size: PseudoTerminal.Size) -> COORD {
        COORD(X: SHORT(clamping: max(1, size.columns)), Y: SHORT(clamping: max(1, size.rows)))
    }
}
#endif
