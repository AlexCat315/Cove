public import Foundation
public import GuavaProcess

/// Tool boundary: resolves PATH and chooses capture or live output; process IO is shared with applications.
public enum ProcessRunner {
  public static func run(
    _ arguments: [String], at directory: URL? = nil,
    environment: [String: String] = ProcessInfo.processInfo.environment,
    input: Data? = nil, timeout: TimeInterval? = nil,
    mergeError: Bool = false, stream: Bool = false
  ) throws -> ProcessOutput {
    guard let executable = arguments.first else { throw ToolError.message("Empty command") }
    var command = ProcessCommand(executable: try Shell.require(executable, environment: environment),
      directory: directory ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath))
    command.arguments = Array(arguments.dropFirst())
    command.environment = environment
    var budget = ProcessBudget()
    budget.outputBytes = 33_554_432
    budget.pendingInputBytes = 33_554_432
    if let timeout {
      guard timeout.isFinite, timeout > 0, timeout <= 86_400 else { throw ProcessError.invalidBudget }
      budget.lifetimeMilliseconds = max(1, Int(timeout * 1000))
    } else { budget.lifetimeMilliseconds = nil }
    let session = try ProcessSession(command: command, budget: budget)
    defer { session.cancel() }
    if let input, !input.isEmpty { try session.write(input) }
    session.closeInput()
    var result = ProcessOutput()
    while let event = try session.nextEvent() {
      switch event {
      case let .stdout(data):
        if stream { try FileHandle.standardOutput.write(contentsOf: data) }
        else { result.stdout.append(data) }
      case let .stderr(data):
        if stream { try FileHandle.standardError.write(contentsOf: data) }
        else if mergeError { result.stdout.append(data) }
        else { result.stderr.append(data) }
      case let .exited(status): result.status = status
      case .stdoutClosed, .stderrClosed: break
      }
      guard result.stdout.count + result.stderr.count <= budget.outputBytes else { throw ProcessError.outputLimit }
    }
    return result
  }
}
