public import Foundation

#if os(macOS) || os(Linux) || os(Windows)
/// One native process lifetime, shared by LSP, tasks and extension imports.
/// Pipe readers drain concurrently; queued writes never block the caller.
public final class ProcessSession: @unchecked Sendable {
    public let id = UUID()
    public let events: AsyncThrowingStream<ProcessEvent, Error>
    private let eventBuffer: ProcessEventBuffer
    private let process = NativeProcess()
    private let input = Pipe()
    private let output = Pipe()
    private let errors = Pipe()
    private let writer = DispatchQueue(label: "guava.process.input")
    private let state = ProcessIOState()
    private let budget: ProcessBudget

    public init(command: ProcessCommand, budget: ProcessBudget = .init()) throws {
        guard command.executable.isFileURL, command.directory.isFileURL,
              !command.executable.path.isEmpty,
              !command.executable.path.contains("\0"), !command.directory.path.contains("\0"),
              !command.arguments.contains(where: { $0.contains("\0") }),
              command.environment.allSatisfy({ !$0.key.contains("=") && !$0.key.contains("\0") && !$0.value.contains("\0") }) else {
            throw ProcessError.invalidCommand
        }
        guard budget.outputBytes > 0, budget.outputBytes <= 33_554_432,
              budget.pendingInputBytes > 0, budget.pendingInputBytes <= 33_554_432,
              budget.lifetimeMilliseconds.map({ $0 > 0 && $0 <= 86_400_000 }) ?? true else {
            throw ProcessError.invalidBudget
        }
        self.budget = budget
        let eventBuffer = ProcessEventBuffer(maximumBytes: budget.outputBytes)
        self.eventBuffer = eventBuffer
        events = AsyncThrowingStream(unfolding: { try await eventBuffer.next() })
        ProcessPlatform.prepareInput(input.fileHandleForWriting)
        do {
            try process.start(command, input: input, output: output, errors: errors) { [weak self] status in
                self?.terminated(status)
            }
        } catch {
            try? output.fileHandleForReading.close()
            try? errors.fileHandleForReading.close()
            close()
            throw error
        }
        installReader(output.fileHandleForReading, isError: false)
        installReader(errors.fileHandleForReading, isError: true)
        if let milliseconds = budget.lifetimeMilliseconds {
            let timer = DispatchSource.makeTimerSource(queue: DispatchQueue(label: "guava.process.deadline"))
            timer.schedule(deadline: .now() + .milliseconds(milliseconds))
            timer.setEventHandler { [weak self] in self?.fail(ProcessError.timedOut) }
            state.installTimer(timer)
            timer.resume()
        }
    }

    deinit { close() }

    public var isRunning: Bool { process.isRunning }
    public var processIdentifier: Int32 { process.processIdentifier }

    public func write(_ data: Data) throws {
        try state.enqueue(data.count, maximum: budget.pendingInputBytes)
        let handle = input.fileHandleForWriting
        writer.async { [state, eventBuffer, process] in
            defer { state.wrote(data.count) }
            do { try ProcessPlatform.write(data, to: handle) }
            catch { Self.fail(error, state: state, events: eventBuffer, process: process) }
        }
    }

    public func closeInput() {
        state.closeInput()
        let handle = input.fileHandleForWriting
        writer.async { try? handle.close() }
    }

    public func cancel() { fail(CancellationError()) }

    public func nextEvent(until deadline: Date? = nil) throws -> ProcessEvent? {
        try eventBuffer.nextSynchronously(until: deadline)
    }

    private func installReader(_ handle: FileHandle, isError: Bool) {
        let thread = Thread { [weak self] in
            // The reader owns this handle until its blocking read completes.
            // Closing it from deinit races fileDescriptor access on macOS.
            defer { try? handle.close() }
            do {
                while true {
                    let data = try PipeRead.available(from: handle)
                    if data.isEmpty { break }
                    guard let self else { return }
                    if !eventBuffer.append(isError ? .stderr(data) : .stdout(data)) {
                        fail(ProcessError.outputLimit)
                        return
                    }
                }
                guard let self else { return }
                _ = eventBuffer.append(isError ? .stderrClosed : .stdoutClosed)
                if let status = state.closedPipe() { finish(status) }
            } catch { self?.fail(error) }
        }
        thread.name = isError ? "guava.process.stderr" : "guava.process.stdout"
        thread.start()
    }

    private func terminated(_ status: Int32) {
        if state.terminated(status) { finish(status) }
    }

    private func finish(_ status: Int32) {
        _ = state.stop()
        state.cancelTimer()
        _ = eventBuffer.append(.exited(status))
        eventBuffer.finish()
    }

    private func fail(_ error: Error) {
        Self.fail(error, state: state, events: eventBuffer, process: process)
    }

    private static func fail(_ error: Error, state: ProcessIOState,
                             events: ProcessEventBuffer, process: NativeProcess) {
        guard state.stop() else { return }
        events.finish(throwing: error)
        state.cancelTimer()
        process.terminate(force: false)
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + .milliseconds(300)) { [process] in
            process.terminate(force: true)
        }
    }

    private func close() {
        state.cancelTimer()
        _ = state.stop()
        eventBuffer.finish()
        process.terminate(force: true)
        let handle = input.fileHandleForWriting
        writer.async { try? handle.close() }
    }
}
#else
public final class ProcessSession: @unchecked Sendable {
    public let id = UUID()
    public let events = AsyncThrowingStream<ProcessEvent, Error> { $0.finish(throwing: ProcessError.unavailable) }
    public init(command: ProcessCommand, budget: ProcessBudget = .init()) throws { throw ProcessError.unavailable }
    public var isRunning: Bool { false }
    public var processIdentifier: Int32 { 0 }
    public func write(_ data: Data) throws { throw ProcessError.unavailable }
    public func closeInput() {}
    public func cancel() {}
    public func nextEvent(until deadline: Date? = nil) throws -> ProcessEvent? { throw ProcessError.unavailable }
}
#endif
