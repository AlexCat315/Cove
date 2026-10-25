public import Foundation
private import GuavaProcess

/// Thin, explicit process launching. Arguments are always passed as a vector;
/// no tool in this public interprets a shell string.
public enum Shell {
  public static func executable(
    _ name: String,
    environment: [String: String] = ProcessInfo.processInfo.environment
  ) -> URL? {
    ToolLocator.executable(name, environment: environment)
  }

  public static func require(
    _ name: String,
    environment: [String: String] = ProcessInfo.processInfo.environment
  ) throws -> URL {
    guard let url = executable(name, environment: environment) else {
      throw ToolError.message("Required tool not found on PATH: \(name)")
    }
    return url
  }

  /// Runs to completion and returns trimmed standard output.
  @discardableResult
  public static func output(
    _ arguments: [String], at directory: URL? = nil,
    environment: [String: String] = ProcessInfo.processInfo.environment,
    echo: Bool = false
  ) throws -> String {
    try run(arguments, at: directory, environment: environment, capture: true, echo: echo)
  }

  /// Runs to completion, forwarding the child's standard streams to this tool.
  public static func inherit(
    _ arguments: [String], at directory: URL? = nil,
    environment: [String: String] = ProcessInfo.processInfo.environment,
    input: String? = nil
  ) throws {
    _ = try run(
      arguments, at: directory, environment: environment, capture: false, echo: true, input: input)
  }

  @discardableResult
  public static func run(
    _ arguments: [String], at directory: URL? = nil,
    environment: [String: String] = ProcessInfo.processInfo.environment,
    capture: Bool, echo: Bool, input: String? = nil
  ) throws -> String {
    guard let command = arguments.first, !command.isEmpty else {
      throw ToolError.message("Empty command")
    }
    if echo { print("$ " + arguments.joined(separator: " ")) }
    if capture {
      let result = try ProcessRunner.run(
        arguments, at: directory, environment: environment,
        input: input.map { Data($0.utf8) }, mergeError: true)
      guard result.status == 0 else {
        if !result.stdout.isEmpty { Terminal.error(result.stdoutText) }
        throw ToolError.command(command, result.status)
      }
      return result.stdoutText
    }
    let result = try ProcessRunner.run(arguments, at: directory, environment: environment,
      input: input.map { Data($0.utf8) }, stream: true)
    guard result.status == 0 else { throw ToolError.command(command, result.status) }

    return ""
  }
}
