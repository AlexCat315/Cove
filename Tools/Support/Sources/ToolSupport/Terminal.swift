public import Foundation

/// One error type for every repository tool, so the CLI can print a single
/// actionable line and return a non-zero status without leaking internals.
public enum ToolError: Error, CustomStringConvertible, LocalizedError {
  case message(String)
  case command(String, Int32)

  public var description: String {
    switch self {
    case .message(let text): return text
    case .command(let command, let status): return "\(command) exited with status \(status)"
    }
  }

  public var errorDescription: String? { description }
}

public enum Terminal {
  public static func info(_ text: String) { print(text) }

  public static func warning(_ text: String) {
    FileHandle.standardError.write(Data("warning: \(text)\n".utf8))
  }

  public static func error(_ text: String) {
    FileHandle.standardError.write(Data("error: \(text)\n".utf8))
  }

  public static func fail(_ text: String) -> Never {
    error(text)
    exit(1)
  }

  public static func fail(_ error: any Error) -> Never {
    if let tool = error as? ToolError { fail(tool.description) }
    fail(String(describing: error))
  }
}
