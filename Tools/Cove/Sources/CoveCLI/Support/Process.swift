import CoveLogging
import Foundation

private import ToolSupport
private import CoveProcess

enum ToolError: Error, LocalizedError {
    case invalid(String)
    case failed(String, Int32)

    var errorDescription: String? {
        switch self {
        case .invalid(let message): return message
        case .failed(let command, let status): return "\(command) exited with status \(status)"
        }
    }
}

enum Shell {
    static func executable(_ name: String, environment: [String: String] = ProcessInfo.processInfo.environment) throws -> URL {
        guard let executable = ToolSupport.Shell.executable(name, environment: environment) else {
            throw ToolError.invalid("Executable not found on PATH: \(name)")
        }
        return executable
    }

    static func start(_ arguments: [String], at directory: URL? = nil,
                      environment: [String: String] = ProcessInfo.processInfo.environment) throws -> Process {
        guard let command = arguments.first else { throw ToolError.invalid("Empty command") }
        let process = Process()
        process.executableURL = try executable(command, environment: environment)
        process.arguments = Array(arguments.dropFirst()); process.currentDirectoryURL = directory
        var childEnvironment = environment
        childEnvironment["COVE_LOG_RUN_ID"] = childEnvironment["COVE_LOG_RUN_ID"] ?? LogSystem.launch.runID
        process.environment = childEnvironment
        try process.run()
        return process
    }

    /// Passes arguments directly to the executable; never interprets a shell command.
    @discardableResult
    static func run(_ arguments: [String], at directory: URL? = nil,
                    environment: [String: String] = ProcessInfo.processInfo.environment,
                    capture: Bool = false, quiet: Bool = false, stdout: FileHandle = .standardOutput) throws -> String {
        guard let command = arguments.first else { throw ToolError.invalid("Empty command") }
        let logger = LogSystem.logger("com.cove.cli.process")
        if !quiet { logger.debug("Starting command", metadata: ["command": "\(URL(fileURLWithPath: command).lastPathComponent)", "arguments": "\(arguments.count - 1)"]) }
        var request = ProcessCommand(executable: try executable(command, environment: environment),
            directory: directory ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath))
        request.arguments = Array(arguments.dropFirst())
        request.environment = environment
        request.environment["COVE_LOG_RUN_ID"] = request.environment["COVE_LOG_RUN_ID"] ?? LogSystem.launch.runID
        var budget = ProcessBudget()
        budget.outputBytes = 33_554_432
        budget.lifetimeMilliseconds = nil
        let session = try ProcessSession(command: request, budget: budget)
        defer { session.cancel() }
        session.closeInput()
        let output = ProcessStream(command: command, stream: "stdout", destination: capture ? nil : stdout, collect: capture)
        let errors = ProcessStream(command: command, stream: "stderr", destination: .standardError)
        var status: Int32 = 0
        while let event = try session.nextEvent() {
            switch event {
            case let .stdout(data): output.consume(data, pid: session.processIdentifier)
            case let .stderr(data): errors.consume(data, pid: session.processIdentifier)
            case let .exited(code): status = code
            case .stdoutClosed, .stderrClosed: break
            }
        }
        let data = output.finish(pid: session.processIdentifier)
        _ = errors.finish(pid: session.processIdentifier)
        guard status == 0 else { throw ToolError.failed(command, status) }
        return String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)

    }
}
